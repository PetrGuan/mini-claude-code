# mini-claude-code Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a minimal, usable Claude Code CLI in Rust with interactive REPL, streaming Claude API, and 6 developer tools (bash, read, write, edit, glob, grep).

**Architecture:** A single-binary Rust CLI using tokio for async, reqwest for streaming HTTP, crossterm for terminal input, and termimad for markdown rendering. The tool system uses a trait-based registry mirroring the original's pattern. The REPL loop sends messages to the Claude API with tool definitions, streams text output in real-time, executes tool calls, and loops until the model stops.

**Tech Stack:** Rust, tokio, reqwest, serde/serde_json, clap, crossterm, termimad, syntect, globwalk

---

## File Structure

```
mini-claude-code/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point (clap), launches REPL
│   ├── api/
│   │   ├── mod.rs           # Re-exports
│   │   ├── types.rs         # API request/response serde types
│   │   ├── client.rs        # Anthropic API client (send message, stream)
│   │   └── stream.rs        # SSE line parser → typed events
│   ├── tools/
│   │   ├── mod.rs           # Tool trait, ToolRegistry, schema helpers
│   │   ├── bash.rs          # BashTool
│   │   ├── read.rs          # FileReadTool
│   │   ├── write.rs         # FileWriteTool
│   │   ├── edit.rs          # FileEditTool
│   │   ├── glob.rs          # GlobTool
│   │   └── grep.rs          # GrepTool
│   ├── ui/
│   │   ├── mod.rs           # Re-exports
│   │   ├── render.rs        # Markdown + syntax highlight rendering
│   │   └── input.rs         # User input (multiline, Ctrl+C handling)
│   └── repl.rs              # REPL loop: input → API → tool exec → output
└── tests/
    ├── tools/
    │   ├── bash_test.rs
    │   ├── read_test.rs
    │   ├── write_test.rs
    │   ├── edit_test.rs
    │   ├── glob_test.rs
    │   └── grep_test.rs
    └── api/
        ├── types_test.rs
        └── stream_test.rs
```

---

### Task 1: Project Scaffold + CLI Entry Point

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

- [ ] **Step 1: Initialize Cargo project**

```bash
cd /Users/petr/Documents/GitHub/mini-claude-code
cargo init --name mini-claude-code
```

- [ ] **Step 2: Set up Cargo.toml with dependencies**

Replace `Cargo.toml` with:

```toml
[package]
name = "mini-claude-code"
version = "0.1.0"
edition = "2021"
description = "A minimal Claude Code CLI in Rust"

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["stream", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
crossterm = "0.28"
termimad = "0.30"
syntect = "5"
globwalk = "0.9"
anyhow = "1"
futures-util = "0.3"
```

- [ ] **Step 3: Write main.rs with clap CLI parsing**

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "mini-claude-code", about = "A minimal Claude Code CLI")]
struct Cli {
    /// Model to use
    #[arg(short, long, default_value = "claude-sonnet-4-20250514")]
    model: String,

    /// Max tokens for response
    #[arg(long, default_value_t = 8192)]
    max_tokens: u32,

    /// Initial prompt (if omitted, starts interactive REPL)
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable must be set");

    println!("mini-claude-code v0.1.0");
    println!("Model: {}", cli.model);
    println!("(REPL not yet implemented)");

