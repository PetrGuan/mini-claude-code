use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;

use crate::tools::{Tool, ToolResult};

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read a file from the filesystem. Returns contents with line numbers."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (0-based). Default 0."
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of lines to read. Default: all lines."
                }
            },
            "required": ["file_path"]
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

        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult {
                    content: format!("Error reading file: {}", e),
                    is_error: true,
                });
            }
        };

        let lines: Vec<&str> = content.lines().collect();
        let offset = input.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = input.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(lines.len());

        let numbered: Vec<String> = lines
            .iter()
            .enumerate()
            .skip(offset)
            .take(limit)
            .map(|(i, line)| format!("{}\t{}", i + 1, line))
            .collect();

        Ok(ToolResult {
            content: numbered.join("\n"),
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_read_file() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "line one").unwrap();
        writeln!(f, "line two").unwrap();

        let tool = ReadTool;
        let input = json!({"file_path": f.path().to_str().unwrap()});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("1\tline one"));
        assert!(result.content.contains("2\tline two"));
    }

    #[tokio::test]
    async fn test_read_nonexistent() {
        let tool = ReadTool;
        let input = json!({"file_path": "/tmp/nonexistent_mini_claude_test_file"});
        let result = tool.execute(input).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let mut f = NamedTempFile::new().unwrap();
        for i in 1..=10 {
            writeln!(f, "line {}", i).unwrap();
        }

        let tool = ReadTool;
        let input = json!({"file_path": f.path().to_str().unwrap(), "offset": 3, "limit": 2});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("4\tline 4"));
        assert!(result.content.contains("5\tline 5"));
        assert!(!result.content.contains("line 3\n"));
        assert!(!result.content.contains("line 6"));
    }
}
