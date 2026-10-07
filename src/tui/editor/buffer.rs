//! The text being edited and the cursor, with the edits that can be made.
//!
//! The text always starts with the `# ` title prefix: nothing here removes
//! it or puts the cursor inside it.

use std::ops::Range;

use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;

use crate::vault::{TITLE_PREFIX, with_title_prefix};

/// The first position the cursor can be at: just after `# `.
pub const MIN: usize = TITLE_PREFIX.len();

#[derive(Clone)]
pub struct Buffer {
    rope: Rope,
    /// A char index, always on a grapheme boundary and never before `MIN`.
    cursor: usize,
    /// Where a selection started; the selection runs between it and the
    /// cursor. Only a selection while it differs from the cursor.
    anchor: Option<usize>,
}

impl Buffer {
    /// The cursor starts at `cursor` (a char index), or the end of the text.
    pub fn new(text: &str, cursor: Option<usize>) -> Self {
        let rope = Rope::from_str(&with_title_prefix(&clean(text)));
        let mut buffer = Buffer { cursor: rope.len_chars(), rope, anchor: None };
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

    /// The selected chars, if any.
    pub fn selection(&self) -> Option<Range<usize>> {
        let anchor = self.anchor.filter(|&a| a != self.cursor)?;
        Some(anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|range| self.rope.slice(range).to_string())
    }

    /// Start selecting from the cursor, unless a selection is under way.
    /// Moving the cursor then extends it; call `settle` afterwards.
    pub fn begin_select(&mut self) {
        self.anchor.get_or_insert(self.cursor);
    }

    /// An empty selection is no selection.
    pub fn settle(&mut self) {
        if self.anchor == Some(self.cursor) {
            self.anchor = None;
        }
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Select `from..to`, with the cursor at `to`.
    pub fn select(&mut self, from: usize, to: usize) {
        self.set_cursor(to);
        self.anchor = Some(from.clamp(MIN, self.len()));
        self.settle();
    }

    pub fn select_all(&mut self) {
        self.select(MIN, self.len());
    }

    /// Delete the selection, if any, leaving the cursor where it was.
    pub fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection() else { return false };
        self.anchor = None;
        self.cursor = range.start;
        self.rope.remove(range);
        true
    }

    /// Type or paste `text` at the cursor, over the selection.
    pub fn insert(&mut self, text: &str) {
        self.delete_selection();
        self.anchor = None;
        let text = clean(text);
        self.rope.insert(self.cursor, &text);
        self.cursor += text.chars().count();
    }

    /// Delete the selection, or the grapheme before the cursor (joining
    /// lines at a line start).
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.anchor = None;
        let start = self.prev_boundary(self.cursor).max(MIN);
        if start < self.cursor {
            self.rope.remove(start..self.cursor);
            self.cursor = start;
        }
    }

