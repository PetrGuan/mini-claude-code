use crate::ui::highlight::highlight_code;
use std::io::{self, Write};

/// Start the AI response area
pub fn print_response_header() {
    println!("\x1b[1;36m  ◆ Claude\x1b[0m");
    println!();
}

/// Print a chunk of streaming text to stdout.
pub fn print_stream_chunk(text: &str) {
    print!("{}", text);
    io::stdout().flush().ok();
}

/// After streaming completes, render the full response with formatting.
/// Clears the raw streamed text, then reprints with header + markdown.
pub fn render_final_response(raw_text: &str, lines_to_clear: usize) {
    // Move cursor up to overwrite raw streamed text + the blank line before it
    for _ in 0..lines_to_clear + 1 {
        print!("\x1b[A\x1b[2K");
    }
    print!("\x1b[2K\r");
    io::stdout().flush().ok();

    // Print header + rendered markdown
    print_response_header();
    let rendered = render_markdown(raw_text);
    print!("{}", rendered);
    io::stdout().flush().ok();
    println!();
}

/// Count how many terminal lines a text block occupies
pub fn count_display_lines(text: &str) -> usize {
    let term_width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    let mut count = 0;
    for line in text.split('\n') {
        let visible_len = visible_char_count(line);
        if visible_len == 0 {
            count += 1;
        } else {
            count += (visible_len + term_width - 1) / term_width;
        }
    }
    count.max(1)
}

/// Print a horizontal separator between conversation turns
pub fn print_separator() {
    let width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    println!("\x1b[2m{}\x1b[0m", "─".repeat(width.min(60)));
}

/// Render markdown text with syntax-highlighted code blocks and basic formatting.
fn render_markdown(text: &str) -> String {
    let mut output = String::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_content = String::new();

    for line in text.lines() {
        if line.starts_with("```") {
            if in_code_block {
                // End of code block — render with syntax highlighting
                output.push_str(&render_code_block(&code_content, &code_lang));
                code_content.clear();
                code_lang.clear();
                in_code_block = false;
            } else {
                // Start of code block
                code_lang = line.trim_start_matches('`').trim().to_string();
                in_code_block = true;
            }
        } else if in_code_block {
            if !code_content.is_empty() {
                code_content.push('\n');
            }
            code_content.push_str(line);
        } else {
            // Regular text — apply inline formatting
            output.push_str(&render_text_line(line));
            output.push('\n');
        }
    }

    // Handle unclosed code block
    if in_code_block && !code_content.is_empty() {
        output.push_str(&render_code_block(&code_content, &code_lang));
    }

    output
}

/// Render a code block with syntax highlighting and a visual frame
fn render_code_block(code: &str, lang: &str) -> String {
    let term_width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    let block_width = term_width.min(80);

    let mut output = String::new();

    // Top border with language label
    let label = if lang.is_empty() {
        String::new()
    } else {
        format!(" {} ", lang)
    };
    let border_len = block_width.saturating_sub(label.len() + 2);
    output.push_str(&format!(
        "\x1b[2m  ╭{}{}\x1b[0m\n",
        label,
        "─".repeat(border_len)
    ));

    // Highlighted code lines
    let highlighted = highlight_code(code, lang);
    for line in highlighted.lines() {
        output.push_str(&format!("\x1b[2m  │\x1b[0m {}\n", line));
    }

    // Bottom border
    output.push_str(&format!(
        "\x1b[2m  ╰{}\x1b[0m\n",
        "─".repeat(block_width)
    ));

    output
}

/// Apply inline formatting to a text line
fn render_text_line(line: &str) -> String {
    // Headers
    if line.starts_with("### ") {
        return format!("\x1b[1;37m{}\x1b[0m", &line[4..]);
    }
    if line.starts_with("## ") {
        return format!("\x1b[1;37m{}\x1b[0m", &line[3..]);
    }
    if line.starts_with("# ") {
        return format!("\x1b[1;36m{}\x1b[0m", &line[2..]);
    }

    // Bullet points
    if line.starts_with("- ") || line.starts_with("* ") {
        return format!("  \x1b[36m●\x1b[0m {}", render_inline(&line[2..]));
    }
    // Numbered lists
    if line.len() > 2 && line.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        if let Some(rest) = line.split_once(". ") {
            return format!("  \x1b[36m{}.\x1b[0m {}", rest.0, render_inline(rest.1));
        }
    }

    render_inline(line)
}

