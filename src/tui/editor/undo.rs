//! Undo and redo. Each step keeps the whole buffer (text and cursor) from
//! before an edit; ropes share their unchanged parts, so that's cheap.
//!
//! Typing is grouped into one step per word (with the spaces after it), and
//! runs of Backspace or Delete into one step each. Moving the cursor
//! between edits starts a new step.

use super::buffer::Buffer;

/// Steps kept for undo; the oldest are dropped past this.
const LIMIT: usize = 1000;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// Typing a character; `space` if it's whitespace.
    Type { space: bool },
    Backspace,
    Delete,
    /// Anything else (Enter, Tab, paste) is always a step of its own.
    Other,
}

#[derive(Default)]
pub struct Undo {
    undo: Vec<Buffer>,
    redo: Vec<Buffer>,
    /// The last edit, and where it left the cursor.
    last: Option<(Kind, usize)>,
}

impl Undo {
    /// Record an edit of `kind` that changed `before` into `after`.
    pub fn edited(&mut self, kind: Kind, before: Buffer, after: &Buffer) {
        if !self.continues(kind, before.cursor()) {
            if self.undo.len() == LIMIT {
                self.undo.remove(0);
            }
            self.undo.push(before);
        }
        self.redo.clear();
        self.last = Some((kind, after.cursor()));
    }

    /// The buffer from before the last step, if any; `current` becomes
    /// what redo goes back to.
    pub fn undo(&mut self, current: Buffer) -> Option<Buffer> {
        let prev = self.undo.pop()?;
        self.redo.push(current);
        self.last = None;
        Some(prev)
    }

    pub fn redo(&mut self, current: Buffer) -> Option<Buffer> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        self.last = None;
        Some(next)
    }

    /// Whether an edit of `kind` at `cursor` belongs to the current step.
    fn continues(&self, kind: Kind, cursor: usize) -> bool {
        let Some((last, at)) = self.last else {
            return false;
        };
        if at != cursor {
            return false;
        }
        match (last, kind) {
            // A new word starts a new step.
            (Kind::Type { space: true }, Kind::Type { space: false }) => false,
            (Kind::Type { .. }, Kind::Type { .. }) => true,
            (Kind::Backspace, Kind::Backspace) | (Kind::Delete, Kind::Delete) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A buffer and its history, editing the way the editor does.
    struct Doc {
        buffer: Buffer,
        history: Undo,
    }

    impl Doc {
        fn new(text: &str) -> Self {
            Doc { buffer: Buffer::new(text, None), history: Undo::default() }
        }

        fn edit(&mut self, kind: Kind, f: impl FnOnce(&mut Buffer)) {
            let before = self.buffer.clone();
            f(&mut self.buffer);
            self.history.edited(kind, before, &self.buffer);
        }

        fn typed(&mut self, text: &str) {
            for ch in text.chars() {
                let kind = Kind::Type { space: ch.is_whitespace() };
                self.edit(kind, |b| b.insert(ch.encode_utf8(&mut [0; 4])));
            }
        }

        fn undo(&mut self) -> String {
            if let Some(prev) = self.history.undo(self.buffer.clone()) {
                self.buffer = prev;
            }
            self.buffer.text()
        }

        fn redo(&mut self) -> String {
            if let Some(next) = self.history.redo(self.buffer.clone()) {
                self.buffer = next;
            }
            self.buffer.text()
        }
    }

    #[test]
    fn typing_undoes_a_word_at_a_time() {
        let mut d = Doc::new("# ");
        d.typed("Hello there world");
        assert_eq!(d.undo(), "# Hello there ");
        assert_eq!(d.undo(), "# Hello ");
        assert_eq!(d.undo(), "# ");
        assert_eq!(d.undo(), "# "); // nothing left
        assert_eq!(d.redo(), "# Hello ");
        assert_eq!(d.buffer.cursor(), 8);
        assert_eq!(d.redo(), "# Hello there ");
        assert_eq!(d.redo(), "# Hello there world");
        assert_eq!(d.redo(), "# Hello there world"); // nothing left
    }

    #[test]
    fn undo_puts_the_cursor_back() {
        let mut d = Doc::new("# Title\nbody");
        d.buffer.set_cursor(7);
        d.typed("d");
        d.buffer.set_cursor(d.buffer.len());
        d.undo();
        assert_eq!(d.buffer.text(), "# Title\nbody");
        assert_eq!(d.buffer.cursor(), 7);
    }

    #[test]
    fn deleting_is_one_step_per_run() {
        let mut d = Doc::new("# one two");
        d.edit(Kind::Backspace, Buffer::backspace);
        d.edit(Kind::Backspace, Buffer::backspace);
        d.edit(Kind::Backspace, Buffer::backspace);
        d.typed("X");
        assert_eq!(d.buffer.text(), "# one X");
        assert_eq!(d.undo(), "# one ");
        assert_eq!(d.undo(), "# one two");
    }

    #[test]
    fn moving_the_cursor_starts_a_new_step() {
        let mut d = Doc::new("# ab");
        d.typed("c");
        d.buffer.set_cursor(2);
        d.typed("x");
        assert_eq!(d.undo(), "# abc");
        assert_eq!(d.undo(), "# ab");
    }

    #[test]
    fn enter_and_paste_are_steps_of_their_own() {
        let mut d = Doc::new("# ");
        d.typed("Hi");
        d.edit(Kind::Other, |b| b.insert("\n"));
        d.typed("there");
        d.edit(Kind::Other, |b| b.insert(" pasted text"));
        assert_eq!(d.undo(), "# Hi\nthere");
        assert_eq!(d.undo(), "# Hi\n");
        assert_eq!(d.undo(), "# Hi");
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut d = Doc::new("# ");
        d.typed("one ");
        d.typed("two");
        d.undo();
        d.typed("three");
        assert_eq!(d.redo(), "# one three");
        assert_eq!(d.undo(), "# one ");
    }
}
