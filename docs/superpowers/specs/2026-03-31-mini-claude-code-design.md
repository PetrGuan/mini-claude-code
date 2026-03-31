# mini-claude-code Design Spec

## Overview

A minimal, practical Claude Code CLI tool built from scratch in Rust. Inspired by Anthropic's Claude Code, stripped down to the essential core: interactive conversation with Claude plus a small set of developer tools (Bash, file read/write/edit, glob, grep).

Goals:
- Learn Claude Code's architecture by reimplementing its core in Rust
- Produce a fast, usable CLI tool for daily development tasks
- Keep the codebase small and understandable (~2-5K lines)

## Core Features

1. **Interactive REPL** — terminal-based conversation loop with streaming output
2. **Bash tool** — execute shell commands, return stdout/stderr
3. **File read** — read file contents (with line numbers)
4. **File write** — create or overwrite files
5. **File edit** — partial file modification via string replacement
6. **Glob search** — find files by pattern
7. **Grep search** — search file contents (delegates to system `rg`)

## Tech Stack

| Component | Choice | Reason |
|-----------|--------|--------|
| Language | Rust | Performance, single binary distribution |
| Async runtime | `tokio` | Rust async standard |
| HTTP client | `reqwest` | Streaming SSE support |
| Terminal UI | `crossterm` + `ratatui` | Mature, cross-platform |
| JSON | `serde` + `serde_json` | Rust standard |
| CLI parsing | `clap` | Rust CLI standard |
| Markdown rendering | `termimad` | Lightweight terminal markdown |
| Syntax highlighting | `syntect` | Code block highlighting |
| File search | `globwalk` | Glob pattern matching |
| Content search | System `rg` (ripgrep) | Same as original, no reinvention |

## Architecture

```
mini-claude-code/
├── src/
│   ├── main.rs              # Entry point, CLI arg parsing (clap)
│   ├── repl.rs              # REPL main loop
│   ├── api/
│   │   ├── mod.rs           # Module exports
│   │   ├── client.rs        # Anthropic API client
│   │   ├── types.rs         # Request/response types (serde)
│   │   └── stream.rs        # SSE stream parser
│   ├── tools/
│   │   ├── mod.rs           # Tool trait + registry
│   │   ├── bash.rs          # BashTool — shell command execution
│   │   ├── read.rs          # FileReadTool — read files with line numbers
│   │   ├── write.rs         # FileWriteTool — create/overwrite files
│   │   ├── edit.rs          # FileEditTool — string replacement editing
│   │   ├── glob.rs          # GlobTool — file pattern search
│   │   └── grep.rs          # GrepTool — content search via rg
│   └── ui/
│       ├── mod.rs           # Module exports
│       ├── render.rs        # Markdown rendering + syntax highlighting
│       └── input.rs         # User input handling (multiline, history)
├── Cargo.toml
└── README.md
```

## Key Design Decisions

### 1. Tool System

Define a `Tool` trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult>;
}
```

Each tool is a struct implementing this trait. Tools are registered in a `ToolRegistry` (a `HashMap<String, Box<dyn Tool>>`). Tool definitions are sent to the Claude API as part of the request, and tool calls from the API response are dispatched through the registry.

### 2. API Interaction Flow

```
User input
  → Build messages array (conversation history)
  → POST /v1/messages (stream: true) with tools
  → Parse SSE events:
      - content_block_delta (text) → render to terminal in real-time
      - content_block_delta (tool_use input) → accumulate JSON
      - content_block_stop (tool_use) → execute tool
      - message_stop → done
  → If tool was called:
      - Append assistant message + tool_result to history
      - Loop back to API call
  → If no tool call (stop_reason: end_turn):
      - Wait for next user input
```

### 3. Streaming Output

Use SSE (Server-Sent Events) from the Anthropic Messages API. Parse the stream line by line:
- `event: content_block_delta` with `type: text_delta` → print text chunk immediately
- `event: content_block_delta` with `type: input_json_delta` → buffer tool input JSON
- `event: message_stop` → end of response

### 4. Conversation History

Maintain a `Vec<Message>` in memory. Each message has a `role` (user/assistant) and `content` (text blocks + tool_use/tool_result blocks). No persistence for v1 — history lives only for the session.

### 5. User Input

Support multiline input. Simple approach: read lines until a blank line or Enter on a non-empty line. Use `crossterm` for raw terminal control and key event handling.

### 6. Error Handling

- API errors (rate limit, auth) → display error, let user retry
- Tool execution errors → return error as tool_result to Claude, let it adapt
- Network errors → display and retry with backoff

## LLM API Details

- **Endpoint**: `https://api.anthropic.com/v1/messages`
- **Auth**: `x-api-key` header, read from `ANTHROPIC_API_KEY` env var
- **Model**: Default `claude-sonnet-4-20250514`, configurable via CLI flag
- **Max tokens**: 8192 default, configurable
- **System prompt**: Minimal prompt describing available tools and working directory context

## Non-Goals (for v1)

- No permission system / user approval flow
- No sub-agent spawning
- No MCP support
- No plugin/skill system
- No IDE bridge
- No persistent memory
- No conversation history persistence
- No cost tracking
- No slash commands
- No voice input
- No vim mode
