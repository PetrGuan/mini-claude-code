use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

use crate::tools::{Tool, ToolResult};

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents using ripgrep. Returns matching lines with file paths and line numbers."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in. Defaults to current directory."
                },
                "glob": {
                    "type": "string",
                    "description": "File glob filter (e.g. '*.rs')"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: Value) -> Result<ToolResult> {
        let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return Ok(ToolResult {
                    content: "Error: 'pattern' field is required".to_string(),
                    is_error: true,
                });
            }
        };

        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let mut cmd = Command::new("rg");
        cmd.arg("--no-heading")
            .arg("--line-number")
            .arg("--color=never")
            .arg("--max-count=50");

        if let Some(glob_pattern) = input.get("glob").and_then(|v| v.as_str()) {
            cmd.arg("--glob").arg(glob_pattern);
        }

        cmd.arg(pattern).arg(path);

        let output = cmd.output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        // rg exits with 1 when no matches found (not an error)
        if output.status.code() == Some(1) && stdout.is_empty() {
            return Ok(ToolResult {
                content: "No matches found".to_string(),
                is_error: false,
            });
        }

        if !output.status.success() && output.status.code() != Some(1) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult {
                content: format!("Error running rg: {}", stderr),
                is_error: true,
            });
        }

        Ok(ToolResult {
            content: stdout.to_string(),
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
    async fn test_grep_finds_matches() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "hello world\nfoo bar\n").unwrap();
        fs::write(dir.path().join("b.txt"), "hello rust\n").unwrap();

        let tool = GrepTool;
        let input = json!({"pattern": "hello", "path": dir.path().to_str().unwrap()});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "hello world\n").unwrap();

        let tool = GrepTool;
        let input = json!({"pattern": "zzzzz", "path": dir.path().to_str().unwrap()});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No matches"));
    }
}
