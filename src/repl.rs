use crate::api::client::AnthropicClient;
use crate::api::stream::SseEvent;
use crate::api::types::{
    ContentBlock, ContentBlockStartData, DeltaData, Message, Role, StreamEvent,
};
use crate::tools::ToolRegistry;
use crate::ui::input::read_user_input;
use crate::ui::render::{
    count_display_lines, print_separator, print_stream_chunk, render_final_response,
};
use crate::ui::spinner::Spinner;
use anyhow::Result;

/// Max characters to store per tool result in conversation history
const MAX_TOOL_RESULT_CHARS: usize = 40_000;

/// Truncate a string at a UTF-8 safe boundary
fn truncate_utf8(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

/// Format a tool input for display — show key parameters concisely
fn format_tool_input(name: &str, input: &serde_json::Value) -> String {
    match name {
        "bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| truncate_utf8(s, 80))
            .unwrap_or_default(),
        "read" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "edit" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "glob" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("{} in {}", pattern, path)
        }
        "grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("/{}/ in {}", pattern, path)
        }
        _ => String::new(),
    }
}

/// Format tool result for preview — show first few meaningful lines
fn format_tool_preview(content: &str, is_error: bool) -> String {
    let prefix = if is_error {
        "\x1b[31m✗\x1b[0m"
    } else {
        "\x1b[32m✓\x1b[0m"
    };

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return format!("  {} (empty)", prefix);
    }

    let preview_lines = if lines.len() <= 3 {
        lines
    } else {
        let mut v = lines[..3].to_vec();
        v.push(&"...");
        v
    };

    let formatted: Vec<String> = preview_lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let truncated = truncate_utf8(line, 120);
            if i == 0 {
                format!("  {} {}", prefix, truncated)
            } else {
                format!("    {}", truncated)
            }
        })
        .collect();

    formatted.join("\n")
}

pub async fn run(client: &AnthropicClient, registry: &ToolRegistry) -> Result<()> {
    let mut messages: Vec<Message> = Vec::new();
    let tool_defs = registry.definitions();

    // Welcome banner
    println!();
    println!("  \x1b[1;36m◆ mini-claude-code\x1b[0m \x1b[2mv0.1.0\x1b[0m");
    println!("  \x1b[2m{} · bash, read, write, edit, glob, grep\x1b[0m", client.model);
    println!("  \x1b[2mEnter twice to send · Ctrl+C to exit\x1b[0m");
    println!();

    let mut turn = 0;
    loop {
        if turn > 0 {
            print_separator();
            println!();
        }
        let input = match read_user_input() {
            Some(s) if s.is_empty() => continue,
            Some(s) => s,
            None => {
                println!("\n  \x1b[2mGoodbye!\x1b[0m");
                break;
            }
        };

        turn += 1;
        messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: input }],
        });

        // Tool use loop: keep calling API until model stops using tools
        loop {
            // Show animated spinner while waiting for API
            let spinner = Spinner::start("Thinking...");

            let mut rx = match client.send_message_stream(&messages, &tool_defs).await {
                Ok(rx) => {
                    drop(spinner);
                    rx
                }
                Err(e) => {
                    drop(spinner);
                    eprintln!("\x1b[1;31m  API Error: {}\x1b[0m\n", e);
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
            let mut first_text = true;
            let mut has_text_content = false;

            while let Some(event_result) = rx.recv().await {
                let sse_event = match event_result {
                    Ok(ev) => ev,
                    Err(e) => {
                        eprintln!("\n\x1b[1;31m  Stream error: {}\x1b[0m", e);
                        stream_error = true;
                        break;
                    }
                };

                match sse_event {
                    SseEvent::Event(event) => match event {
                        StreamEvent::ContentBlockStart { content_block, .. } => {
                            match content_block {
                                ContentBlockStartData::Text { text } => {
                                    if first_text {
                                        println!(); // blank line before streaming
                                        first_text = false;
                                    }
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
                                }
                            }
                        }
                        StreamEvent::ContentBlockDelta { delta, .. } => match delta {
                            DeltaData::TextDelta { text } => {
                                print_stream_chunk(&text);
                                current_text.push_str(&text);
                                has_text_content = true;
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
                            eprintln!("\n\x1b[1;31m  API Error: {}\x1b[0m", error.message);
                            stream_error = true;
                            break;
                        }
                    },
                    SseEvent::Done => break,
                }
            }

            // Re-render streamed text with markdown formatting
            if has_text_content && !stream_error {
                let full_text: String = assistant_content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .chain(
                        if !current_text.is_empty() {
                            Some(current_text.as_str())
                        } else {
                            None
                        },
                    )
                    .collect::<Vec<_>>()
                    .join("");

                if !full_text.is_empty() {
                    let lines = count_display_lines(&full_text);
                    render_final_response(&full_text, lines);
                }
            } else {
                println!();
            }

            if stream_error {
                messages.pop();
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

            // Execute tools with nice formatting
            let mut tool_results: Vec<ContentBlock> = Vec::new();
            for (id, name, input) in &tool_uses {
                let input_desc = format_tool_input(name, input);
                let tool_label = if input_desc.is_empty() {
                    format!("\x1b[1;33m  [{}]\x1b[0m", name)
                } else {
                    format!("\x1b[1;33m  [{}: {}]\x1b[0m", name, input_desc)
                };
                println!("{}", tool_label);

                match registry.get(name) {
                    Some(tool) => {
                        let tool_spinner = Spinner::start(&format!("Running {}...", name));
                        match tool.execute(input.clone()).await {
                            Ok(result) => {
                                drop(tool_spinner);
                                println!("{}", format_tool_preview(&result.content, result.is_error));
                                let stored_content =
                                    truncate_utf8(&result.content, MAX_TOOL_RESULT_CHARS);
                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: stored_content,
                                    is_error: if result.is_error { Some(true) } else { None },
                                });
                            }
                            Err(e) => {
                                drop(tool_spinner);
                                println!("  \x1b[31m✗ Error: {}\x1b[0m", e);
                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: format!("Error: {}", e),
                                    is_error: Some(true),
                                });
                            }
                        }
                    }
                    None => {
                        println!("  \x1b[31m✗ Unknown tool '{}'\x1b[0m", name);
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

    #[test]
    fn test_format_tool_input_bash() {
        let input = serde_json::json!({"command": "ls -la"});
        assert_eq!(format_tool_input("bash", &input), "ls -la");
    }

    #[test]
    fn test_format_tool_input_grep() {
        let input = serde_json::json!({"pattern": "TODO", "path": "src/"});
        assert_eq!(format_tool_input("grep", &input), "/TODO/ in src/");
    }

    #[test]
    fn test_format_tool_preview_success() {
        let preview = format_tool_preview("line1\nline2\nline3", false);
        assert!(preview.contains("✓"));
        assert!(preview.contains("line1"));
    }

    #[test]
    fn test_format_tool_preview_error() {
        let preview = format_tool_preview("error msg", true);
        assert!(preview.contains("✗"));
    }

    #[test]
    fn test_format_tool_preview_truncates_lines() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let preview = format_tool_preview(content, false);
        assert!(preview.contains("..."));
        assert!(!preview.contains("line4"));
    }
}