    Ok(())
}
```

- [ ] **Step 4: Verify it compiles and runs**

```bash
cd /Users/petr/Documents/GitHub/mini-claude-code
cargo build
cargo run -- --help
```

Expected: Help text showing `--model`, `--max-tokens`, and `[PROMPT]` arguments.

- [ ] **Step 5: Commit**

```bash
git init
echo "target/" > .gitignore
git add Cargo.toml src/main.rs .gitignore
git commit -m "feat: project scaffold with CLI entry point"
```

---

### Task 2: API Types

**Files:**
- Create: `src/api/mod.rs`
- Create: `src/api/types.rs`
- Create: `tests/api/types_test.rs`

- [ ] **Step 1: Create api module**

`src/api/mod.rs`:

```rust
pub mod types;
pub mod client;
pub mod stream;
```

Register module in `src/main.rs` by adding at the top:

```rust
mod api;
mod tools;
mod ui;
mod repl;
```

(Create placeholder files for `src/tools/mod.rs`, `src/ui/mod.rs`, `src/repl.rs` so it compiles.)

`src/tools/mod.rs`:

```rust
// placeholder
```

`src/ui/mod.rs`:

```rust
// placeholder
```

`src/repl.rs`:

```rust
// placeholder
```

- [ ] **Step 2: Write the API types**

`src/api/types.rs`:

```rust
use serde::{Deserialize, Serialize};

// === Request Types ===

#[derive(Debug, Serialize)]
pub struct CreateMessageRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// === Streaming Event Types ===

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageStartData },

    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStartData,
    },

    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: usize,
        delta: DeltaData,
    },

    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },

    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDeltaData },

    #[serde(rename = "message_stop")]
    MessageStop,

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "error")]
    Error { error: ApiError },
}

#[derive(Debug, Deserialize)]
pub struct MessageStartData {
    pub id: String,
    pub model: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockStartData {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum DeltaData {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },

    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
pub struct MessageDeltaData {
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}
```

- [ ] **Step 3: Write test for serialization/deserialization**

Create `tests/api/types_test.rs`:

```rust
use mini_claude_code::api::types::*;

#[test]
fn test_serialize_create_message_request() {
    let req = CreateMessageRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        max_tokens: 8192,
        system: Some("You are a helpful assistant.".to_string()),
        messages: vec![Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "Hello".to_string(),
            }],
        }],
        tools: vec![],
        stream: true,
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["model"], "claude-sonnet-4-20250514");
    assert_eq!(json["messages"][0]["role"], "user");
    assert_eq!(json["messages"][0]["content"][0]["type"], "text");
    assert_eq!(json["stream"], true);
}

#[test]
fn test_deserialize_text_delta_event() {
    let json = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
    let event: StreamEvent = serde_json::from_str(json).unwrap();
    match event {
        StreamEvent::ContentBlockDelta { index, delta } => {
            assert_eq!(index, 0);
            match delta {
                DeltaData::TextDelta { text } => assert_eq!(text, "Hello"),
                _ => panic!("Expected TextDelta"),
            }
        }
        _ => panic!("Expected ContentBlockDelta"),
    }
}

#[test]
fn test_deserialize_tool_use_start() {
    let json = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_123","name":"bash"}}"#;
    let event: StreamEvent = serde_json::from_str(json).unwrap();
    match event {
        StreamEvent::ContentBlockStart { index, content_block } => {
            assert_eq!(index, 1);
            match content_block {
                ContentBlockStartData::ToolUse { id, name } => {
                    assert_eq!(id, "toolu_123");
                    assert_eq!(name, "bash");
                }
                _ => panic!("Expected ToolUse"),
            }
        }
        _ => panic!("Expected ContentBlockStart"),
    }
}

#[test]
fn test_deserialize_message_stop() {
    let json = r#"{"type":"message_stop"}"#;
    let event: StreamEvent = serde_json::from_str(json).unwrap();
    assert!(matches!(event, StreamEvent::MessageStop));
}

#[test]
fn test_tool_result_serialization() {
    let block = ContentBlock::ToolResult {
        tool_use_id: "toolu_123".to_string(),
        content: "file contents here".to_string(),
        is_error: None,
    };
    let json = serde_json::to_value(&block).unwrap();
    assert_eq!(json["type"], "tool_result");
    assert_eq!(json["tool_use_id"], "toolu_123");
    assert!(json.get("is_error").is_none());
}
```

To make tests work, expose modules as a library. Create `src/lib.rs`:

```rust
pub mod api;
pub mod tools;
pub mod ui;
```

- [ ] **Step 4: Run tests**

```bash
cargo test --test api/types_test
```

If Rust complains about test module paths, use this structure instead — create `tests/api_types_test.rs` (flat file):

```bash
cargo test api_types
```

Expected: All 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/api/ src/lib.rs src/tools/mod.rs src/ui/mod.rs src/repl.rs src/main.rs tests/
git commit -m "feat: API request/response types with serde"
```

