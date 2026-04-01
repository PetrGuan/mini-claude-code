# Session Persistence & Resume — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist conversations to disk (JSONL) and let users resume them via `-c` (most recent) or `--resume` (interactive picker).

**Architecture:** A `Session` struct manages JSONL file I/O — append on each message, load on resume. The REPL accepts optional pre-loaded messages. CLI flags route to new/continue/pick flows. An interactive picker uses crossterm raw mode for session selection.

**Tech Stack:** serde_json (JSONL), uuid (session IDs), chrono or std::time (timestamps), crossterm (picker UI)

---

## File Structure

```
src/
├── session.rs        # NEW — Session struct, save/load/list, directory management
├── ui/picker.rs      # NEW — Interactive session selector
├── ui/mod.rs         # MODIFY — add pub mod picker
├── main.rs           # MODIFY — add -c/--resume flags, route to session flows
├── repl.rs           # MODIFY — accept pre-loaded messages, call session.append()
├── lib.rs            # MODIFY — add pub mod session
```

---

### Task 1: Session Module — Core Types & Save

**Files:**
- Create: `src/session.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add uuid dependency to Cargo.toml**

Add under `[dependencies]`:
```toml
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 2: Create session.rs with types and new/append**

`src/session.rs`:

```rust
use crate::api::types::Message;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const BASE_DIR: &str = ".mini-claude-code";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum SessionLine {
    #[serde(rename = "meta")]
    Meta {
        session_id: String,
        created: String,
        cwd: String,
        model: String,
    },
    #[serde(rename = "message")]
    Message {
        #[serde(flatten)]
        message: Message,
    },
}

pub struct Session {
    pub id: String,
    pub model: String,
    path: PathBuf,
    file: File,
}

impl Session {
    /// Create a new session and write the meta line.
    pub fn new(cwd: &str, model: &str) -> Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        let dir = project_dir(cwd)?;
        fs::create_dir_all(&dir)?;

        let path = dir.join(format!("{}.jsonl", id));
        let mut file = File::create(&path)?;

        let now = humantime::format_rfc3339(SystemTime::now()).to_string();
        let meta = SessionLine::Meta {
            session_id: id.clone(),
            created: now,
            cwd: cwd.to_string(),
            model: model.to_string(),
        };
        writeln!(file, "{}", serde_json::to_string(&meta)?)?;
        file.flush()?;

        Ok(Self {
            id,
            model: model.to_string(),
            path,
            file,
        })
    }

    /// Append a message to the session file.
    pub fn append_message(&mut self, message: &Message) -> Result<()> {
        let line = SessionLine::Message {
            message: message.clone(),
        };
        writeln!(self.file, "{}", serde_json::to_string(&line)?)?;
        self.file.flush()?;
        Ok(())
    }

    /// Load a session from a JSONL file. Returns (model, messages).
    pub fn load(path: &Path) -> Result<(String, Vec<Message>)> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut model = String::new();
        let mut messages = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<SessionLine>(&line) {
                Ok(SessionLine::Meta {
                    model: m,
                    ..
                }) => {
                    model = m;
                }
                Ok(SessionLine::Message { message }) => {
                    messages.push(message);
                }
                Err(_) => continue, // skip malformed lines
            }
        }

        if model.is_empty() {
            return Err(anyhow!("Session file missing metadata"));
        }

        Ok((model, messages))
    }

    /// Open an existing session file for appending (used after load).
    pub fn open_existing(path: &Path, model: &str) -> Result<Self> {
        let file = OpenOptions::new().append(true).open(path)?;
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            id,
            model: model.to_string(),
            path: path.to_path_buf(),
            file,
        })
    }
}

/// Info about a session for display in the picker.
pub struct SessionInfo {
    pub path: PathBuf,
    pub title: String,
    pub modified: SystemTime,
    pub message_count: usize,
}

/// List all sessions for the given working directory, sorted by most recent first.
pub fn list_sessions(cwd: &str) -> Result<Vec<SessionInfo>> {
    let dir = match project_dir(cwd) {
        Ok(d) => d,
        Err(_) => return Ok(Vec::new()),
    };

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();

    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let modified = entry.metadata()?.modified()?;

        // Read first few lines to get title and count
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut title = String::new();
        let mut message_count = 0;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            if let Ok(parsed) = serde_json::from_str::<SessionLine>(&line) {
                match parsed {
                    SessionLine::Message { message } => {
                        message_count += 1;
                        // Use first user text message as title
                        if title.is_empty() {
                            if let Some(text) = message.content.iter().find_map(|b| match b {
                                crate::api::types::ContentBlock::Text { text } => Some(text.as_str()),
                                _ => None,
                            }) {
                                title = text.chars().take(50).collect();
                            }
                        }
                    }
                    SessionLine::Meta { .. } => {}
                }
            }
        }

        if title.is_empty() {
            title = "(empty session)".to_string();
        }

        sessions.push(SessionInfo {
            path,
            title,
            modified,
            message_count,
        });
    }

    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(sessions)
}

/// Find the most recent session for the given working directory.
pub fn most_recent_session(cwd: &str) -> Result<Option<PathBuf>> {
    let sessions = list_sessions(cwd)?;
    Ok(sessions.into_iter().next().map(|s| s.path))
}

/// Get the project directory path for a given working directory.
fn project_dir(cwd: &str) -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| anyhow!("HOME not set"))?;
    let sanitized = cwd.replace('/', "-");
    Ok(PathBuf::from(home)
        .join(BASE_DIR)
        .join("projects")
        .join(sanitized))
}

/// Format a SystemTime as a relative time string ("2 hours ago", "3 days ago").
pub fn format_relative_time(time: SystemTime) -> String {
    let elapsed = SystemTime::now()
        .duration_since(time)
        .unwrap_or_default();
    let secs = elapsed.as_secs();

    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        let mins = secs / 60;
        format!("{} min{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if secs < 86400 {
        let hours = secs / 3600;
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else {
        let days = secs / 86400;
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{ContentBlock, Role};
    use tempfile::TempDir;

    fn test_session_in(dir: &Path, model: &str) -> Session {
        // Override project_dir by creating session directly
        let id = uuid::Uuid::new_v4().to_string();
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(format!("{}.jsonl", id));
        let mut file = File::create(&path).unwrap();
        let meta = SessionLine::Meta {
            session_id: id.clone(),
            created: "2026-03-31T12:00:00Z".to_string(),
            cwd: "/test".to_string(),
            model: model.to_string(),
        };
        writeln!(file, "{}", serde_json::to_string(&meta).unwrap()).unwrap();
        Session {
            id,
            model: model.to_string(),
            path,
            file,
        }
    }

    #[test]
    fn test_append_and_load() {
        let dir = TempDir::new().unwrap();
        let mut session = test_session_in(dir.path(), "haiku");

        let msg = Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: "hello".to_string(),
            }],
        };
        session.append_message(&msg).unwrap();

        let (model, messages) = Session::load(&session.path).unwrap();
        assert_eq!(model, "haiku");
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_format_relative_time() {
        let now = SystemTime::now();
        assert_eq!(format_relative_time(now), "just now");

        let one_hour_ago = now - std::time::Duration::from_secs(3600);
        assert_eq!(format_relative_time(one_hour_ago), "1 hour ago");

        let two_days_ago = now - std::time::Duration::from_secs(86400 * 2);
        assert_eq!(format_relative_time(two_days_ago), "2 days ago");
    }
}
```

