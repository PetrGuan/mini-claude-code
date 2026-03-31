# mini-claude-code

A minimal [Claude Code](https://docs.anthropic.com/en/docs/claude-code) CLI rebuilt from scratch in Rust. Interactive AI coding assistant in your terminal — chat with Claude, run commands, read/write/edit files, and search your codebase.

Built as a learning project by studying the [leaked Claude Code source](https://github.com/PetrGuan/claude-code), reimplementing the core in ~1,400 lines of Rust.

## Demo

```
$ mini-claude-code

mini-claude-code v0.1.0
Model: claude-haiku-4-5-20251001
Type your message (press Enter twice to send, Ctrl+C to exit)

> What files are in the src directory?

[Tool: glob]
  Executing glob...

Here are the files in src/:
- src/main.rs — CLI entry point
- src/repl.rs — REPL loop
- src/auth.rs — Authentication (OAuth + API key)
- src/api/ — Anthropic API client + SSE streaming
- src/tools/ — 6 developer tools
- src/ui/ — Terminal input + rendering
```

## Features

- **Interactive REPL** with streaming output
- **6 built-in tools** that Claude can invoke:

  | Tool | Description |
  |------|-------------|
  | `bash` | Execute shell commands (120s timeout) |
  | `read` | Read files with line numbers, offset, limit |
  | `write` | Create/overwrite files (auto-creates parent dirs) |
  | `edit` | Find-and-replace editing (unique match required) |
  | `glob` | Find files by pattern (`**/*.rs`) |
  | `grep` | Search file contents via ripgrep |

- **Smart authentication** — auto-detects API key from env, macOS Keychain, or OAuth login
- **Streaming** — text appears in real-time as Claude generates it
- **Tool loop** — Claude can chain multiple tool calls before responding
- **Single binary** — 5.8 MB release build, no runtime dependencies

## Install

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (1.70+)
- [ripgrep](https://github.com/BurntSushi/ripgrep) (`brew install ripgrep`)

### Build from source

```bash
git clone https://github.com/PetrGuan/mini-claude-code.git
cd mini-claude-code
cargo build --release
```

The binary is at `target/release/mini-claude-code`.

## Usage

```bash
# With API key
ANTHROPIC_API_KEY=sk-ant-xxx ./target/release/mini-claude-code

# Or if you've logged into Claude Code before (reads key from macOS Keychain)
./target/release/mini-claude-code

# Or it will open a browser for OAuth login on first run
./target/release/mini-claude-code
```

### Options

```
Usage: mini-claude-code [OPTIONS]

Options:
  -m, --model <MODEL>            Model to use [default: claude-haiku-4-5-20251001]
      --max-tokens <MAX_TOKENS>  Max tokens for response [default: 8192]
  -h, --help                     Print help
```

### Authentication priority

1. `ANTHROPIC_API_KEY` environment variable
2. macOS Keychain — `mini-claude-code` cached key
3. macOS Keychain — `Claude Code` managed key (if you've used Claude Code)
4. Interactive OAuth login (opens browser, creates and caches API key)

### Input

- Type your message, press **Enter twice** to send
- **Ctrl+C** to exit

## Architecture

```
src/
├── main.rs              # CLI entry (clap), launches REPL
├── auth.rs              # Multi-strategy auth (env → keychain → OAuth PKCE)
├── repl.rs              # REPL loop: input → API → tool execution → output
├── api/
│   ├── client.rs        # Anthropic API client (streaming)
│   ├── types.rs         # Request/response serde types
│   └── stream.rs        # SSE stream parser
├── tools/
│   ├── mod.rs           # Tool trait + registry
│   ├── bash.rs          # BashTool (with timeout)
│   ├── read.rs          # FileReadTool
│   ├── write.rs         # FileWriteTool
│   ├── edit.rs          # FileEditTool
│   ├── glob.rs          # GlobTool
│   └── grep.rs          # GrepTool (via ripgrep)
└── ui/
    ├── input.rs         # Terminal input (raw mode + RAII guard)
    └── render.rs        # Stream output
```

### How it works

```
User input → Build messages → POST /v1/messages (stream: true)
  → Stream text to terminal in real-time
  → If tool_use → execute tool → send result back → loop
  → If end_turn → wait for next input
```

## Tech Stack

| Component | Choice |
|-----------|--------|
| Language | Rust |
| Async runtime | tokio |
| HTTP | reqwest (streaming SSE) |
| JSON | serde + serde_json |
| CLI | clap |
| Terminal | crossterm |
| File search | globwalk + ripgrep |

## What's NOT included (vs Claude Code)

This is intentionally minimal. Not included:

- Permission system / user approval prompts
- Sub-agent spawning
- MCP protocol support
- Plugin / skill system
- IDE integration
- Persistent memory
- Conversation history persistence
- Cost tracking
- Slash commands
- Markdown rendering (streamed as plain text)

## Development

```bash
# Run tests
cargo test

# Run in debug mode
cargo run

# Build release
cargo build --release
```

## Motivation

This project was built to:

1. **Learn** — understand Claude Code's architecture by reimplementing the core
2. **Build** — produce a fast, usable CLI tool for daily development tasks
3. **Explore Rust** — practice Rust with a real-world async + CLI project

The original Claude Code is ~512K lines of TypeScript. This is ~1,400 lines of Rust doing the essential 20%.

## License

MIT