---

### Task 3: SSE Stream Parser

**Files:**
- Create: `src/api/stream.rs`
- Create: `tests/api/stream_test.rs`

- [ ] **Step 1: Write the SSE stream parser**

`src/api/stream.rs`:

```rust
use crate::api::types::StreamEvent;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::Response;
use tokio::sync::mpsc;

/// Parsed SSE event from the Anthropic API stream
pub enum SseEvent {
    /// A successfully parsed StreamEvent
    Event(StreamEvent),
    /// Stream has ended
    Done,
}

/// Parse an SSE stream from an HTTP response into a channel of events.
/// Each SSE message has the format:
///   event: <event_type>\n
///   data: <json>\n\n
pub async fn parse_sse_stream(
    response: Response,
    tx: mpsc::UnboundedSender<Result<SseEvent>>,
) {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));

                // Process complete SSE messages (separated by double newline)
                while let Some(pos) = buffer.find("\n\n") {
                    let message = buffer[..pos].to_string();
                    buffer = buffer[pos + 2..].to_string();

                    if let Some(event) = parse_sse_message(&message) {
                        match event {
                            Ok(sse_event) => {
                                if tx.send(Ok(sse_event)).is_err() {
                                    return; // receiver dropped
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Err(e));
                                return;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(Err(anyhow!("Stream error: {}", e)));
                return;
            }
        }
    }

    let _ = tx.send(Ok(SseEvent::Done));
}

fn parse_sse_message(message: &str) -> Option<Result<SseEvent>> {
    let mut data_line = None;

    for line in message.lines() {
        if let Some(value) = line.strip_prefix("data: ") {
            data_line = Some(value);
        }
    }

    let data = data_line?;

    // SSE spec: "data: [DONE]" signals end of stream (some APIs use this)
    if data == "[DONE]" {
        return Some(Ok(SseEvent::Done));
    }

    match serde_json::from_str::<StreamEvent>(data) {
        Ok(event) => Some(Ok(SseEvent::Event(event))),
        Err(e) => Some(Err(anyhow!("Failed to parse SSE data: {} — raw: {}", e, data))),
    }
}
```

- [ ] **Step 2: Write tests for SSE parsing**

Create `tests/stream_test.rs`:

```rust
use mini_claude_code::api::stream::parse_sse_message;

// Since parse_sse_message is private, we test through the public API.
// Instead, create a unit test inside stream.rs:

// Add to the bottom of src/api/stream.rs:
```

Add this to the bottom of `src/api/stream.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_delta() {
        let message = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}";
        let result = parse_sse_message(message).unwrap().unwrap();
        match result {
            SseEvent::Event(StreamEvent::ContentBlockDelta { index, delta }) => {
                assert_eq!(index, 0);
            }
            _ => panic!("Expected ContentBlockDelta event"),
        }
    }

    #[test]
    fn test_parse_message_stop() {
        let message = "event: message_stop\ndata: {\"type\":\"message_stop\"}";
        let result = parse_sse_message(message).unwrap().unwrap();
        assert!(matches!(result, SseEvent::Event(StreamEvent::MessageStop)));
    }

    #[test]
    fn test_parse_done_signal() {
        let message = "data: [DONE]";
        let result = parse_sse_message(message).unwrap().unwrap();
        assert!(matches!(result, SseEvent::Done));
    }

    #[test]
    fn test_skip_empty_lines() {
        let result = parse_sse_message(": ping");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_ping() {
        let message = "event: ping\ndata: {\"type\":\"ping\"}";
        let result = parse_sse_message(message).unwrap().unwrap();
        assert!(matches!(result, SseEvent::Event(StreamEvent::Ping)));
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test stream::tests
```

