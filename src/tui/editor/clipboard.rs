//! The system clipboard. If it can't be reached (no display, say), copying
//! and pasting still work within writui, from the last text copied.

use std::sync::Mutex;

/// Kept for the whole run: on Linux the clipboard's contents are served by
/// the program that copied them.
static SYSTEM: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

#[derive(Default)]
pub struct Clipboard {
    /// The last text copied in writui.
    held: String,
}

impl Clipboard {
    pub fn copy(&mut self, text: &str) {
        self.held = text.to_string();
        with_system(|system| system.set_text(text).ok());
    }

    /// What's on the clipboard now.
    pub fn paste(&self) -> String {
        with_system(|system| system.get_text().ok())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| self.held.clone())
    }
}

/// Tests never touch the real clipboard.
fn with_system<T>(f: impl FnOnce(&mut arboard::Clipboard) -> Option<T>) -> Option<T> {
    if cfg!(test) {
        return None;
    }
    let mut guard = SYSTEM.lock().ok()?;
    if guard.is_none() {
        *guard = arboard::Clipboard::new().ok();
    }
    f(guard.as_mut()?)
}
