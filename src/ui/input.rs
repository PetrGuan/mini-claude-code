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
const PASTE_COLLAPSE_LINES: usize = 5;

/// A line editor that tracks cursor position within the current line.
struct LineEditor {
    lines: Vec<String>,
    cursor: usize,
    consecutive_newlines: usize,
    displayed_lines: usize,
    /// Stored pasted content with unique IDs
    pasted: Vec<(usize, String)>,
    next_paste_id: usize,
}

impl LineEditor {
    fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: 0,
            consecutive_newlines: 0,
            displayed_lines: 1,
            pasted: Vec::new(),
            next_paste_id: 1,
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
        self.displayed_lines = self.lines.len();
        self.cursor = 0;
        false
    }

    /// Find the placeholder span (start, end) that contains or touches `pos`.
    /// Returns None if pos is not inside a placeholder.
    fn placeholder_span_at(&self, pos: usize) -> Option<(usize, usize, usize)> {
        let line = self.current_line();
        for &(id, _) in &self.pasted {
            // Reconstruct placeholder for this ID
            // We need the line count, look it up
            if let Some((_, content)) = self.pasted.iter().find(|(pid, _)| *pid == id) {
                let lc = content.split('\n').count();
                let placeholder = format!("[Pasted#{}: {} lines]", id, lc);
                if let Some(start) = line.find(&placeholder) {
                    let end = start + placeholder.len();
                    if pos > start && pos <= end {
                        return Some((start, end, id));
                    }
                }
            }
        }
        None
    }

    fn backspace(&mut self) -> bool {
        let is_current_empty = self.current_line().is_empty();

        if is_current_empty && self.lines.len() > 1 {
            self.lines.pop();
            self.cursor = self.current_line().len();
            self.consecutive_newlines = 0;
            self.displayed_lines = self.lines.len();
            return true; // went up a line
        }

        if !is_current_empty && self.cursor > 0 {
            // Check if cursor is inside a placeholder — delete the whole thing
            if let Some((start, end, id)) = self.placeholder_span_at(self.cursor) {
                let line = self.lines.last_mut().unwrap();
                line.replace_range(start..end, "");
                self.cursor = start;
                self.pasted.retain(|(pid, _)| *pid != id);
            } else {
                let line = self.lines.last_mut().unwrap();
                let new_cursor = line[..self.cursor]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                line.replace_range(new_cursor..self.cursor, "");
                self.cursor = new_cursor;
            }
            self.consecutive_newlines = 0;
        }
        false
    }

    fn delete_at_cursor(&mut self) {
        let len = self.current_line().len();
        if self.cursor < len {
            // Check if cursor is at the start of a placeholder
            if let Some((start, end, id)) = self.placeholder_span_at(self.cursor + 1) {
                if self.cursor == start {
                    let line = self.lines.last_mut().unwrap();
                    line.replace_range(start..end, "");
                    self.pasted.retain(|(pid, _)| *pid != id);
                    return;
                }
            }
            let line = self.lines.last_mut().unwrap();
            let next = line[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(line.len());
            line.replace_range(self.cursor..next, "");
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            // If inside a placeholder, jump to its start
            if let Some((start, _, _)) = self.placeholder_span_at(self.cursor) {
                self.cursor = start;
            } else {
                let line = self.current_line();
                self.cursor = line[..self.cursor]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
            }
        }
    }

    fn move_right(&mut self) {
        let len = self.current_line().len();
        if self.cursor < len {
            // If at the start of a placeholder, jump to its end
            if let Some((start, end, _)) = self.placeholder_span_at(self.cursor + 1) {
                if self.cursor == start {
                    self.cursor = end;
                    return;
                }
            }
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

    fn kill_to_end(&mut self) {
        let line = self.lines.last_mut().unwrap();
        line.truncate(self.cursor);
    }

    fn clear_line(&mut self) {
        self.lines.last_mut().unwrap().clear();
        self.cursor = 0;
    }

    fn delete_word(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let line = self.lines.last_mut().unwrap();
        let before = &line[..self.cursor];
        // Find the start of the previous word
        let trimmed = before.trim_end();
        let new_cursor = if trimmed.is_empty() {
            0
        } else {
            // Find last whitespace, then advance past it (char-aware)
            match trimmed.rfind(|c: char| c.is_whitespace()) {
                Some(byte_idx) => {
                    // Advance past the whitespace character
                    let ws_char = trimmed[byte_idx..].chars().next().unwrap();
                    byte_idx + ws_char.len_utf8()
                }
                None => 0,
            }
        };
        line.replace_range(new_cursor..self.cursor, "");
        self.cursor = new_cursor;
    }

    /// Insert pasted text. Long pastes (>5 lines) are collapsed to a placeholder.
    fn insert_paste(&mut self, text: &str) {
        self.consecutive_newlines = 0;
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let line_count = normalized.split('\n').count();

        if line_count > PASTE_COLLAPSE_LINES {
            // Store full content with unique ID, insert a placeholder
            let id = self.next_paste_id;
            self.next_paste_id += 1;
            let placeholder = format!("[Pasted#{}: {} lines]", id, line_count);
            self.pasted.push((id, normalized));
            for c in placeholder.chars() {
                self.insert_char(c);
            }
        } else {
            // Short paste — insert inline
            for (i, part) in normalized.split('\n').enumerate() {
                if i > 0 {
                    self.lines.push(String::new());
                    self.cursor = 0;
                }
                for c in part.chars() {
                    self.insert_char(c);
                }
            }
        }
        self.displayed_lines = self.lines.len();
    }

    /// Get full text with paste placeholders expanded back to original content.
    /// If a placeholder was edited/deleted, appends the content to avoid data loss.
    fn full_text(&self) -> String {
        let raw = self.lines.join("\n");
        if self.pasted.is_empty() {
            return raw;
        }
        let mut result = raw;
        for (id, content) in self.pasted.iter() {
            let line_count = content.split('\n').count();
            let placeholder = format!("[Pasted#{}: {} lines]", id, line_count);
            if result.contains(&placeholder) {
                result = result.replacen(&placeholder, content, 1);
            } else {
                // Placeholder was edited away — append content to avoid data loss
                result.push('\n');
                result.push_str(content);
            }
        }
        result
    }
}

/// Read user input from the terminal.
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
            Event::Paste(text) => {
                let old_displayed = editor.displayed_lines;
                editor.insert_paste(&text);
                redraw_all(&editor, old_displayed);
            }

            Event::Key(key_event) => match key_event {
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
                } if editor.is_empty() => {
                    println!();
                    return None;
                }

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

                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    let went_up = editor.backspace();
                    if went_up {
                        // Clear current (now-empty) line, move up, redraw previous line
                        let line = editor.current_line().to_string();
                        let prefix = line_prefix(editor.lines.len() == 1);
                        // Erase the empty line we're on, move up, redraw
                        print!("\x1b[2K\x1b[A\r\x1b[2K{}{}", prefix, line);
                        position_cursor(&editor);
                        io::stdout().flush().ok();
                    } else {
                        redraw_current_line(&editor);
                    }
                }

                KeyEvent {
                    code: KeyCode::Delete,
                    ..
                } => {
                    editor.delete_at_cursor();
                    redraw_current_line(&editor);
                }

                KeyEvent {
                    code: KeyCode::Left,
                    ..
                } => {
                    editor.move_left();
                    position_cursor(&editor);
                    io::stdout().flush().ok();
                }

                KeyEvent {
                    code: KeyCode::Right,
                    ..
                } => {
                    editor.move_right();
                    position_cursor(&editor);
                    io::stdout().flush().ok();
                }

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

                KeyEvent {
                    code: KeyCode::Char('u'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    editor.clear_line();
                    redraw_current_line(&editor);
                }

                KeyEvent {
                    code: KeyCode::Char('w'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    editor.delete_word();
                    redraw_current_line(&editor);
                }

                KeyEvent {
                    code: KeyCode::Char('k'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    editor.kill_to_end();
                    redraw_current_line(&editor);
                }

                KeyEvent {
                    code: KeyCode::Char(c),
                    ..
                } => {
                    let at_end = editor.cursor >= editor.current_line().len();
                    editor.insert_char(c);
                    if at_end {
                        print!("{}", c);
                        io::stdout().flush().ok();
                    } else {
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

fn redraw_current_line(editor: &LineEditor) {
    let line = editor.current_line();
    let prefix = line_prefix(editor.lines.len() == 1);
    print!("\r\x1b[2K{}{}", prefix, line);
    position_cursor(editor);
    io::stdout().flush().ok();
}

/// Redraw all lines. `old_displayed` is how many lines were on screen before.
fn redraw_all(editor: &LineEditor, old_displayed: usize) {
    // Move up to the first displayed line
    let up = old_displayed.saturating_sub(1);
    if up > 0 {
        print!("\x1b[{}A", up);
    }
    print!("\r");

    // Clear old lines and print new ones
    for (i, line) in editor.lines.iter().enumerate() {
        let prefix = line_prefix(i == 0);
        print!("\x1b[2K{}{}", prefix, line);
        if i < editor.lines.len() - 1 {
            print!("\r\n");
        }
    }
    // Clear any leftover lines from before
    let extra = old_displayed.saturating_sub(editor.lines.len());
    for _ in 0..extra {
        print!("\r\n\x1b[2K");
    }
    if extra > 0 {
        // Move back up to the last editor line
        print!("\x1b[{}A", extra);
    }

    position_cursor(editor);
    io::stdout().flush().ok();
}

fn position_cursor(editor: &LineEditor) {
    let prefix_width = if editor.lines.len() == 1 {
        display_width(PROMPT)
    } else {
        display_width(CONTINUATION)
    };
    let text_before_cursor = &editor.current_line()[..editor.cursor];
    let col = prefix_width + display_width(text_before_cursor);
    if col == 0 {
        print!("\r");
    } else {
        print!("\r\x1b[{}C", col);
    }
}

fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            if c.is_ascii() {
                1
            } else {
                // Rough approximation: CJK fullwidth = 2, others = 1
                // A proper solution would use the unicode-width crate
                if is_wide_char(c) {
                    2
                } else {
                    1
                }
            }
        })
        .sum()
}

/// Check if a character is likely a wide (fullwidth) character.
fn is_wide_char(c: char) -> bool {
    let cp = c as u32;
    // CJK Unified Ideographs
    (0x4E00..=0x9FFF).contains(&cp)
    // CJK Extension A-F
    || (0x3400..=0x4DBF).contains(&cp)
    || (0x20000..=0x2A6DF).contains(&cp)
    // CJK Compatibility Ideographs
    || (0xF900..=0xFAFF).contains(&cp)
    // Fullwidth Forms
    || (0xFF01..=0xFF60).contains(&cp)
    || (0xFFE0..=0xFFE6).contains(&cp)
    // Hangul Syllables
    || (0xAC00..=0xD7AF).contains(&cp)
    // Katakana / Hiragana
    || (0x3040..=0x309F).contains(&cp)
    || (0x30A0..=0x30FF).contains(&cp)
    // Emoji (common ranges)
    || (0x1F600..=0x1F64F).contains(&cp)
    || (0x1F300..=0x1F5FF).contains(&cp)
    || (0x1F680..=0x1F6FF).contains(&cp)
    || (0x1F900..=0x1F9FF).contains(&cp)
    || (0x2600..=0x26FF).contains(&cp)
    || (0x2700..=0x27BF).contains(&cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_at_end() {
        let mut ed = LineEditor::new();
        ed.insert_char('a');
        ed.insert_char('b');
        assert_eq!(ed.current_line(), "ab");
        assert_eq!(ed.cursor, 2);
    }

    #[test]
    fn test_insert_at_middle() {
        let mut ed = LineEditor::new();
        ed.insert_char('a');
        ed.insert_char('c');
        ed.cursor = 1; // between a and c
        ed.insert_char('b');
        assert_eq!(ed.current_line(), "abc");
        assert_eq!(ed.cursor, 2);
    }

    #[test]
    fn test_backspace_at_end() {
        let mut ed = LineEditor::new();
        ed.insert_char('a');
        ed.insert_char('b');
        ed.backspace();
        assert_eq!(ed.current_line(), "a");
        assert_eq!(ed.cursor, 1);
    }

    #[test]
    fn test_backspace_at_start_does_nothing() {
        let mut ed = LineEditor::new();
        ed.insert_char('a');
        ed.cursor = 0;
        ed.backspace();
        assert_eq!(ed.current_line(), "a");
        assert_eq!(ed.cursor, 0);
    }

    #[test]
    fn test_backspace_joins_lines() {
        let mut ed = LineEditor::new();
        ed.insert_char('a');
        ed.newline();
        // Now on empty second line
        let went_up = ed.backspace();
        assert!(went_up);
        assert_eq!(ed.lines.len(), 1);
        assert_eq!(ed.current_line(), "a");
    }

    #[test]
    fn test_delete_at_cursor() {
        let mut ed = LineEditor::new();
        ed.insert_char('a');
        ed.insert_char('b');
        ed.insert_char('c');
        ed.cursor = 1;
        ed.delete_at_cursor();
        assert_eq!(ed.current_line(), "ac");
        assert_eq!(ed.cursor, 1);
    }

    #[test]
    fn test_paste_short_inline() {
        let mut ed = LineEditor::new();
        ed.insert_paste("line1\r\nline2\r\nline3");
        assert_eq!(ed.lines.len(), 3);
        assert_eq!(ed.lines[0], "line1");
        assert_eq!(ed.lines[1], "line2");
        assert_eq!(ed.lines[2], "line3");
        assert!(ed.pasted.is_empty());
    }

    #[test]
    fn test_paste_empty() {
        let mut ed = LineEditor::new();
        ed.insert_paste("");
        assert_eq!(ed.lines.len(), 1);
        assert_eq!(ed.current_line(), "");
    }

    #[test]
    fn test_paste_long_collapsed() {
        let mut ed = LineEditor::new();
        let long_text = (1..=10).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        ed.insert_paste(&long_text);
        assert_eq!(ed.lines.len(), 1);
        assert!(ed.current_line().contains("[Pasted#1: 10 lines]"));
        assert_eq!(ed.pasted.len(), 1);
        let full = ed.full_text();
        assert!(full.contains("line 1"));
        assert!(full.contains("line 10"));
    }

    #[test]
    fn test_paste_exactly_threshold_is_inline() {
        let mut ed = LineEditor::new();
        let text = (1..=5).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        ed.insert_paste(&text);
        assert_eq!(ed.lines.len(), 5);
        assert!(ed.pasted.is_empty());
    }

    #[test]
    fn test_two_pastes_same_length_unique_ids() {
        let mut ed = LineEditor::new();
        let text_a = (1..=8).map(|i| format!("a{}", i)).collect::<Vec<_>>().join("\n");
        let text_b = (1..=8).map(|i| format!("b{}", i)).collect::<Vec<_>>().join("\n");
        ed.insert_paste(&text_a);
        ed.insert_char(' ');
        ed.insert_paste(&text_b);
        assert!(ed.current_line().contains("[Pasted#1:"));
        assert!(ed.current_line().contains("[Pasted#2:"));
        let full = ed.full_text();
        assert!(full.contains("a1"));
        assert!(full.contains("b1"));
    }

    #[test]
    fn test_paste_backspace_deletes_whole_placeholder() {
        let mut ed = LineEditor::new();
        let long_text = (1..=10).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        ed.insert_paste(&long_text);
        assert_eq!(ed.pasted.len(), 1);
        // Cursor is at end of placeholder, backspace should delete the whole thing
        ed.backspace();
        assert_eq!(ed.current_line(), "");
        assert!(ed.pasted.is_empty());
    }

    #[test]
    fn test_paste_placeholder_fallback_appends() {
        let mut ed = LineEditor::new();
        let long_text = (1..=10).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        ed.insert_paste(&long_text);
        // Forcefully corrupt the placeholder (simulating edge case)
        ed.lines[0] = "corrupted".to_string();
        ed.cursor = ed.lines[0].len();
        // full_text should append the content as fallback
        let full = ed.full_text();
        assert!(full.contains("corrupted"));
        assert!(full.contains("line 1"));
    }

    #[test]
    fn test_delete_word_basic() {
        let mut ed = LineEditor::new();
        for c in "hello world".chars() {
            ed.insert_char(c);
        }
        ed.delete_word();
        assert_eq!(ed.current_line(), "hello ");
    }

    #[test]
    fn test_delete_word_at_start() {
        let mut ed = LineEditor::new();
        ed.insert_char('a');
        ed.cursor = 0;
        ed.delete_word();
        assert_eq!(ed.current_line(), "a"); // no change
    }

    #[test]
    fn test_move_left_right() {
        let mut ed = LineEditor::new();
        ed.insert_char('a');
        ed.insert_char('b');
        ed.move_left();
        assert_eq!(ed.cursor, 1);
        ed.move_right();
        assert_eq!(ed.cursor, 2);
    }

    #[test]
    fn test_newline_submit_on_double() {
        let mut ed = LineEditor::new();
        assert!(!ed.newline()); // first Enter
        assert!(ed.newline()); // second Enter → submit
    }

    #[test]
    fn test_consecutive_newlines_reset_on_char() {
        let mut ed = LineEditor::new();
        ed.newline();
        ed.insert_char('a');
        assert_eq!(ed.consecutive_newlines, 0);
        assert!(!ed.newline()); // reset, so this is first Enter again
    }

    #[test]
    fn test_utf8_insert_and_backspace() {
        let mut ed = LineEditor::new();
        ed.insert_char('你');
        ed.insert_char('好');
        assert_eq!(ed.current_line(), "你好");
        ed.backspace();
        assert_eq!(ed.current_line(), "你");
    }

    #[test]
    fn test_display_width_ascii() {
        assert_eq!(display_width("hello"), 5);
    }

    #[test]
    fn test_display_width_cjk() {
        assert_eq!(display_width("你好"), 4);
    }

    #[test]
    fn test_is_wide_char() {
        assert!(is_wide_char('你'));
        assert!(is_wide_char('好'));
        assert!(!is_wide_char('a'));
        assert!(!is_wide_char('é'));
    }
}