Expected: All 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/api/stream.rs
git commit -m "feat: SSE stream parser for Anthropic API"
```

---

### Task 4: API Client

**Files:**
- Create: `src/api/client.rs`

- [ ] **Step 1: Write the API client**

`src/api/client.rs`:

```rust
use crate::api::stream::{parse_sse_stream, SseEvent};
use crate::api::types::{CreateMessageRequest, Message, ToolDefinition};
use anyhow::{anyhow, Result};
use reqwest::Client;
use tokio::sync::mpsc;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub system_prompt: Option<String>,
}

impl AnthropicClient {
    pub fn new(api_key: String, model: String, max_tokens: u32) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            max_tokens,
            system_prompt: None,
        }
    }

    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
    }

    /// Send a streaming message request. Returns a channel that yields SSE events.
    pub async fn send_message_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<mpsc::UnboundedReceiver<Result<SseEvent>>> {
        let request = CreateMessageRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system: self.system_prompt.clone(),
            messages: messages.to_vec(),
            tools: tools.to_vec(),
            stream: true,
        };

        let response = self
            .client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error ({}): {}", status, body));
        }

        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(parse_sse_stream(response, tx));

        Ok(rx)
    }
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build
```

Expected: Compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add src/api/client.rs
git commit -m "feat: Anthropic API client with streaming support"
```

---

### Task 5: Tool Trait + Registry

**Files:**
- Create: `src/tools/mod.rs`

- [ ] **Step 1: Define the Tool trait and ToolRegistry**

`src/tools/mod.rs`:

```rust
pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod read;
pub mod write;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::api::types::ToolDefinition;

pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, input: Value) -> Result<ToolResult>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Generate tool definitions for the API request
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.input_schema(),
            })
            .collect()
    }
}

/// Create a registry with all built-in tools
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(bash::BashTool));
    registry.register(Box::new(read::ReadTool));
    registry.register(Box::new(write::WriteTool));
    registry.register(Box::new(edit::EditTool));
    registry.register(Box::new(glob::GlobTool));
    registry.register(Box::new(grep::GrepTool));
    registry
}
```

Add `async-trait` to `Cargo.toml`:

```toml
async-trait = "0.1"
```

- [ ] **Step 2: Verify it compiles** (will need placeholder tool files first — create them in next steps, or create empty stubs)

Create empty stubs for each tool so it compiles. Each file (`src/tools/bash.rs`, `read.rs`, `write.rs`, `edit.rs`, `glob.rs`, `grep.rs`) gets:

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use crate::tools::{Tool, ToolResult};

pub struct BashTool;  // (change name per file)

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str { "bash" }
    fn description(&self) -> &str { "TODO" }
    fn input_schema(&self) -> Value { json!({}) }
    async fn execute(&self, _input: Value) -> Result<ToolResult> {
        Ok(ToolResult { content: "not implemented".into(), is_error: true })
    }
}
```

(Use `ReadTool` / `read` for read.rs, `WriteTool` / `write` for write.rs, etc.)

```bash
cargo build
```

- [ ] **Step 3: Commit**

```bash
git add src/tools/ Cargo.toml
git commit -m "feat: Tool trait, registry, and tool stubs"
```

---

### Task 6: BashTool

**Files:**
- Modify: `src/tools/bash.rs`
- Create: `tests/tools/bash_test.rs`

- [ ] **Step 1: Write the BashTool test**

Add to bottom of `src/tools/bash.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test bash::tests
```

Expected: FAIL — stubs return "not implemented".

- [ ] **Step 3: Implement BashTool**

`src/tools/bash.rs`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

use crate::tools::{Tool, ToolResult};

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

        let output = Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()
            .await?;

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
```

