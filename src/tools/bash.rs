use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;

use crate::tools::{Tool, ToolResult};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command and return its output."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let command = match input.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Ok(ToolResult {
                    content: "Error: 'command' field is required".to_string(),
                    is_error: true,
                });
            }
        };

        let output = match tokio::time::timeout(
            COMMAND_TIMEOUT,
            Command::new("bash").arg("-c").arg(command).output(),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                return Ok(ToolResult {
                    content: format!(
                        "Error: command timed out after {} seconds",
                        COMMAND_TIMEOUT.as_secs()
                    ),
                    is_error: true,
                });
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let content = if stderr.is_empty() {
            stdout.to_string()
        } else if stdout.is_empty() {
            stderr.to_string()
        } else {
            format!("{}\n{}", stdout, stderr)
        };

        Ok(ToolResult {
            content,
            is_error: !output.status.success(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo() {
        let tool = BashTool;
        let input = json!({"command": "echo hello"});
        let result = tool.execute(input).await.unwrap();
        assert_eq!(result.content.trim(), "hello");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_failing_command() {
        let tool = BashTool;
        let input = json!({"command": "false"});
        let result = tool.execute(input).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_missing_command_field() {
        let tool = BashTool;
        let input = json!({});
        let result = tool.execute(input).await.unwrap();
        assert!(result.is_error);
    }
}