- [ ] **Step 3: Add `humantime` dependency to Cargo.toml**

```toml
humantime = "2"
```

- [ ] **Step 4: Register module in lib.rs and main.rs**

Add `pub mod session;` to `src/lib.rs` and `mod session;` to `src/main.rs`.

- [ ] **Step 5: Verify it compiles and tests pass**

```bash
cargo test session::tests
```

Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/session.rs src/lib.rs src/main.rs Cargo.toml Cargo.lock
git commit -m "feat: session module — JSONL save/load/list"
```

---

### Task 2: Wire Session into REPL

**Files:**
- Modify: `src/repl.rs`

- [ ] **Step 1: Change `run` signature to accept optional session and pre-loaded messages**

Change the function signature from:
```rust
pub async fn run(client: &AnthropicClient, registry: &ToolRegistry) -> Result<()> {
```
to:
```rust
pub async fn run(
    client: &AnthropicClient,
    registry: &ToolRegistry,
    mut session: crate::session::Session,
    mut messages: Vec<Message>,
) -> Result<()> {
```

Remove the line `let mut messages: Vec<Message> = Vec::new();` since messages are now a parameter.

- [ ] **Step 2: Add session.append_message() calls after each message push**

After each `messages.push(...)` in the REPL loop (there are 3: user message, assistant message, tool results), add `session.append_message(...)`. Specifically:

After the user message push (~line 141):
```rust
        messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: input }],
        });
        let _ = session.append_message(messages.last().unwrap());
