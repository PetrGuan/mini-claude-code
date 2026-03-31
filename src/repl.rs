use crate::api::client::AnthropicClient;
use crate::api::stream::SseEvent;
use crate::api::types::{ContentBlock, ContentBlockStartData, DeltaData, Message, Role, StreamEvent};
use crate::tools::ToolRegistry;
use crate::ui::input::read_user_input;
use crate::ui::render::{create_skin, print_stream_chunk};
use anyhow::Result;

pub async fn run(client: &AnthropicClient, registry: &ToolRegistry) -> Result<()> {
    let _skin = create_skin();
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
            let mut rx = client.send_message_stream(&messages, &tool_defs).await?;

            let mut assistant_content: Vec<ContentBlock> = Vec::new();
            let mut current_text = String::new();
            let mut current_tool_id = String::new();
            let mut current_tool_name = String::new();
            let mut current_tool_input_json = String::new();

            println!();

            while let Some(event_result) = rx.recv().await {
                match event_result? {
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
                        }
                    },
                    SseEvent::Done => break,
                }
            }

            println!();

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
                                let preview = if result.content.len() > 200 {
                                    format!("{}...", &result.content[..200])
                                } else {
                                    result.content.clone()
                                };
                                println!("\x1b[2m  Result: {}\x1b[0m", preview);
                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: result.content,
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
