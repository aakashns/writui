//! The system clipboard. If it can't be reached (no display, say), copying
//! and pasting still work within writui, from the last text copied.
//!
//! Selected text goes on the clipboard as soon as it's selected, so the
//! terminal's own copy key (Cmd+C on a Mac, which writui never sees, and
//! which can't see writui's selection) has nothing left to do. Until it's
//! copied for real, that's provisional: typing, deleting or pasting over the
//! selection puts back what was on the clipboard before, and pasting over
//! it pastes that, not the selection itself.

#[cfg(not(test))]
/// Kept for the whole run: on Linux the clipboard's contents are served by
/// the program that copied them.
static SYSTEM: std::sync::Mutex<Option<arboard::Clipboard>> = std::sync::Mutex::new(None);

#[derive(Default)]
pub struct Clipboard {
    /// The last text copied in writui.
    held: String,
    /// The selection, put on the clipboard but not copied for real.
    selected: Option<Selected>,
}

struct Selected {
    text: String,
    /// What was on the clipboard before (`None`: nothing, or not text).
    before: Option<String>,
}

impl Clipboard {
    pub fn copy(&mut self, text: &str) {
        self.selected = None;
        self.held = text.to_string();
        system_set(text);
    }

    /// What's on the clipboard now.
    pub fn paste(&self) -> String {
        system_get().filter(|text| !text.is_empty()).unwrap_or_else(|| self.held.clone())
    }

    /// The text selected now (if any): put it on the clipboard. With no
    /// selection, the last one stays there, as if copied.
    pub fn select(&mut self, text: Option<&str>) {
        let Some(text) = text else {
            self.selected = None;
            return;
        };
        if self.selected.as_ref().is_some_and(|s| s.text == text) {
            return;
        }
        let now = system_get();
        // A selection that's grown or shrunk keeps what was there first,
        // unless something else has been copied since.
        let before = match self.selected.take() {
            Some(s) if now.as_deref() == Some(s.text.as_str()) => s.before,
            _ => now,
        };
        system_set(text);
        self.selected = Some(Selected { text: text.to_string(), before });
    }

    /// The selection was typed, deleted or pasted over: put back what was
    /// on the clipboard before it (unless something else was copied since).
    pub fn restore(&mut self) {
        let Some(selected) = self.selected.take() else { return };
        if system_get().as_deref() == Some(selected.text.as_str()) {
            match selected.before {
                Some(before) => system_set(&before),
                None => system_clear(),
            }
        }
    }

    /// What pasting `text` over the selection should put there: the
    /// terminal reads the clipboard, which holds the selection itself, so
    /// it's what was on the clipboard before it.
    pub fn over_selection(&self, text: String) -> String {
        match &self.selected {
            Some(s) if s.text == text => s.before.clone().unwrap_or(text),
            _ => text,
        }
    }
}

#[cfg(not(test))]
fn system_get() -> Option<String> {
    with_system(|system| system.get_text().ok())
}

#[cfg(not(test))]
fn system_set(text: &str) {
    with_system(|system| system.set_text(text).ok());
}

#[cfg(not(test))]
fn system_clear() {
    with_system(|system| system.clear().ok());
}

#[cfg(not(test))]
fn with_system<T>(f: impl FnOnce(&mut arboard::Clipboard) -> Option<T>) -> Option<T> {
    let mut guard = SYSTEM.lock().ok()?;
    if guard.is_none() {
        *guard = arboard::Clipboard::new().ok();
    }
    f(guard.as_mut()?)
}

// Tests never touch the real clipboard: each test thread has its own.
#[cfg(test)]
thread_local! {
    static FAKE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn system_get() -> Option<String> {
    FAKE.with(|fake| fake.borrow().clone())
}

#[cfg(test)]
fn system_set(text: &str) {
    FAKE.with(|fake| *fake.borrow_mut() = Some(text.to_string()));
}

#[cfg(test)]
fn system_clear() {
    FAKE.with(|fake| *fake.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selection_is_on_the_clipboard_until_it_is_typed_over() {
        let mut clipboard = Clipboard::default();
        system_set("url");
        clipboard.select(Some("wo"));
        clipboard.select(Some("word"));
        assert_eq!(system_get().as_deref(), Some("word"));
        // Pasting over it pastes what was there before.
        assert_eq!(clipboard.over_selection("word".into()), "url");
        assert_eq!(clipboard.over_selection("other".into()), "other");
        clipboard.restore();
        assert_eq!(system_get().as_deref(), Some("url"));
        // Deselected (the cursor moved on): it stays, as if copied.
        clipboard.select(Some("kept"));
        clipboard.select(None);
        clipboard.restore();
        assert_eq!(system_get().as_deref(), Some("kept"));
        // Copied elsewhere since: that's left alone.
        clipboard.select(Some("mine"));
        system_set("theirs");
        clipboard.restore();
        assert_eq!(system_get().as_deref(), Some("theirs"));
    }
}