- [ ] **Step 4: Run tests**

```bash
cargo test bash::tests
```

Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/bash.rs
git commit -m "feat: BashTool implementation"
```

---

### Task 7: FileReadTool

**Files:**
- Modify: `src/tools/read.rs`

- [ ] **Step 1: Write tests**

Add to bottom of `src/tools/read.rs`:

```rust
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
        assert!(!result.content.contains("line 3"));
        assert!(!result.content.contains("line 6"));
    }
}
```

Add `tempfile` to `Cargo.toml` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test read::tests
```

Expected: FAIL.

- [ ] **Step 3: Implement ReadTool**

`src/tools/read.rs`:

```rust
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
        let offset = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(lines.len());

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
```

- [ ] **Step 4: Run tests**

```bash
cargo test read::tests
```

Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/read.rs Cargo.toml
git commit -m "feat: FileReadTool with line numbers, offset, limit"
```

---

### Task 8: FileWriteTool

**Files:**
- Modify: `src/tools/write.rs`

- [ ] **Step 1: Write tests**

Add to bottom of `src/tools/write.rs`:

```rust
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
        let input = json!({
            "file_path": path.to_str().unwrap(),
            "content": "hello world"
        });
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
        let input = json!({
            "file_path": path.to_str().unwrap(),
            "content": "new"
        });
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a/b/c/test.txt");

        let tool = WriteTool;
        let input = json!({
            "file_path": path.to_str().unwrap(),
            "content": "nested"
        });
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(fs::read_to_string(&path).unwrap(), "nested");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test write::tests
```

- [ ] **Step 3: Implement WriteTool**

`src/tools/write.rs`:

```rust
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
                fs::create_dir_all(parent).map_err(|e| {
                    anyhow::anyhow!("Failed to create directories: {}", e)
                })?;
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
```

- [ ] **Step 4: Run tests**

```bash
cargo test write::tests
```

Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/write.rs
git commit -m "feat: FileWriteTool with parent directory creation"
```

---

### Task 9: FileEditTool

**Files:**
- Modify: `src/tools/edit.rs`

- [ ] **Step 1: Write tests**

Add to bottom of `src/tools/edit.rs`:

```rust
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
        let input = json!({
            "file_path": path.to_str().unwrap(),
            "old_string": "foo bar",
            "new_string": "baz qux"
        });
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
        let input = json!({
            "file_path": path.to_str().unwrap(),
            "old_string": "not here",
            "new_string": "replacement"
        });
        let result = tool.execute(input).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_edit_multiple_matches_fails() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "aaa\naaa\n").unwrap();

        let tool = EditTool;
        let input = json!({
            "file_path": path.to_str().unwrap(),
            "old_string": "aaa",
            "new_string": "bbb"
        });
        let result = tool.execute(input).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("multiple"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test edit::tests
```

- [ ] **Step 3: Implement EditTool**

`src/tools/edit.rs`:

```rust
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
                content: format!(
                    "Error: old_string found multiple times ({} matches). Provide a more unique string.",
                    count
                ),
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
```

- [ ] **Step 4: Run tests**

```bash
cargo test edit::tests
```

Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/edit.rs
git commit -m "feat: FileEditTool with unique string replacement"
```

---

### Task 10: GlobTool

**Files:**
- Modify: `src/tools/glob.rs`

- [ ] **Step 1: Write tests**

Add to bottom of `src/tools/glob.rs`:

```rust
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
        let input = json!({
            "pattern": "*.txt",
            "path": dir.path().to_str().unwrap()
        });
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
        let input = json!({
            "pattern": "*.xyz",
            "path": dir.path().to_str().unwrap()
        });
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No files found"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test glob::tests
```

- [ ] **Step 3: Implement GlobTool**

`src/tools/glob.rs`:

```rust
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

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

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
```

- [ ] **Step 4: Run tests**

```bash
cargo test glob::tests
```

Expected: All 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/glob.rs
git commit -m "feat: GlobTool for file pattern matching"
```

