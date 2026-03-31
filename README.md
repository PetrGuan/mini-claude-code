# mini-claude-code

A minimal [Claude Code](https://docs.anthropic.com/en/docs/claude-code) CLI rebuilt from scratch in Rust. Interactive AI coding assistant in your terminal — chat with Claude, run commands, read/write/edit files, and search your codebase.

Inspired by [Claude Code](https://docs.anthropic.com/en/docs/claude-code), reimplemented from scratch in ~2,400 lines of Rust.

## Demo

```
  ◆ mini-claude-code v0.1.0
  claude-haiku-4-5-20251001 · bash, read, write, edit, glob, grep
  Enter to send · /cost for usage · Ctrl+C to exit

  ◇ What files are in the src directory?

  ◆ Claude

  [glob: **/*.rs in src/]
  ✓ src/main.rs
    src/repl.rs
    src/auth.rs
    ...

  Here are the source files:
  ● src/main.rs — CLI entry point
  ● src/repl.rs — REPL loop
  ● src/auth.rs — Authentication

  ────────────────────────────────────────

  ◇ /cost

  Session Cost

  Input tokens:       1.2K
  Output tokens:        384
  ─────────────────────
  Total:              1.6K
  Turns:                  1
  Est. cost:         $0.0025
  Model:             claude-haiku-4-5-20251001
```

## Features

- **Interactive REPL** — single Enter to send, streaming output
- **6 built-in tools** that Claude can invoke:

  | Tool | Description |
  |------|-------------|
  | `bash` | Execute shell commands (120s timeout) |
  | `read` | Read files with line numbers, offset, limit |
  | `write` | Create/overwrite files (auto-creates parent dirs) |
  | `edit` | Find-and-replace editing (unique match required) |
  | `glob` | Find files by pattern (`**/*.rs`) |
  | `grep` | Search file contents via ripgrep |

- **Syntax-highlighted code blocks** — powered by syntect, with bordered frames
- **Markdown rendering** — headers, bold, italic, inline code, lists
- **Smart paste** — bracketed paste support; long pastes (>5 lines) collapse to `[Pasted#1: 42 lines]`
- **Animated spinner** — braille animation during thinking + terminal progress bar (blue laser in iTerm2/Ghostty)
- **Cost tracking** — `/cost` command for detailed breakdown, session summary on exit
- **Smart authentication** — auto-detects API key from env, macOS Keychain, or OAuth login
- **Full line editor** — cursor movement (arrow keys, Home/End), Ctrl+W/U/K shortcuts, proper backspace
- **Single binary** — 7.6 MB release build, no runtime dependencies

## Install

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (1.80+)
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

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| Enter | Send message |
| Ctrl+C | Exit |
| ← → | Move cursor |
| Home / Ctrl+A | Jump to start of line |
| End / Ctrl+E | Jump to end of line |
| Ctrl+W | Delete previous word |
| Ctrl+U | Clear line |
| Ctrl+K | Delete to end of line |
| Cmd+V | Paste (long pastes auto-collapse) |

### Commands

| Command | Description |
|---------|-------------|
| `/cost` | Show token usage and estimated cost |

## Architecture

```
src/
├── main.rs              # CLI entry (clap), launches REPL
├── auth.rs              # Multi-strategy auth (env → keychain → OAuth PKCE)
├── cost.rs              # Token/cost tracking with model pricing
├── repl.rs              # REPL loop: input → API → tool execution → output
├── api/
│   ├── client.rs        # Anthropic API client (streaming)
│   ├── types.rs         # Request/response serde types (incl. Usage)
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
    ├── input.rs         # Line editor (cursor, paste, shortcuts)
    ├── render.rs        # Markdown rendering + code block formatting
    ├── highlight.rs     # Syntax highlighting (syntect, lazy-loaded)
    └── spinner.rs       # Animated spinner + terminal progress bar
```

### How it works

```
User input → Build messages → POST /v1/messages (stream: true)
  → Stream text to terminal in real-time
  → If tool_use → execute tool → send result back → loop
  → If end_turn → render markdown → wait for next input
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
| Syntax highlighting | syntect |
| File search | globwalk + ripgrep |

## What's NOT included (vs Claude Code)

This is intentionally minimal. Not included:

- Permission system / user approval prompts
- Sub-agent spawning
- MCP protocol support
- Plugin / skill system
- IDE integration
- Persistent memory
- Conversation history persistence / resume

See [docs/vision.md](docs/vision.md) for the roadmap of planned features.

## Development

```bash
# Run tests (83 tests)
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

~2,400 lines of Rust, focused on the essential 20% of functionality.

## License

MIT
