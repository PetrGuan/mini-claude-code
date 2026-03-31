use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

use crate::tools::{Tool, ToolResult};

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, overwrites if it does."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
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
        let content = match input.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return Ok(ToolResult {
                    content: "Error: 'content' field is required".to_string(),
                    is_error: true,
                });
            }
        };

        if let Some(parent) = Path::new(file_path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| anyhow::anyhow!("Failed to create directories: {}", e))?;
            }
        }

        match fs::write(file_path, content) {
            Ok(_) => Ok(ToolResult {
                content: format!("Successfully wrote to {}", file_path),
                is_error: false,
            }),
            Err(e) => Ok(ToolResult {
                content: format!("Error writing file: {}", e),
                is_error: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_write_new_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        let tool = WriteTool;
        let input = json!({"file_path": path.to_str().unwrap(), "content": "hello world"});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
    }

    #[tokio::test]
    async fn test_write_overwrites() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "old").unwrap();

        let tool = WriteTool;
        let input = json!({"file_path": path.to_str().unwrap(), "content": "new"});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a/b/c/test.txt");

        let tool = WriteTool;
        let input = json!({"file_path": path.to_str().unwrap(), "content": "nested"});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(fs::read_to_string(&path).unwrap(), "nested");
    }
}