/// Apply inline formatting: **bold**, *italic*, `code`
fn render_inline(text: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Inline code: `...`
        if chars[i] == '`' {
            if let Some(end) = find_closing(&chars, i + 1, '`') {
                let code: String = chars[i + 1..end].iter().collect();
                result.push_str(&format!("\x1b[48;5;236m\x1b[38;5;216m {}\x1b[0m", code));
                i = end + 1;
                continue;
            }
        }
        // Bold: **...**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_double_closing(&chars, i + 2, '*') {
                let bold: String = chars[i + 2..end].iter().collect();
                result.push_str(&format!("\x1b[1m{}\x1b[0m", bold));
                i = end + 2;
                continue;
            }
        }
        // Italic: *...*
        if chars[i] == '*' && (i + 1 < len && chars[i + 1] != '*') {
            if let Some(end) = find_closing(&chars, i + 1, '*') {
                let italic: String = chars[i + 1..end].iter().collect();
                result.push_str(&format!("\x1b[3m{}\x1b[0m", italic));
                i = end + 1;
                continue;
            }
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

fn find_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    for i in start..chars.len() {
        if chars[i] == marker {
            return Some(i);
        }
    }
    None
}

fn find_double_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    for i in start..chars.len().saturating_sub(1) {
        if chars[i] == marker && chars[i + 1] == marker {
            return Some(i);
        }
    }
    None
}

/// Get the visible character count (excluding ANSI escape codes)
fn visible_char_count(s: &str) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_empty_string() {
        let result = render_markdown("");
        // Empty input produces empty or whitespace-only output
        assert!(result.trim().is_empty());
    }

    #[test]
    fn test_render_plain_text() {
        let result = render_markdown("hello world");
        assert!(result.contains("hello world"));
    }

    #[test]
    fn test_render_header() {
        let result = render_markdown("# My Title");
        // Should contain ANSI codes for cyan
        assert!(result.contains("My Title"));
        assert!(result.contains("\x1b[1;36m"));
    }

    #[test]
    fn test_render_bullet_list() {
        let result = render_markdown("- item one\n- item two");
        assert!(result.contains("●"));
        assert!(result.contains("item one"));
    }

    #[test]
    fn test_render_code_block() {
        let result = render_markdown("```rust\nfn main() {}\n```");
        // Should contain code block border
        assert!(result.contains("╭"));
        assert!(result.contains("╰"));
        assert!(result.contains("rust"));
    }

    #[test]
    fn test_render_unclosed_code_block() {
        // Should not panic, should render the code
        let result = render_markdown("```python\nprint('hello')");
        assert!(result.contains("print"));
    }

    #[test]
    fn test_render_inline_code() {
        let result = render_markdown("use `cargo run` to start");
        assert!(result.contains("cargo run"));
    }

    #[test]
    fn test_render_bold() {
        let result = render_markdown("this is **bold** text");
        assert!(result.contains("bold"));
        assert!(result.contains("\x1b[1m"));
    }

    #[test]
    fn test_render_edge_case_double_star_only() {
        // Should not panic on "**" with nothing inside
        let result = render_markdown("**");
        assert!(result.contains("**"));
    }

    #[test]
    fn test_render_single_star() {
        // Should not panic
        let result = render_markdown("*");
        assert!(result.contains("*"));
    }

    #[test]
    fn test_visible_char_count_plain() {
        assert_eq!(visible_char_count("hello"), 5);
    }

    #[test]
    fn test_visible_char_count_with_ansi() {
        assert_eq!(visible_char_count("\x1b[1mhello\x1b[0m"), 5);
    }

    #[test]
    fn test_visible_char_count_empty() {
        assert_eq!(visible_char_count(""), 0);
    }

    #[test]
    fn test_find_double_closing_short_input() {
        // Should not panic on empty or short input
        assert_eq!(find_double_closing(&[], 0, '*'), None);
        assert_eq!(find_double_closing(&['*'], 0, '*'), None);
        assert_eq!(find_double_closing(&['*', '*'], 0, '*'), Some(0));
    }
}