    /// Delete the selection, or the grapheme after the cursor.
    pub fn delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.anchor = None;
        let end = self.next_boundary(self.cursor);
        if end > self.cursor {
            self.rope.remove(self.cursor..end);
        }
    }

    /// Move to the start of the word before the cursor.
    pub fn word_left(&mut self) {
        let mut i = self.cursor;
        while i > MIN && !is_word(self.rope.char(i - 1)) {
            i -= 1;
        }
        while i > MIN && is_word(self.rope.char(i - 1)) {
            i -= 1;
        }
        self.cursor = self.snap(i);
    }

    /// Move to the end of the word after the cursor.
    pub fn word_right(&mut self) {
        let mut i = self.cursor;
        while i < self.len() && !is_word(self.rope.char(i)) {
            i += 1;
        }
        while i < self.len() && is_word(self.rope.char(i)) {
            i += 1;
        }
        self.cursor = self.snap(i);
    }

    /// Move to the start of the paragraph (line) the cursor is in, or if
    /// it's there already, of the one before, skipping blank lines.
    pub fn paragraph_left(&mut self) {
        let mut line = self.rope.char_to_line(self.cursor);
        if self.cursor == self.rope.line_to_char(line) {
            line = line.saturating_sub(1);
            while line > 0 && self.line_text(line).is_empty() {
                line -= 1;
            }
        }
        self.cursor = self.rope.line_to_char(line).max(MIN);
    }

    /// Move to the end of the paragraph (line) the cursor is in, or if
    /// it's there already, of the one after, skipping blank lines.
    pub fn paragraph_right(&mut self) {
        let mut line = self.rope.char_to_line(self.cursor);
        if self.cursor == self.line_end(line) {
            line += 1;
            while line < self.rope.len_lines() && self.line_text(line).is_empty() {
                line += 1;
            }
        }
        self.cursor = if line < self.rope.len_lines() { self.line_end(line) } else { self.len() };
    }

    /// The chars of the word at `pos` (or the run of spaces or punctuation
    /// there), for a double-click.
    pub fn word_at(&self, pos: usize) -> Range<usize> {
        let line = self.rope.char_to_line(pos);
        let (start, end) = (self.rope.line_to_char(line), self.line_end(line));
        let Some(probe) = (pos < end).then_some(pos).or(pos.checked_sub(1).filter(|&p| p >= start)) else {
            return pos..pos;
        };
        let class = is_word(self.rope.char(probe));
        let mut from = probe;
        while from > start && is_word(self.rope.char(from - 1)) == class {
            from -= 1;
        }
        let mut to = probe + 1;
        while to < end && is_word(self.rope.char(to)) == class {
            to += 1;
        }
        self.snap(from).max(MIN)..self.snap(to)
    }

    /// The chars of the line (paragraph) at `pos`, without its newline.
    pub fn line_at(&self, pos: usize) -> Range<usize> {
        let line = self.rope.char_to_line(pos);
        self.rope.line_to_char(line).max(MIN)..self.line_end(line)
    }

    /// Where a line ends, before its newline.
    fn line_end(&self, line: usize) -> usize {
        self.rope.line_to_char(line) + self.line_text(line).chars().count()
    }

    /// Move `pos`, if it's inside a grapheme, to the end of it.
    fn snap(&self, pos: usize) -> usize {
        if pos == 0 { 0 } else { self.next_boundary(self.prev_boundary(pos)) }
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

/// Letters, digits and the like (and the marks and apostrophes inside
/// words), as opposed to spaces and punctuation.
fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '\'' | '’' | '\u{300}'..='\u{36f}')
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

    #[test]
    fn selecting_and_replacing() {
        let mut b = Buffer::new("# Title\nHello world", None);
        b.set_cursor(8);
        b.begin_select();
        b.set_cursor(13);
        b.settle();
        assert_eq!(b.selection(), Some(8..13));
        assert_eq!(b.selected_text().as_deref(), Some("Hello"));
        b.insert("Bye");
        assert_eq!(b.text(), "# Title\nBye world");
        assert_eq!(b.selection(), None);
        b.select_all();
        assert_eq!(b.selection(), Some(MIN..b.len())); // never takes the "# "
        b.backspace();
        assert_eq!(b.text(), "# ");
        // Selecting nothing selects nothing, and a stale anchor is dropped.
        b.insert("ab");
        b.begin_select();
        b.settle();
        assert_eq!(b.selection(), None);
        b.insert("c");
        b.left();
        assert_eq!(b.selection(), None);
    }

    #[test]
    fn moves_by_word_and_paragraph() {
        let mut b = Buffer::new("# One two\n\nit's, three\nlast", None);
        b.set_cursor(0);
        b.word_right();
        assert_eq!(b.cursor(), 5); // "# One"
        b.word_right();
        assert_eq!(b.cursor(), 9);
        b.word_right(); // over the blank line
        assert_eq!(b.cursor(), 15); // "it's"
        b.word_left();
        assert_eq!(b.cursor(), 11);
        b.word_left();
        assert_eq!(b.cursor(), 6);
        b.word_left();
        b.word_left();
        assert_eq!(b.cursor(), MIN);

        b.paragraph_right();
        assert_eq!(b.cursor(), 9); // end of the title line
        b.paragraph_right(); // skips the blank line
        assert_eq!(b.cursor(), 22);
        b.paragraph_left();
        assert_eq!(b.cursor(), 11);
        b.paragraph_left();
        assert_eq!(b.cursor(), MIN);
    }

    #[test]
    fn words_and_lines_at_a_position() {
        let b = Buffer::new("# One  two!\nnext", None);
        assert_eq!(b.word_at(3), 2..5); // "One"
        assert_eq!(b.word_at(5), 5..7); // the spaces
        assert_eq!(b.word_at(10), 10..11); // "!"
        assert_eq!(b.word_at(11), 10..11); // end of line: the word before
        assert_eq!(b.line_at(4), MIN..11);
        assert_eq!(b.line_at(14), 12..16);
    }
}