```

After the assistant message push (~line 288):
```rust
            messages.push(Message {
                role: Role::Assistant,
                content: assistant_content.clone(),
            });
            let _ = session.append_message(messages.last().unwrap());
```

After the tool results push (~line 355):
```rust
            messages.push(Message {
                role: Role::User,
                content: tool_results,
            });
            let _ = session.append_message(messages.last().unwrap());
```

- [ ] **Step 3: Show context summary when resuming (messages not empty)**

After the welcome banner, add:
```rust
    // Show context summary if resuming
    if !messages.is_empty() {
        let user_msgs: Vec<_> = messages
            .iter()
            .filter(|m| matches!(m.role, Role::User))
            .filter_map(|m| {
                m.content.iter().find_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
            })
            .collect();

        let msg_count = messages.len();
        println!("  \x1b[2mResuming session ({} messages)\x1b[0m", msg_count);
        if let Some(last) = user_msgs.last() {
            let preview: String = last.chars().take(80).collect();
            println!("  \x1b[2mLast: {}\x1b[0m", preview);
        }
        println!();
    }
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo build
```

Note: `main.rs` will need updating in the next task to pass session/messages to `run()`. For now it won't compile — that's expected.

- [ ] **Step 5: Commit**

```bash
git add src/repl.rs
git commit -m "feat: wire session persistence into REPL loop"
```

---

### Task 3: CLI Flags & Main Routing

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add CLI flags for continue and resume**

Update the `Cli` struct:

```rust
#[derive(Parser)]
#[command(name = "mini-claude-code", about = "A minimal Claude Code CLI")]
struct Cli {
    /// Model to use
    #[arg(short, long, default_value = "claude-haiku-4-5-20251001")]
    model: String,

    /// Max tokens for response
    #[arg(long, default_value_t = 8192)]
    max_tokens: u32,

    /// Continue most recent session
    #[arg(short = 'c', long = "continue")]
    continue_session: bool,

    /// Resume a session (interactive picker)
    #[arg(short, long)]
    resume: bool,
}
```

- [ ] **Step 2: Update main() to route based on flags**

Replace the body of the `block_on` async block with:

```rust
    tokio::runtime::Runtime::new()?.block_on(async {
        let cwd = std::env::current_dir()?.display().to_string();

        let (session, messages, model) = if cli.continue_session {
            // Continue most recent session
            match session::most_recent_session(&cwd)? {
                Some(path) => {
                    let (model, messages) = session::Session::load(&path)?;
                    let session = session::Session::open_existing(&path, &model)?;
                    (session, messages, model)
                }
                None => {
                    eprintln!("  \x1b[2mNo previous sessions found. Starting new session.\x1b[0m");
                    let session = session::Session::new(&cwd, &cli.model)?;
                    (session, Vec::new(), cli.model.clone())
                }
            }
        } else if cli.resume {
            // Interactive picker (implemented in Task 4)
            match ui::picker::pick_session(&cwd)? {
                Some(path) => {
                    let (model, messages) = session::Session::load(&path)?;
                    let session = session::Session::open_existing(&path, &model)?;
                    (session, messages, model)
                }
                None => {
                    let session = session::Session::new(&cwd, &cli.model)?;
                    (session, Vec::new(), cli.model.clone())
                }
            }
        } else {
            // New session
            let session = session::Session::new(&cwd, &cli.model)?;
            (session, Vec::new(), cli.model.clone())
        };

        let mut client = api::client::AnthropicClient::new(auth, model, cli.max_tokens);
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

        repl::run(&client, &registry, session, messages).await
    })
```

- [ ] **Step 3: Create a stub `ui::picker::pick_session`**

Create `src/ui/picker.rs`:

```rust
use std::path::{Path, PathBuf};
use anyhow::Result;

/// Show interactive session picker. Returns selected session path, or None for new session.
pub fn pick_session(cwd: &str) -> Result<Option<PathBuf>> {
    // Stub — implemented in Task 4
    Ok(None)
}
```

Add `pub mod picker;` to `src/ui/mod.rs`.

- [ ] **Step 4: Verify it compiles and runs**

```bash
cargo build
cargo run -- --help
```

Expected: Help text shows `-c, --continue` and `-r, --resume` flags.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/ui/picker.rs src/ui/mod.rs
git commit -m "feat: CLI flags for --continue and --resume"
```

