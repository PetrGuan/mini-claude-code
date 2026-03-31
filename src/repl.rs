use crate::api::client::AnthropicClient;
use crate::api::stream::SseEvent;
use crate::api::types::{
    ContentBlock, ContentBlockStartData, DeltaData, Message, Role, StreamEvent,
};
use crate::tools::ToolRegistry;
use crate::ui::input::read_user_input;
use crate::ui::render::print_stream_chunk;
use anyhow::Result;

/// Max characters to store per tool result in conversation history (I3)
const MAX_TOOL_RESULT_CHARS: usize = 40_000;

/// Truncate a string at a UTF-8 safe boundary (C1)
fn truncate_utf8(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

pub async fn run(client: &AnthropicClient, registry: &ToolRegistry) -> Result<()> {
    let mut messages: Vec<Message> = Vec::new();
    let tool_defs = registry.definitions();

    println!("\x1b[1;32mmini-claude-code\x1b[0m v0.1.0");
    println!("Model: {}", client.model);
    println!("Type your message (press Enter twice to send, Ctrl+C to exit)\n");

    loop {
        let input = match read_user_input() {
            Some(s) if s.is_empty() => continue,
            Some(s) => s,
            None => {
                println!("Goodbye!");
                break;
            }
        };

        messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: input }],
        });

        // Tool use loop: keep calling API until model stops using tools
        loop {
            // I4: API errors are non-fatal — display and return to prompt
            let mut rx = match client.send_message_stream(&messages, &tool_defs).await {
                Ok(rx) => rx,
                Err(e) => {
                    eprintln!("\n\x1b[1;31mAPI Error: {}\x1b[0m", e);
                    // Remove the last user message so user can retry
                    messages.pop();
                    break;
                }
            };

            let mut assistant_content: Vec<ContentBlock> = Vec::new();
            let mut current_text = String::new();
            let mut current_tool_id = String::new();
            let mut current_tool_name = String::new();
            let mut current_tool_input_json = String::new();
            let mut stream_error = false;

            println!();

            while let Some(event_result) = rx.recv().await {
                // I4: Stream errors are non-fatal
                let sse_event = match event_result {
                    Ok(ev) => ev,
                    Err(e) => {
                        eprintln!("\n\x1b[1;31mStream error: {}\x1b[0m", e);
                        stream_error = true;
                        break;
                    }
                };

                match sse_event {
                    SseEvent::Event(event) => match event {
                        StreamEvent::ContentBlockStart { content_block, .. } => {
                            match content_block {
                                ContentBlockStartData::Text { text } => {
                                    current_text = text;
                                }
                                ContentBlockStartData::ToolUse { id, name } => {
                                    if !current_text.is_empty() {
                                        assistant_content.push(ContentBlock::Text {
                                            text: current_text.clone(),
                                        });
                                        current_text.clear();
                                    }
                                    current_tool_id = id;
                                    current_tool_name = name.clone();
                                    current_tool_input_json.clear();
                                    println!("\n\x1b[1;33m[Tool: {}]\x1b[0m", name);
                                }
                            }
                        }
                        StreamEvent::ContentBlockDelta { delta, .. } => match delta {
                            DeltaData::TextDelta { text } => {
                                print_stream_chunk(&text);
                                current_text.push_str(&text);
                            }
                            DeltaData::InputJsonDelta { partial_json } => {
                                current_tool_input_json.push_str(&partial_json);
                            }
                        },
                        StreamEvent::ContentBlockStop { .. } => {
                            if !current_tool_name.is_empty() {
                                let tool_input: serde_json::Value =
                                    serde_json::from_str(&current_tool_input_json)
                                        .unwrap_or(serde_json::Value::Object(Default::default()));
                                assistant_content.push(ContentBlock::ToolUse {
                                    id: current_tool_id.clone(),
                                    name: current_tool_name.clone(),
                                    input: tool_input,
                                });
                                current_tool_name.clear();
                            } else if !current_text.is_empty() {
                                assistant_content.push(ContentBlock::Text {
                                    text: current_text.clone(),
                                });
                                current_text.clear();
                            }
                        }
                        StreamEvent::MessageDelta { .. } => {}
                        StreamEvent::MessageStop => {}
                        StreamEvent::Ping => {}
                        StreamEvent::MessageStart { .. } => {}
                        StreamEvent::Error { error } => {
                            eprintln!("\n\x1b[1;31mAPI Error: {}\x1b[0m", error.message);
                            stream_error = true;
                            break;
                        }
                    },
                    SseEvent::Done => break,
                }
            }

            println!();

            // If stream errored, drop partial response and return to prompt
            if stream_error {
                messages.pop(); // remove the user message that caused the error
                break;
            }

            messages.push(Message {
                role: Role::Assistant,
                content: assistant_content.clone(),
            });

            let tool_uses: Vec<_> = assistant_content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolUse { id, name, input } => {
                        Some((id.clone(), name.clone(), input.clone()))
                    }
                    _ => None,
                })
                .collect();

            if tool_uses.is_empty() {
                break;
            }

            let mut tool_results: Vec<ContentBlock> = Vec::new();
            for (id, name, input) in &tool_uses {
                match registry.get(name) {
                    Some(tool) => {
                        println!("\x1b[2m  Executing {}...\x1b[0m", name);
                        match tool.execute(input.clone()).await {
                            Ok(result) => {
                                let preview = truncate_utf8(&result.content, 200);
                                println!("\x1b[2m  Result: {}\x1b[0m", preview);
                                // I3: Truncate large tool results before storing in history
                                let stored_content =
                                    truncate_utf8(&result.content, MAX_TOOL_RESULT_CHARS);
                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: stored_content,
                                    is_error: if result.is_error { Some(true) } else { None },
                                });
                            }
                            Err(e) => {
                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: format!("Error: {}", e),
                                    is_error: Some(true),
                                });
                            }
                        }
                    }
                    None => {
                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: format!("Error: unknown tool '{}'", name),
                            is_error: Some(true),
                        });
                    }
                }
            }

            messages.push(Message {
                role: Role::User,
                content: tool_results,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_utf8_ascii() {
        let s = "hello world";
        assert_eq!(truncate_utf8(s, 5), "hello...");
    }

    #[test]
    fn test_truncate_utf8_no_truncation() {
        let s = "hello";
        assert_eq!(truncate_utf8(s, 10), "hello");
    }

    #[test]
    fn test_truncate_utf8_multibyte() {
        let s = "你好世界测试";
        assert_eq!(truncate_utf8(s, 4), "你好世界...");
    }
}
