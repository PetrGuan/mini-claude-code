use anyhow::Result;
use async_trait::async_trait;
use globwalk::GlobWalkerBuilder;
use serde_json::{json, Value};

use crate::tools::{Tool, ToolResult};

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g. '**/*.rs')"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in. Defaults to current directory."
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

        let walker = match GlobWalkerBuilder::from_patterns(path, &[pattern])
            .max_depth(20)
            .build()
        {
            Ok(w) => w,
            Err(e) => {
                return Ok(ToolResult {
                    content: format!("Error building glob: {}", e),
                    is_error: true,
                });
            }
        };

        let mut files: Vec<String> = walker
            .filter_map(Result::ok)
            .map(|entry| entry.path().display().to_string())
            .collect();

        files.sort();

        if files.is_empty() {
            return Ok(ToolResult {
                content: "No files found matching pattern".to_string(),
                is_error: false,
            });
        }

        Ok(ToolResult {
            content: files.join("\n"),
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
    async fn test_glob_finds_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "").unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();
        fs::write(dir.path().join("c.rs"), "").unwrap();

        let tool = GlobTool;
        let input = json!({"pattern": "*.txt", "path": dir.path().to_str().unwrap()});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("a.txt"));
        assert!(result.content.contains("b.txt"));
        assert!(!result.content.contains("c.rs"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let dir = TempDir::new().unwrap();

        let tool = GlobTool;
        let input = json!({"pattern": "*.xyz", "path": dir.path().to_str().unwrap()});
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No files found"));
    }
}
