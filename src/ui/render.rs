use std::io::{self, Write};
use termimad::MadSkin;

/// Print a chunk of streaming text directly to stdout (no formatting).
pub fn print_stream_chunk(text: &str) {
    print!("{}", text);
    io::stdout().flush().ok();
}

/// Render accumulated markdown text with formatting after streaming completes.
/// Clears the raw streamed text and reprints with proper markdown rendering.
pub fn render_markdown_response(raw_text: &str, lines_printed: usize) {
    // Move cursor up to overwrite the raw streamed text
    if lines_printed > 0 {
        // Move up and clear each line
        for _ in 0..lines_printed {
            print!("\x1b[A\x1b[2K");
        }
        // Also clear current line
        print!("\x1b[2K\r");
        io::stdout().flush().ok();
    }

    let skin = create_skin();
    skin.print_text(raw_text);
}

/// Count displayed lines for a text block (accounting for terminal width wrapping)
pub fn count_display_lines(text: &str) -> usize {
    let term_width = crossterm::terminal::size().map(|(w, _)| w as usize).unwrap_or(80);
    let mut count = 0;
    for line in text.split('\n') {
        if line.is_empty() {
            count += 1;
        } else {
            // Account for line wrapping
            count += (line.len() + term_width - 1) / term_width;
        }
    }
    // At minimum 1 line
    count.max(1)
}

fn create_skin() -> MadSkin {
    let mut skin = MadSkin::default();

    // Subtle styling — let the content breathe
    // Code blocks: dim background
    skin.code_block.set_bg(termimad::crossterm::style::Color::AnsiValue(235));
    // Inline code: slight highlight
    skin.inline_code.set_fg(termimad::crossterm::style::Color::AnsiValue(223));
    // Bold: bright white
    skin.bold.set_fg(termimad::crossterm::style::Color::White);
    // Headers: green, bold
    skin.headers[0].set_fg(termimad::crossterm::style::Color::Green);
    skin.headers[1].set_fg(termimad::crossterm::style::Color::Green);

    skin
}

/// Print a horizontal separator between conversation turns
pub fn print_separator() {
    let width = crossterm::terminal::size().map(|(w, _)| w as usize).unwrap_or(80);
    let line = "\x1b[2m".to_string() + &"─".repeat(width.min(60)) + "\x1b[0m";
    println!("{}", line);
}
