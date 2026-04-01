use crate::session::{format_relative_time, list_sessions, SessionInfo};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use std::io::{self, Write};
use std::path::PathBuf;

/// RAII guard for raw mode in picker
struct RawModeGuard;
impl Drop for RawModeGuard {
    fn drop(&mut self) {
        terminal::disable_raw_mode().ok();
    }
}

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
    let _guard = RawModeGuard;

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

    drop(_guard); // disable raw mode
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