---

### Task 11: GrepTool

**Files:**
- Modify: `src/tools/grep.rs`

- [ ] **Step 1: Write tests**

Add to bottom of `src/tools/grep.rs`:

```rust
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
        let input = json!({
            "pattern": "hello",
            "path": dir.path().to_str().unwrap()
        });
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "hello world\n").unwrap();

        let tool = GrepTool;
        let input = json!({
            "pattern": "zzzzz",
            "path": dir.path().to_str().unwrap()
        });
        let result = tool.execute(input).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("No matches"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test grep::tests
```

- [ ] **Step 3: Implement GrepTool**

`src/tools/grep.rs`:

```rust
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

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

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
```

- [ ] **Step 4: Run tests**

```bash
cargo test grep::tests
```

Expected: All 2 tests pass (requires `rg` installed on system).

- [ ] **Step 5: Commit**

```bash
git add src/tools/grep.rs
git commit -m "feat: GrepTool using ripgrep"
```

---

### Task 12: Markdown Rendering

**Files:**
- Create: `src/ui/render.rs`
- Create: `src/ui/mod.rs`

- [ ] **Step 1: Implement markdown renderer**

`src/ui/mod.rs`:

```rust
pub mod render;
pub mod input;
```

`src/ui/input.rs` (placeholder):

```rust
// placeholder — implemented in Task 13
```

`src/ui/render.rs`:

```rust
use termimad::MadSkin;

pub fn create_skin() -> MadSkin {
    let mut skin = MadSkin::default();
    // Use terminal defaults — termimad handles code blocks,
    // bold, italic, headers, lists out of the box
    skin
}

pub fn render_markdown(text: &str, skin: &MadSkin) {
    skin.print_text(text);
}

/// Print a chunk of streaming text directly (no markdown processing).
/// Used during streaming to print text as it arrives.
pub fn print_stream_chunk(text: &str) {
    use std::io::{self, Write};
    print!("{}", text);
    io::stdout().flush().ok();
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build
```

- [ ] **Step 3: Commit**

```bash
git add src/ui/
git commit -m "feat: markdown rendering with termimad"
```

---

### Task 13: User Input

**Files:**
- Modify: `src/ui/input.rs`

- [ ] **Step 1: Implement user input handling**

`src/ui/input.rs`:

```rust
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use std::io::{self, Write};

/// Read user input from the terminal.
/// Supports multiline: press Enter twice to submit, Ctrl+C to exit.
pub fn read_user_input() -> Option<String> {
    print!("\x1b[1;34m> \x1b[0m");
    io::stdout().flush().ok();

    let mut input = String::new();
    let mut consecutive_newlines = 0;

    terminal::enable_raw_mode().ok()?;

    loop {
        if let Ok(Event::Key(key_event)) = event::read() {
            match key_event {
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    terminal::disable_raw_mode().ok();
                    println!();
                    return None; // signal exit
                }
                KeyEvent {
                    code: KeyCode::Char('d'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } if input.is_empty() => {
                    terminal::disable_raw_mode().ok();
                    println!();
                    return None; // signal exit on empty input
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    consecutive_newlines += 1;
                    if consecutive_newlines >= 2 {
                        terminal::disable_raw_mode().ok();
                        println!();
                        let trimmed = input.trim().to_string();
                        if trimmed.is_empty() {
                            return Some(String::new());
                        }
                        return Some(trimmed);
                    }
                    input.push('\n');
                    print!("\r\n\x1b[1;34m. \x1b[0m");
                    io::stdout().flush().ok();
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    if input.ends_with('\n') {
                        input.pop();
                        consecutive_newlines = 0;
                        // Move cursor up and clear line
                        print!("\x1b[A\x1b[999C");
                        io::stdout().flush().ok();
                    } else if !input.is_empty() {
                        input.pop();
                        print!("\x1b[D \x1b[D");
                        io::stdout().flush().ok();
                    }
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    ..
                } => {
                    consecutive_newlines = 0;
                    input.push(c);
                    print!("{}", c);
                    io::stdout().flush().ok();
                }
                _ => {}
            }
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build
```

