use std::io::{self, Write};
use termimad::MadSkin;

/// Start the AI response area — print a header with the assistant label
pub fn print_response_header() {
    println!("\x1b[1;36m  ◆ Claude\x1b[0m");
    println!();
}

/// Print a chunk of streaming text to stdout.
pub fn print_stream_chunk(text: &str) {
    print!("{}", text);
    io::stdout().flush().ok();
}

/// After streaming completes, render the full response with markdown.
/// This replaces the raw streamed text with a formatted version.
pub fn render_final_response(raw_text: &str, lines_to_clear: usize) {
    // Move cursor up to overwrite raw streamed text
    for _ in 0..lines_to_clear {
        print!("\x1b[A\x1b[2K");
    }
    print!("\x1b[2K\r");
    io::stdout().flush().ok();

    // Render with termimad
    let skin = create_skin();
    skin.print_text(raw_text);
    println!();
}

/// Count how many terminal lines a text block occupies
pub fn count_display_lines(text: &str) -> usize {
    let term_width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    let mut count = 0;
    for line in text.split('\n') {
        let visible_len = strip_ansi_len(line);
        if visible_len == 0 {
            count += 1;
        } else {
            count += (visible_len + term_width - 1) / term_width;
        }
    }
    count.max(1)
}

/// Get the visible length of a string (excluding ANSI escape codes)
fn strip_ansi_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            len += 1;
        }
    }
    len
}

fn create_skin() -> MadSkin {
    let mut skin = MadSkin::default();

    // Code blocks: dark background, stand out clearly
    skin.code_block
        .set_bg(termimad::crossterm::style::Color::AnsiValue(235));
    skin.code_block
        .set_fg(termimad::crossterm::style::Color::AnsiValue(252));

    // Inline code: warm highlight
    skin.inline_code
        .set_fg(termimad::crossterm::style::Color::AnsiValue(216));
    skin.inline_code
        .set_bg(termimad::crossterm::style::Color::AnsiValue(236));

    // Bold: bright
    skin.bold
        .set_fg(termimad::crossterm::style::Color::White);

    // Italic: soft cyan
    skin.italic
        .set_fg(termimad::crossterm::style::Color::AnsiValue(117));

    // Headers
    skin.headers[0].set_fg(termimad::crossterm::style::Color::AnsiValue(117));
    skin.headers[1].set_fg(termimad::crossterm::style::Color::AnsiValue(117));

    // Bullet points
    skin.bullet = termimad::StyledChar::from_fg_char(
        termimad::crossterm::style::Color::AnsiValue(75),
        '●',
    );

    skin
}

/// Print a horizontal separator between conversation turns
pub fn print_separator() {
    let width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    let line_width = width.min(60);
    println!("\x1b[2m{}\x1b[0m", "─".repeat(line_width));
}
