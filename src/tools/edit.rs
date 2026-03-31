use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;

use crate::tools::{Tool, ToolResult};

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing an exact string match with new content. The old_string must appear exactly once in the file."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace (must be unique in the file)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return Ok(ToolResult {
                    content: "Error: 'file_path' field is required".to_string(),
                    is_error: true,
                });
            }
        };
        let old_string = match input.get("old_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return Ok(ToolResult {
                    content: "Error: 'old_string' field is required".to_string(),
                    is_error: true,
                });
            }
        };
        let new_string = match input.get("new_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return Ok(ToolResult {
                    content: "Error: 'new_string' field is required".to_string(),
                    is_error: true,
                });
            }
        };

        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult {
                    content: format!("Error reading file: {}", e),
                    is_error: true,
                });
            }
        };

        let count = content.matches(old_string).count();
        if count == 0 {
            return Ok(ToolResult {
                content: "Error: old_string not found in file".to_string(),
                is_error: true,
            });
        }
        if count > 1 {
            return Ok(ToolResult {
                content: format!("Error: old_string found multiple times ({} matches). Provide a more unique string.", count),
                is_error: true,
            });
        }

        let new_content = content.replacen(old_string, new_string, 1);
        fs::write(file_path, &new_content)?;

        Ok(ToolResult {
            content: format!("Successfully edited {}", file_path),
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_edit_replace() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello world\nfoo bar\n").unwrap();

        let tool = EditTool;
        let input = json!({"file_path": path.to_str().unwrap(), "old_string": "foo bar", "new_string": "baz qux"});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello world\nbaz qux\n");
    }

    #[tokio::test]
    async fn test_edit_string_not_found() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello world\n").unwrap();

        let tool = EditTool;
        let input = json!({"file_path": path.to_str().unwrap(), "old_string": "not here", "new_string": "replacement"});
        let result = tool.execute(input).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_edit_multiple_matches_fails() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "aaa\naaa\n").unwrap();

        let tool = EditTool;
        let input = json!({"file_path": path.to_str().unwrap(), "old_string": "aaa", "new_string": "bbb"});
        let result = tool.execute(input).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("multiple"));
    }
}
