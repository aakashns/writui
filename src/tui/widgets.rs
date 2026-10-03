//! Small shared pieces: layout helpers and the password field.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use unicode_width::UnicodeWidthChar;
use zeroize::Zeroizing;

/// The comfortable reading/writing width, in columns.
pub const COLUMN_WIDTH: u16 = 68;

/// Shown on the setup and unlock screens of debug builds, so the shared
/// password for `.dev-data/` vaults is never forgotten.
pub fn dev_password_note() -> Option<&'static str> {
    cfg!(debug_assertions).then_some("Dev build · dev vault password: writui-dev")
}

/// A column of at most `width`, horizontally centred in `area`.
pub fn column(area: Rect, width: u16) -> Rect {
    let width = width.min(area.width);
    Rect::new(area.x + (area.width - width) / 2, area.y, width, area.height)
}

/// A `width` x `height` box centred in `area` (clamped to fit).
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

/// Cut `text` to fit in `width` columns, ending with `…` if it was cut.
pub fn truncate(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > width {
            if width > 0 {
                while used + 1 > width {
                    used -= out.pop().and_then(|c| c.width()).unwrap_or(0);
                }
                out.push('…');
            }
            return out;
        }
        used += w;
        out.push(ch);
    }
    out
}

/// A masked text input whose contents are wiped from memory when dropped.
#[derive(Default)]
pub struct PasswordField {
    value: Zeroizing<String>,
}

impl PasswordField {
    /// Apply a key press. Returns true if the field changed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.clear();
                true
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value.push(ch);
                true
            }
            KeyCode::Backspace => self.value.pop().is_some(),
            _ => false,
        }
    }

    pub fn paste(&mut self, text: &str) {
        self.value.push_str(text.trim_end_matches(['\r', '\n']));
    }

    pub fn clear(&mut self) {
        self.value = Zeroizing::default();
    }

    pub fn value(&self) -> &Zeroizing<String> {
        &self.value
    }

    pub fn len(&self) -> usize {
        self.value.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn masked(&self) -> String {
        "•".repeat(self.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_fits_width() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 6), "hello…");
        assert_eq!(truncate("hello", 0), "");
        assert_eq!(truncate("日本語です", 5), "日本…");
    }
}
