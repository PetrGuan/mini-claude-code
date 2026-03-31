use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use std::io::{self, Write};

/// RAII guard that ensures raw mode is disabled when dropped
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        terminal::disable_raw_mode().ok();
    }
}

const PROMPT: &str = "  ◇ ";
const CONTINUATION: &str = "  . ";

/// Read user input from the terminal.
/// Supports multiline: press Enter twice to submit, Ctrl+C to exit.
pub fn read_user_input() -> Option<String> {
    print!("\x1b[1;33m{}\x1b[0m", PROMPT);
    io::stdout().flush().ok();

    let mut lines: Vec<String> = vec![String::new()];
    let mut consecutive_newlines = 0;

    let _guard = match RawModeGuard::enable() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error: failed to enable raw mode: {}", e);
            return None;
        }
    };

    loop {
        if let Ok(Event::Key(key_event)) = event::read() {
            match key_event {
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    println!();
                    return None;
                }
                KeyEvent {
                    code: KeyCode::Char('d'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } if lines.len() == 1 && lines[0].is_empty() => {
                    println!();
                    return None;
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    consecutive_newlines += 1;
                    if consecutive_newlines >= 2 {
                        println!();
                        let full: String = lines.join("\n");
                        let trimmed = full.trim().to_string();
                        if trimmed.is_empty() {
                            return Some(String::new());
                        }
                        return Some(trimmed);
                    }
                    lines.push(String::new());
                    print!("\r\n\x1b[2m{}\x1b[0m", CONTINUATION);
                    io::stdout().flush().ok();
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    let is_current_empty = lines.last().map_or(true, |l| l.is_empty());
                    let line_count = lines.len();

                    if is_current_empty && line_count > 1 {
                        // Delete the newline — go back to previous line
                        lines.pop();
                        consecutive_newlines = 0;
                        let prev = lines.last().unwrap().clone();
                        let prefix = if lines.len() == 1 {
                            format!("\x1b[1;33m{}\x1b[0m", PROMPT)
                        } else {
                            format!("\x1b[2m{}\x1b[0m", CONTINUATION)
                        };
                        print!("\x1b[A\r\x1b[2K{}{}", prefix, prev);
                        io::stdout().flush().ok();
                    } else if !is_current_empty {
                        // Delete last character — redraw current line
                        lines.last_mut().unwrap().pop();
                        consecutive_newlines = 0;
                        let current = lines.last().unwrap().clone();
                        let prefix = if lines.len() == 1 {
                            format!("\x1b[1;33m{}\x1b[0m", PROMPT)
                        } else {
                            format!("\x1b[2m{}\x1b[0m", CONTINUATION)
                        };
                        print!("\r\x1b[2K{}{}", prefix, current);
                        io::stdout().flush().ok();
                    }
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    ..
                } => {
                    consecutive_newlines = 0;
                    lines.last_mut().unwrap().push(c);
                    print!("{}", c);
                    io::stdout().flush().ok();
                }
                _ => {}
            }
        }
    }
}