---

### Task 4: Interactive Session Picker

**Files:**
- Modify: `src/ui/picker.rs`

- [ ] **Step 1: Implement the picker**

Replace `src/ui/picker.rs` with:

```rust
use crate::session::{format_relative_time, list_sessions, SessionInfo};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use std::io::{self, Write};
use std::path::PathBuf;

/// Show interactive session picker. Returns selected session path, or None for new session.
pub fn pick_session(cwd: &str) -> Result<Option<PathBuf>> {
    let sessions = list_sessions(cwd)?;

    if sessions.is_empty() {
        println!("  \x1b[2mNo previous sessions found.\x1b[0m");
        return Ok(None);
    }

    println!();
    println!("  \x1b[1;36m◆ Select session to resume\x1b[0m");
    println!();

    let mut selected: usize = 0;

    terminal::enable_raw_mode()?;

    draw_list(&sessions, selected);
    println!();
    print!("  \x1b[2m↑↓ navigate · Enter select · Esc new session\x1b[0m");
    io::stdout().flush()?;

    let result = loop {
        if let Ok(Event::Key(key)) = event::read() {
            match key {
                KeyEvent {
                    code: KeyCode::Up, ..
                } => {
                    if selected > 0 {
                        selected -= 1;
                        redraw(&sessions, selected);
                    }
                }
                KeyEvent {
                    code: KeyCode::Down,
                    ..
                } => {
                    if selected < sessions.len() - 1 {
                        selected += 1;
                        redraw(&sessions, selected);
                    }
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    break Some(sessions[selected].path.clone());
                }
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    break None;
                }
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    break None;
                }
                _ => {}
            }
        }
    };

    terminal::disable_raw_mode()?;
    // Clear the picker UI
    let total_lines = sessions.len() + 2; // list + blank + hint
    for _ in 0..total_lines {
        print!("\x1b[A\x1b[2K");
    }
    io::stdout().flush()?;
    println!();

    Ok(result)
}

fn draw_list(sessions: &[SessionInfo], selected: usize) {
    for (i, session) in sessions.iter().enumerate() {
        let time = format_relative_time(session.modified);
        let indicator = if i == selected { ">" } else { " " };
        let style_start = if i == selected { "\x1b[1;36m" } else { "" };
        let style_end = if i == selected { "\x1b[0m" } else { "" };

        println!(
            "\r  {} {}{}{} \x1b[2m{} · {} messages\x1b[0m",
            indicator, style_start, session.title, style_end, time, session.message_count
        );
    }
}

fn redraw(sessions: &[SessionInfo], selected: usize) {
    // Move up to start of list + hint line + blank line
    let up = sessions.len() + 2;
    for _ in 0..up {
        print!("\x1b[A\x1b[2K");
    }
    draw_list(sessions, selected);
    println!();
    print!("  \x1b[2m↑↓ navigate · Enter select · Esc new session\x1b[0m");
    io::stdout().flush().ok();
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build
```

- [ ] **Step 3: Commit**

```bash
git add src/ui/picker.rs
git commit -m "feat: interactive session picker for --resume"
```

---

### Task 5: Integration Test & Polish

**Files:**
- Modify: `src/repl.rs` (banner update)

- [ ] **Step 1: Update the welcome banner to mention resume**

Change the hint line in repl.rs:
```rust
    println!("  \x1b[2mEnter to send · /cost for usage · Ctrl+C to exit\x1b[0m");
```
to:
```rust
    println!("  \x1b[2mEnter to send · /cost for usage · -c to resume · Ctrl+C to exit\x1b[0m");
```

- [ ] **Step 2: Build release and test all flows manually**

```bash
cargo build --release
```

Test:
1. **New session:** `cargo run` → chat → Ctrl+C → check `~/.mini-claude-code/projects/` has JSONL file
2. **Continue:** `cargo run -- -c` → should resume with context summary
3. **Resume picker:** `cargo run -- --resume` → should show session list

- [ ] **Step 3: Verify all tests pass**

```bash
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "polish: banner update, integration testing"
```

- [ ] **Step 5: Push**

```bash
git push origin master
```
