//! Small shared pieces: layout helpers and the password field.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use unicode_width::UnicodeWidthChar;
use zeroize::Zeroizing;

/// The comfortable reading/writing width, in columns.
pub const COLUMN_WIDTH: u16 = 68;

/// A boxed password field: wide enough for a long password, and two fit
/// side by side in the column.
pub const PASSWORD_BOX: (u16, u16) = (33, 3);

/// Shown on the setup and unlock screens of debug builds, so the shared
/// password for `.dev-data/` vaults is never forgotten.
pub fn dev_password_note() -> Option<&'static str> {
    cfg!(debug_assertions).then_some("Dev build · dev vault password: writui-dev")
}

/// How writui works, in a few lines, on the password screens: for new
/// people, and as a reminder.
pub fn overview() -> Vec<Line<'static>> {
    const POINTS: [&str; 6] = [
        "All your writing lives in one file, encrypted with this password.",
        "Forget the password and the writing is gone: there's no recovery.",
        "Write in markdown. Every post starts with a # title.",
        "Posts save as you type; Ctrl+S also keeps a version to go back to.",
        "While writing, Ctrl+K shows every command and its shortcut.",
        "Ctrl+O locks writui; unlocking takes you back where you were.",
    ];
    let mut lines = vec![Line::from("Overview".bold())];
    lines.extend(POINTS.iter().map(|point| Line::from(format!("· {point}").dim())));
    lines
}

/// writui's logo, drawn large for the password screens: the `>` mark, the
/// green bar (the caret), and the name in pixel letters, each line split
/// into those three parts.
#[rustfmt::skip]
const BANNER: [[&str; 3]; 6] = [
    ["         ", "  ", "                         ██                         ██"],
    ["▄▄▄      ", "▄▄", "                                 ██"],
    [" ▀██▄    ", "██", "    ██    ██ ██ ▄████ █████    ███████  ██    ██ █████"],
    ["   ▀██▄  ", "██", "    ██ ▄▄ ██ ███▀        ██      ██     ██    ██    ██"],
    ["  ▄██▀   ", "██", "    ██▄██▄██ ██          ██      ██     ██    ██    ██"],
    ["▄██▀     ", "██", "    ▀█▀  ▀█▀ ██       ████████   ▀█████ ▀██▄▄▄██ ████████"],
];

/// The password screens' title: the banner, or just "writui" when `area`
/// is too small to fit the banner above `other_lines` more lines.
pub fn title(area: Rect, other_lines: usize) -> Vec<Line<'static>> {
    if area.width < COLUMN_WIDTH || (area.height as usize) < BANNER.len() + other_lines {
        return vec![Line::from("writui".bold())];
    }
    BANNER
        .iter()
        .map(|[mark, bar, name]| Line::from(vec![Span::raw(*mark), bar.green(), Span::raw(*name)]))
        .collect()
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
    Rect::new(area.x + (area.width - width) / 2, area.y + (area.height - height) / 2, width, height)
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

    /// Draw the field in a box with `label` on its top edge, dimmed unless
    /// `focused`. A password too long for the box shows its end. Returns
    /// where the cursor goes.
    pub fn render_boxed(
        &self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        focused: bool,
    ) -> Position {
        let style = if focused { Style::new() } else { Style::new().dim() };
        let label = if focused { label.bold() } else { label.dim() };
        let block = Block::bordered().border_style(style).title(Line::from(vec![
            " ".into(),
            label,
            " ".into(),
        ]));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let inner = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
        let shown = self.len().min(inner.width.saturating_sub(1) as usize);
        frame.render_widget(Paragraph::new("•".repeat(shown)), inner);
        Position::new(inner.x + shown as u16, inner.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overview_is_short() {
        let lines = overview();
        assert!(lines.len() <= 7);
        assert!(lines.iter().all(|line| line.width() <= COLUMN_WIDTH as usize));
    }

    #[test]
    fn the_banner_fits_the_column() {
        let lines = title(Rect::new(0, 0, 100, 30), 10);
        assert_eq!(lines.len(), BANNER.len());
        assert!(lines.iter().all(|line| line.width() <= COLUMN_WIDTH as usize));
    }

    #[test]
    fn small_windows_get_the_plain_title() {
        assert_eq!(title(Rect::new(0, 0, 60, 30), 10).len(), 1);
        assert_eq!(title(Rect::new(0, 0, 100, 12), 10).len(), 1);
    }

    #[test]
    fn truncate_fits_width() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 6), "hello…");
        assert_eq!(truncate("hello", 0), "");
        assert_eq!(truncate("日本語です", 5), "日本…");
    }
}