- [ ] **Step 3: Commit**

```bash
git add src/ui/input.rs
git commit -m "feat: terminal user input with multiline support"
```

---

### Task 14: REPL Loop + System Prompt

**Files:**
- Create: `src/repl.rs`

- [ ] **Step 1: Implement the REPL**

`src/repl.rs`:

```rust
use crate::api::client::AnthropicClient;
use crate::api::stream::SseEvent;
use crate::api::types::{ContentBlock, ContentBlockStartData, DeltaData, Message, Role, StreamEvent};
use crate::tools::ToolRegistry;
use crate::ui::input::read_user_input;
use crate::ui::render::{create_skin, print_stream_chunk, render_markdown};
use anyhow::Result;

pub async fn run(client: &AnthropicClient, registry: &ToolRegistry) -> Result<()> {
    let skin = create_skin();
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

        // Add user message
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
            let mut stop_reason: Option<String> = None;

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
                                    // Flush any accumulated text
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
                                // Parse accumulated tool input and add to content
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
                        StreamEvent::MessageDelta { delta } => {
                            stop_reason = delta.stop_reason;
                        }
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

            // Add assistant message to history
            messages.push(Message {
                role: Role::Assistant,
                content: assistant_content.clone(),
            });

            // Execute any tool calls
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
                break; // No tool calls — done with this turn
            }

            // Execute tools and collect results
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

            // Add tool results as user message
            messages.push(Message {
                role: Role::User,
                content: tool_results,
            });

            // Loop back to call API again with tool results
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Wire up main.rs**

Update `src/main.rs`:

```rust
mod api;
mod repl;
mod tools;
mod ui;

use clap::Parser;

#[derive(Parser)]
#[command(name = "mini-claude-code", about = "A minimal Claude Code CLI")]
struct Cli {
    /// Model to use
    #[arg(short, long, default_value = "claude-sonnet-4-20250514")]
    model: String,

    /// Max tokens for response
    #[arg(long, default_value_t = 8192)]
    max_tokens: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable must be set");

    let cwd = std::env::current_dir()?.display().to_string();

    let mut client = api::client::AnthropicClient::new(api_key, cli.model, cli.max_tokens);
    client.set_system_prompt(format!(
        "You are a helpful coding assistant running in the terminal.\n\
         Working directory: {}\n\
         You have access to tools for running bash commands, reading/writing/editing files, \
         and searching with glob patterns and grep.\n\
         When using tools, always use absolute paths.\n\
         Be concise in your responses.",
        cwd
    ));

    let registry = tools::create_default_registry();

    repl::run(&client, &registry).await
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo build
```

- [ ] **Step 4: Commit**

```bash
git add src/repl.rs src/main.rs
git commit -m "feat: REPL loop with streaming output and tool execution"
```

---

### Task 15: Integration Test — End to End

**Files:**
- (No new files — manual test)

- [ ] **Step 1: Build release binary**

```bash
cd /Users/petr/Documents/GitHub/mini-claude-code
cargo build --release
```

- [ ] **Step 2: Run manual smoke test**

```bash
ANTHROPIC_API_KEY=<your-key> cargo run
```

Test these interactions:
1. "What files are in the current directory?" — should trigger glob or bash tool
2. "Read the Cargo.toml file" — should trigger read tool
3. "Create a file called /tmp/mini-test.txt with the text 'hello from mini-claude-code'" — should trigger write tool
4. "Search for 'tokio' in the src directory" — should trigger grep tool

- [ ] **Step 3: Fix any issues found**

Address compilation errors or runtime bugs.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "chore: fix issues from integration testing"
```

- [ ] **Step 5: Tag v0.1.0**

```bash
git tag v0.1.0
```
