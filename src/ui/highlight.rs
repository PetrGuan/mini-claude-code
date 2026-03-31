use std::sync::LazyLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Syntax-highlight a code block and return the colored string.
/// Falls back to plain text if the language is unknown.
pub fn highlight_code(code: &str, lang: &str) -> String {
    let ss = &*SYNTAX_SET;
    let ts = &*THEME_SET;
    let theme = &ts.themes["base16-ocean.dark"];

    let syntax = ss
        .find_syntax_by_token(lang)
        .or_else(|| ss.find_syntax_by_extension(lang))
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let mut h = HighlightLines::new(syntax, theme);
    let mut output = String::new();

    for line in LinesWithEndings::from(code) {
        match h.highlight_line(line, &ss) {
            Ok(ranges) => {
                let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                output.push_str(&escaped);
            }
            Err(_) => {
                output.push_str(line);
            }
        }
    }

    // Reset colors at the end
    output.push_str("\x1b[0m");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust() {
        let code = "fn main() {\n    println!(\"hello\");\n}";
        let result = highlight_code(code, "rust");
        // Should contain ANSI escape codes
        assert!(result.contains("\x1b["));
        // Should contain the actual code
        assert!(result.contains("main"));
    }

    #[test]
    fn test_highlight_unknown_lang() {
        let code = "some plain text";
        let result = highlight_code(code, "nonexistent_lang_xyz");
        assert!(result.contains("some plain text"));
    }
}
