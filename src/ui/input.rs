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
                    return None;
                }
                KeyEvent {
                    code: KeyCode::Char('d'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } if input.is_empty() => {
                    terminal::disable_raw_mode().ok();
                    println!();
                    return None;
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
