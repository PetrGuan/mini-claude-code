# Session Persistence & Resume — Design Spec

## Goal

Persist conversation history to disk so users can resume previous sessions. Eliminate the pain of losing context when closing the terminal.

## Storage

### Directory structure

```
~/.mini-claude-code/
  projects/
    -Users-petr-Documents-GitHub-mini-claude-code/
      <session-id>.jsonl
      <session-id>.jsonl
    -Users-petr-Documents-GitHub-other-project/
      <session-id>.jsonl
```

Directory name: working directory path with `/` replaced by `-`.

### File format (JSONL)

First line is session metadata, subsequent lines are messages:

```jsonl
{"type":"meta","session_id":"abc123","created":"2026-03-31T12:00:00Z","cwd":"/Users/petr/Documents/GitHub/mini-claude-code","model":"claude-haiku-4-5-20251001"}
{"type":"message","role":"user","content":[{"type":"text","text":"hello"}]}
{"type":"message","role":"assistant","content":[{"type":"text","text":"Hi! How can I help?"}]}
{"type":"message","role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_123","content":"file contents..."}]}
```

Each line is self-contained JSON. Messages use the existing `Message` type (already has Serialize/Deserialize).

### Session ID

UUID v4, generated at session start. Example: `a1b2c3d4-e5f6-7890-abcd-ef1234567890`.

## CLI Interface

| Usage | Effect |
|-------|--------|
| `mini-claude-code` | Start new session |
| `mini-claude-code -c` / `--continue` | Resume most recent session in current directory |
| `mini-claude-code --resume` | Show interactive session picker |

## Auto-save

Every message is appended to the JSONL file immediately after it is added to the conversation history:
- User message (after Enter)
- Assistant message (after streaming completes)
- Tool result messages (after tool execution)

No explicit save command needed. Crash-safe by design — partial sessions are recoverable.

## Resume flow

### `-c` / `--continue`

1. Find the most recent `.jsonl` file in the current project directory (by modification time)
2. Read and parse all lines
3. Rebuild `Vec<Message>` from `type: "message"` lines
4. Restore model from `type: "meta"` line
5. Print a brief context summary: last 2 user messages
6. Continue REPL loop with restored history

If no sessions exist, print a message and start a new session.

### `--resume` (interactive picker)

Show a list of all sessions in the current project directory:

```
  ◆ Select session to resume

  > Fix authentication bug          2 hours ago · 12 messages
    Initial project setup           1 day ago · 47 messages
    Debug streaming parser          3 days ago · 8 messages

  ↑↓ navigate · Enter select · Esc new session
```

Each entry shows:
- **Title**: first user message, truncated to 40 characters
- **Time**: relative ("2 hours ago", "3 days ago")
- **Message count**: number of messages

Navigate with arrow keys, Enter to select, Esc to start a new session.

### Context summary on resume

After loading a session, show the user what was happening:

```
  ◆ Resuming session from 2 hours ago (12 messages)

  ◇ (you) Fix the authentication timeout bug in login.rs
  ◆ (claude) I found the issue — the session token expiration...

  ────────────────────────────────────────
```

Show the last 2 user messages and last assistant text response (truncated) so the user remembers where they left off.

## Architecture

### New file: `src/session.rs`

```rust
pub struct Session {
    pub id: String,
    pub created: String,
    pub cwd: String,
    pub model: String,
    file: Option<File>,  // open handle for appending
}

impl Session {
    pub fn new(cwd: &str, model: &str) -> Session;
    pub fn append_message(&mut self, message: &Message) -> Result<()>;
    pub fn load(path: &Path) -> Result<(SessionMeta, Vec<Message>)>;
    pub fn list_sessions(cwd: &str) -> Result<Vec<SessionInfo>>;
    pub fn most_recent(cwd: &str) -> Result<Option<PathBuf>>;
}

pub struct SessionInfo {
    pub path: PathBuf,
    pub title: String,        // first user message, truncated
    pub modified: SystemTime,
    pub message_count: usize,
}
```

### New file: `src/ui/picker.rs`

Interactive session picker using crossterm raw mode (same pattern as input.rs).

### Modified: `src/main.rs`

Add `-c`/`--continue` and `--resume` flags to clap.

### Modified: `src/repl.rs`

- Accept optional `Vec<Message>` for pre-loaded history
- Call `session.append_message()` after each message is added to history
- On resume, print context summary before entering REPL loop

## Non-goals

- No cross-project session browsing (only current directory)
- No session search/filter (just a list for v1)
- No session deletion command
- No session renaming
- No AI-generated titles
