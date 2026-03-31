# mini-claude-code — Vision & Roadmap

What makes mini-claude-code different from Claude Code? Not just "smaller" — a different philosophy.

Claude Code is 512K lines, covers every edge case, and treats the user as a passenger. mini-claude-code explores the opposite: minimal, transparent, composable, and user-controlled.

---

## 1. Radical Transparency

Claude Code is a black box. mini-claude-code should be a glass box.

**Ideas:**
- `--verbose` flag: print the exact JSON sent to the API and received back
- Real-time token counter in the prompt (input/output tokens per turn)
- `--show-system-prompt` to inspect what the AI actually sees
- `/history` command to view and edit the message array mid-conversation
- `/replay <turn>` to re-send a modified version of a previous message

**Philosophy:** The user is a driver, not a passenger. Every API call is visible and auditable. This also makes mini-claude-code a learning tool — watch how LLM tool use actually works under the hood.

---

## 2. Unix Philosophy

Claude Code is a monolithic REPL. mini-claude-code should also be a composable Unix tool.

**Ideas:**
- `--pipe` mode: read from stdin, write to stdout, no interactive UI
  ```bash
  echo "explain this code" | mini-claude-code --pipe < src/main.rs
  git diff | mini-claude-code --pipe "review this diff"
  cat error.log | mini-claude-code --pipe "what went wrong?"
  ```
- `--tool <name> <args>` mode: execute a single tool without conversation
  ```bash
  mini-claude-code --tool grep "TODO" src/
  mini-claude-code --tool read src/main.rs
  ```
- `--one-shot <prompt>` mode: single question, print answer, exit (no REPL)
- Composable with `jq`, `rg`, `fzf`, and other CLI tools
- Exit codes that make sense in scripts (0 = success, 1 = error, 2 = tool failure)

**Philosophy:** Do one thing well. Be a good citizen in the Unix ecosystem. Text in, text out.

---

## 3. Session as Code

Conversations disappear after you close the terminal. What if they didn't?

**Ideas:**
- Auto-export every session to a replay file (JSON or YAML)
  ```
  ~/.mini-claude-code/sessions/2026-03-31T12:00:00.json
  ```
- `--replay <file>` to re-execute a session (all tool calls become real commands)
- `--export-script <file>` to convert a session into a standalone shell script
  ```bash
  # Auto-generated from mini-claude-code session 2026-03-31
  # Turn 1: "create a hello world rust project"
  mkdir hello-world && cd hello-world
  cargo init
  cat > src/main.rs << 'EOF'
  fn main() { println!("Hello, world!"); }
  EOF
  cargo run
  ```
- Sessions are git-friendly — diff two sessions to see what changed
- `--resume` to continue a previous session

**Philosophy:** AI-assisted work should be reproducible, auditable, and version-controllable. Not trapped in a chat window.

---

## 4. Cost Consciousness

Claude Code barely mentions cost. mini-claude-code should make cost a first-class citizen.

**Ideas:**
- Show cost per turn in the prompt: `[$0.003 | 1.2K tokens]`
- Running session total: `Session: $0.15 / 45K tokens`
- `--budget <amount>` flag: stop and warn when approaching limit
  ```bash
  mini-claude-code --budget 1.00  # max $1 per session
  ```
- Auto model selection: use Haiku for simple questions, upgrade to Sonnet only when needed
- `/cost` command to show breakdown by turn

**Philosophy:** AI tools shouldn't be surprise bills. Users should know exactly what they're spending, in real time.

---

## 5. Programmable AI

mini-claude-code as a library, not just a CLI.

**Ideas:**
- Expose the core as a Rust library crate (`mini_claude_code::Client`)
  ```rust
  let client = mini_claude_code::Client::new(auth);
  let response = client.chat("explain this code", &tools).await?;
  ```
- `.mini-claude-code.toml` project config:
  ```toml
  [model]
  default = "claude-haiku-4-5-20251001"
  upgrade_to = "claude-sonnet-4-20250514"

  [system_prompt]
  prepend = "You are working on a Rust project. Follow Rust conventions."

  [tools.custom.lint]
  command = "cargo clippy --message-format=json"
  description = "Run Rust linter"
  ```
- Custom tools defined in config — any shell command becomes a tool Claude can invoke
- Custom system prompt per project directory

**Philosophy:** The tool adapts to the user's workflow, not the other way around.

---

## Implementation Order

These are roughly independent. Suggested order based on impact and complexity:

| Priority | Feature | Effort | Impact |
|----------|---------|--------|--------|
| 1 | Unix Philosophy (`--pipe`, `--one-shot`) | Small | High — immediately useful |
| 2 | Radical Transparency (`--verbose`, token counts) | Small | High — unique differentiator |
| 3 | Cost Consciousness (per-turn cost display) | Small | Medium — builds on transparency |
| 4 | Session as Code (export, replay) | Medium | Medium — novel concept |
| 5 | Programmable AI (config, custom tools, library) | Large | High — but can wait |

Each item gets its own design → plan → implementation cycle.
