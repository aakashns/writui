//! The text being edited and the cursor, with the edits that can be made.
//!
//! The text always starts with the `# ` title prefix: nothing here removes
//! it or puts the cursor inside it.

use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;

use crate::vault::{TITLE_PREFIX, with_title_prefix};

/// The first position the cursor can be at: just after `# `.
pub const MIN: usize = TITLE_PREFIX.len();

pub struct Buffer {
    rope: Rope,
    /// A char index, always on a grapheme boundary and never before `MIN`.
    cursor: usize,
}

impl Buffer {
    /// The cursor starts at `cursor` (a char index), or the end of the text.
    pub fn new(text: &str, cursor: Option<usize>) -> Self {
        let rope = Rope::from_str(&with_title_prefix(&clean(text)));
        let mut buffer = Buffer { cursor: rope.len_chars(), rope };
        if let Some(pos) = cursor {
            buffer.set_cursor(pos);
            // A stored position could be inside a grapheme (if the text was
            // cleaned up above); move it to the end of that grapheme.
            buffer.cursor = buffer.next_boundary(buffer.prev_boundary(buffer.cursor));
        }
        buffer
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn len(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.clamp(MIN, self.len());
    }

    /// Type or paste `text` at the cursor.
    pub fn insert(&mut self, text: &str) {
        let text = clean(text);
        self.rope.insert(self.cursor, &text);
        self.cursor += text.chars().count();
    }

    /// Delete the grapheme before the cursor (joining lines at a line start).
    pub fn backspace(&mut self) {
        let start = self.prev_boundary(self.cursor).max(MIN);
        if start < self.cursor {
            self.rope.remove(start..self.cursor);
            self.cursor = start;
        }
    }

    /// Delete the grapheme after the cursor.
    pub fn delete(&mut self) {
        let end = self.next_boundary(self.cursor);
        if end > self.cursor {
            self.rope.remove(self.cursor..end);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.prev_boundary(self.cursor).max(MIN);
    }

    pub fn right(&mut self) {
        self.cursor = self.next_boundary(self.cursor);
    }

    /// The grapheme boundary before `pos` (the newline, at a line start).
    pub fn prev_boundary(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        let line = self.rope.char_to_line(pos);
        let start = self.rope.line_to_char(line);
        if pos == start {
            return pos - 1;
        }
        let offset = pos - start;
        let mut prev = 0;
        let mut end = 0;
        for g in self.line_text(line).graphemes(true) {
            end += g.chars().count();
            if end >= offset {
                break;
            }
            prev = end;
        }
        start + prev
    }

    /// The grapheme boundary after `pos` (past the newline, at a line end).
    pub fn next_boundary(&self, pos: usize) -> usize {
        if pos >= self.len() {
            return self.len();
        }
        let line = self.rope.char_to_line(pos);
        let start = self.rope.line_to_char(line);
        let offset = pos - start;
        let mut end = 0;
        for g in self.line_text(line).graphemes(true) {
            end += g.chars().count();
            if end > offset {
                return start + end;
            }
        }
        pos + 1
    }

    fn line_text(&self, line: usize) -> String {
        let mut text = self.rope.line(line).to_string();
        if text.ends_with('\n') {
            text.pop();
        }
        text
    }
}

/// Newlines as `\n` only, and no other control characters except tabs.
fn clean(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|&ch| ch == '\n' || ch == '\t' || !ch.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_at_a_stored_cursor() {
        let text = "# cafe\u{301}\nmore";
        assert_eq!(Buffer::new(text, Some(4)).cursor(), 4);
        assert_eq!(Buffer::new(text, Some(6)).cursor(), 7); // inside "é": after it
        assert_eq!(Buffer::new(text, Some(7)).cursor(), 7);
        assert_eq!(Buffer::new(text, Some(8)).cursor(), 8); // start of the next line
        assert_eq!(Buffer::new(text, Some(0)).cursor(), MIN);
        assert_eq!(Buffer::new(text, Some(999)).cursor(), 12);
    }

    #[test]
    fn title_prefix_cannot_be_removed() {
        let mut b = Buffer::new("# ", None);
        assert_eq!(b.cursor(), MIN);
        b.backspace();
        b.left();
        assert_eq!(b.text(), "# ");
        assert_eq!(b.cursor(), MIN);
        b.set_cursor(0);
        assert_eq!(b.cursor(), MIN);
        b.insert("Hi");
        b.backspace();
        b.backspace();
        b.backspace();
        assert_eq!(b.text(), "# ");
    }

    #[test]
    fn a_missing_prefix_is_added() {
        assert_eq!(Buffer::new("", None).text(), "# ");
        assert_eq!(Buffer::new("Title", None).text(), "# Title");
        assert_eq!(Buffer::new("#Title", None).text(), "# Title");
    }

    #[test]
    fn typing_and_deleting() {
        let mut b = Buffer::new("# Title", None);
        b.insert("\nHello");
        assert_eq!(b.text(), "# Title\nHello");
        b.set_cursor(8);
        b.backspace(); // joins the lines
        assert_eq!(b.text(), "# TitleHello");
        assert_eq!(b.cursor(), 7);
        b.delete();
        assert_eq!(b.text(), "# Titleello");
        b.set_cursor(b.len());
        b.delete(); // nothing after the end
        assert_eq!(b.text(), "# Titleello");
    }

    #[test]
    fn pasted_text_is_cleaned() {
        let mut b = Buffer::new("# ", None);
        b.insert("one\r\ntwo\rthree\u{7}\tfour");
        assert_eq!(b.text(), "# one\ntwo\nthree\tfour");
    }

    #[test]
    fn moves_by_grapheme() {
        // "e" + combining accent, and a flag made of two chars.
        let mut b = Buffer::new("# cafe\u{301}\n🇮🇳", None);
        b.left();
        assert_eq!(b.cursor(), 8);
        b.left(); // over the newline
        assert_eq!(b.cursor(), 7);
        b.left(); // over "é" (two chars)
        assert_eq!(b.cursor(), 5);
        b.right();
        b.right();
        b.right();
        assert_eq!(b.cursor(), 10);
        b.backspace(); // the whole flag
        assert_eq!(b.text(), "# cafe\u{301}\n");
    }
}
