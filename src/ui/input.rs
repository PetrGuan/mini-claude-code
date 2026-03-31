use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyModifiers,
};
use crossterm::terminal;
use std::io::{self, Write};

/// RAII guard that ensures raw mode and bracketed paste are disabled when dropped
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        // Enable bracketed paste so we can distinguish pasted text from typed text
        crossterm::execute!(io::stdout(), EnableBracketedPaste)?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        crossterm::execute!(io::stdout(), DisableBracketedPaste).ok();
        terminal::disable_raw_mode().ok();
    }
}

const PROMPT: &str = "  ◇ ";
const CONTINUATION: &str = "  . ";

/// A line editor that tracks cursor position within the current line.
struct LineEditor {
    lines: Vec<String>,
    cursor: usize, // cursor position within the current (last) line
    consecutive_newlines: usize,
}

impl LineEditor {
    fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: 0,
            consecutive_newlines: 0,
        }
    }

    fn current_line(&self) -> &str {
        self.lines.last().unwrap()
    }

    fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    fn insert_char(&mut self, c: char) {
        self.consecutive_newlines = 0;
        let line = self.lines.last_mut().unwrap();
        if self.cursor >= line.len() {
            line.push(c);
        } else {
            line.insert(self.cursor, c);
        }
        self.cursor += c.len_utf8();
    }

    fn newline(&mut self) -> bool {
        self.consecutive_newlines += 1;
        if self.consecutive_newlines >= 2 {
            return true; // signal: submit
        }
        self.lines.push(String::new());
        self.cursor = 0;
        false
    }

    fn backspace(&mut self) -> bool {
        let is_current_empty = self.current_line().is_empty();

        if is_current_empty && self.lines.len() > 1 {
            self.lines.pop();
            self.cursor = self.current_line().len();
            self.consecutive_newlines = 0;
            return true; // redraw needed (went up a line)
        }

        if !is_current_empty && self.cursor > 0 {
            let line = self.lines.last_mut().unwrap();
            // Find the previous character boundary
            let new_cursor = line[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            line.replace_range(new_cursor..self.cursor, "");
            self.cursor = new_cursor;
            self.consecutive_newlines = 0;
        }
        false
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            let line = self.current_line();
            self.cursor = line[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    fn move_right(&mut self) {
        let len = self.current_line().len();
        if self.cursor < len {
            let line = self.current_line();
            self.cursor = line[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(len);
        }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.current_line().len();
    }

    /// Delete from cursor to end of line (Ctrl+K)
    fn kill_to_end(&mut self) {
        let line = self.lines.last_mut().unwrap();
        line.truncate(self.cursor);
    }

    /// Clear entire current line (Ctrl+U)
    fn clear_line(&mut self) {
        self.lines.last_mut().unwrap().clear();
        self.cursor = 0;
    }

    /// Delete previous word (Ctrl+W)
    fn delete_word(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let line = self.lines.last_mut().unwrap();
        let before = &line[..self.cursor];
        // Skip trailing spaces, then skip non-spaces
        let new_cursor = before
            .trim_end()
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        line.replace_range(new_cursor..self.cursor, "");
        self.cursor = new_cursor;
    }

    /// Insert pasted text (may contain newlines)
    fn insert_paste(&mut self, text: &str) {
        self.consecutive_newlines = 0;
        for c in text.chars() {
            if c == '\n' || c == '\r' {
                self.lines.push(String::new());
                self.cursor = 0;
            } else {
                self.insert_char(c);
            }
        }
    }

    fn full_text(&self) -> String {
        self.lines.join("\n")
    }
}

/// Read user input from the terminal.
/// Supports multiline, paste, cursor movement, and common shortcuts.
pub fn read_user_input() -> Option<String> {
    print!("\x1b[1;33m{}\x1b[0m", PROMPT);
    io::stdout().flush().ok();

    let mut editor = LineEditor::new();

    let _guard = match RawModeGuard::enable() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error: failed to enable raw mode: {}", e);
            return None;
        }
    };

    loop {
        let event = match event::read() {
            Ok(ev) => ev,
            Err(_) => continue,
        };

        match event {
            // Bracketed paste — handle pasted text as a block
            Event::Paste(text) => {
                editor.insert_paste(&text);
                redraw_all(&editor);
            }

            Event::Key(key_event) => match key_event {
                // Ctrl+C → exit
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    println!();
                    return None;
                }

                // Ctrl+D on empty → exit
                KeyEvent {
                    code: KeyCode::Char('d'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } if editor.is_empty() => {
                    println!();
                    return None;
                }

                // Enter
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    if editor.newline() {
                        println!();
                        let text = editor.full_text();
                        let trimmed = text.trim().to_string();
                        return Some(trimmed);
                    }
                    print!("\r\n\x1b[2m{}\x1b[0m", CONTINUATION);
                    io::stdout().flush().ok();
                }

                // Backspace
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    let went_up = editor.backspace();
                    if went_up {
                        // Went to previous line — need to redraw
                        let line = editor.current_line().to_string();
                        let prefix = line_prefix(editor.lines.len() == 1);
                        print!("\x1b[A\r\x1b[2K{}{}", prefix, line);
                        position_cursor(&editor);
                        io::stdout().flush().ok();
                    } else {
                        redraw_current_line(&editor);
                    }
                }

                // Delete key
                KeyEvent {
                    code: KeyCode::Delete,
                    ..
                } => {
                    let line = editor.lines.last_mut().unwrap();
                    if editor.cursor < line.len() {
                        let next = line[editor.cursor..]
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| editor.cursor + i)
                            .unwrap_or(line.len());
                        line.replace_range(editor.cursor..next, "");
                        redraw_current_line(&editor);
                    }
                }

                // Left arrow
                KeyEvent {
                    code: KeyCode::Left,
                    ..
                } => {
                    editor.move_left();
                    position_cursor(&editor);
                    io::stdout().flush().ok();
                }

                // Right arrow
                KeyEvent {
                    code: KeyCode::Right,
                    ..
                } => {
                    editor.move_right();
                    position_cursor(&editor);
                    io::stdout().flush().ok();
                }

                // Home / Ctrl+A
                KeyEvent {
                    code: KeyCode::Home,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('a'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    editor.move_home();
                    position_cursor(&editor);
                    io::stdout().flush().ok();
                }

                // End / Ctrl+E
                KeyEvent {
                    code: KeyCode::End,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('e'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    editor.move_end();
                    position_cursor(&editor);
                    io::stdout().flush().ok();
                }

                // Ctrl+U — clear line
                KeyEvent {
                    code: KeyCode::Char('u'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    editor.clear_line();
                    redraw_current_line(&editor);
                }

                // Ctrl+W — delete word
                KeyEvent {
                    code: KeyCode::Char('w'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    editor.delete_word();
                    redraw_current_line(&editor);
                }

                // Ctrl+K — kill to end of line
                KeyEvent {
                    code: KeyCode::Char('k'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    editor.kill_to_end();
                    redraw_current_line(&editor);
                }

                // Regular character
                KeyEvent {
                    code: KeyCode::Char(c),
                    ..
                } => {
                    let at_end = editor.cursor >= editor.current_line().len();
                    editor.insert_char(c);
                    if at_end {
                        // Fast path: just print the character
                        print!("{}", c);
                        io::stdout().flush().ok();
                    } else {
                        // Inserted in middle: redraw line
                        redraw_current_line(&editor);
                    }
                }

                _ => {}
            },

            _ => {}
        }
    }
}

fn line_prefix(is_first: bool) -> String {
    if is_first {
        format!("\x1b[1;33m{}\x1b[0m", PROMPT)
    } else {
        format!("\x1b[2m{}\x1b[0m", CONTINUATION)
    }
}

/// Redraw just the current (last) line
fn redraw_current_line(editor: &LineEditor) {
    let line = editor.current_line();
    let prefix = line_prefix(editor.lines.len() == 1);
    print!("\r\x1b[2K{}{}", prefix, line);
    position_cursor(editor);
    io::stdout().flush().ok();
}

/// Redraw all lines (used after paste)
fn redraw_all(editor: &LineEditor) {
    // Move to the first line
    let up = editor.lines.len() - 1;
    if up > 0 {
        print!("\x1b[{}A", up);
    }
    print!("\r");

    for (i, line) in editor.lines.iter().enumerate() {
        let prefix = line_prefix(i == 0);
        print!("\x1b[2K{}{}", prefix, line);
        if i < editor.lines.len() - 1 {
            print!("\r\n");
        }
    }
    position_cursor(editor);
    io::stdout().flush().ok();
}

/// Move the terminal cursor to match editor.cursor position
fn position_cursor(editor: &LineEditor) {
    let prefix_width = if editor.lines.len() == 1 {
        display_width(PROMPT)
    } else {
        display_width(CONTINUATION)
    };
    let text_before_cursor = &editor.current_line()[..editor.cursor];
    let col = prefix_width + display_width(text_before_cursor);
    // Move to absolute column (1-based)
    print!("\r\x1b[{}C", col);
}

/// Get display width of a string (ASCII = 1, CJK/emoji ≈ 2)
fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            if c.is_ascii() {
                1
            } else {
                // CJK and most non-ASCII are 2 columns wide
                2
            }
        })
        .sum()
}
