//! A one-line field for typing a file's path, with Tab completing it as a
//! shell does: Tab fills in as much as the matching files and folders
//! share and lists them under the field; Tab again goes through them one
//! by one (Shift+Tab back), Enter keeps the one picked, and they can be
//! clicked.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::files::{common_prefix, completions};

/// The longest path that can be typed.
const MAX_CHARS: usize = 1024;

const GAP: &str = "  ";

pub struct PathField {
    value: String,
    /// What Tab found, listed under the field.
    matches: Vec<String>,
    /// The match Tab has gone to, if it's going through them.
    picked: Option<usize>,
    /// Said under the field instead (e.g. "Nothing matches."), until the
    /// next key.
    note: Option<String>,
    hits: Vec<(Rect, usize)>,
}

impl PathField {
    pub fn new(value: &str) -> Self {
        let mut field = PathField { value: String::new(), matches: Vec::new(), picked: None, note: None, hits: Vec::new() };
        field.push(value);
        field
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// Going through what Tab found: Enter keeps the one picked.
    pub fn picking(&self) -> bool {
        self.picked.is_some()
    }

    /// Showing what Tab found (Esc hides it).
    pub fn listing(&self) -> bool {
        !self.matches.is_empty()
    }

    /// Stop going through what Tab found, keeping the path as it is.
    pub fn done(&mut self) {
        self.matches.clear();
        self.picked = None;
        self.note = None;
    }

    /// Handle a key typed into the field. Returns whether it was used.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Tab => self.complete(1),
            KeyCode::BackTab => self.complete(-1),
            KeyCode::Char('u') if ctrl => self.set(""),
            // Back to the previous `/`, as in a shell.
            KeyCode::Char('w') if ctrl => self.delete_part(),
            KeyCode::Backspace if alt || ctrl => self.delete_part(),
            KeyCode::Char(ch) if !ctrl && !alt => {
                self.done();
                self.push(&ch.to_string());
            }
            KeyCode::Backspace => {
                self.done();
                self.value.pop();
            }
            _ => return false,
        }
        true
    }

    pub fn paste(&mut self, text: &str) {
        self.done();
        self.push(text.lines().next().unwrap_or_default());
    }

    fn set(&mut self, value: &str) {
        self.done();
        self.value.clear();
        self.push(value);
    }

    fn delete_part(&mut self) {
        let trimmed = self.value.trim_end_matches('/');
        let keep = trimmed.rfind('/').map_or(0, |i| i + 1);
        let value = self.value[..keep].to_string();
        self.set(&value);
    }

    /// Add `text`, leaving out control characters, up to the longest path.
    fn push(&mut self, text: &str) {
        let room = MAX_CHARS.saturating_sub(self.value.chars().count());
        self.value.extend(text.chars().filter(|ch| !ch.is_control()).take(room));
    }

    /// Tab (`step` 1) or Shift+Tab (-1).
    fn complete(&mut self, step: isize) {
        if !self.matches.is_empty() {
            let count = self.matches.len() as isize;
            let next = match self.picked {
                Some(i) => (i as isize + step).rem_euclid(count),
                None if step > 0 => 0,
                None => count - 1,
            } as usize;
            self.picked = Some(next);
            self.value = self.matches[next].clone();
            return;
        }
        let matches = completions(&self.value);
        match matches.len() {
            0 => self.note = Some("Nothing here starts with that.".into()),
            1 => {
                let only = matches[0].clone();
                self.set(&only);
            }
            _ => {
                let shared = common_prefix(&matches);
                if shared.chars().count() > self.value.chars().count() {
                    self.value = shared;
                }
                self.matches = matches;
            }
        }
    }

    /// Pick one of the listed matches by clicking it. Returns whether the
    /// click was on one.
    pub fn click(&mut self, column: u16, row: u16) -> bool {
        let pos = Position::new(column, row);
        let Some(&(_, i)) = self.hits.iter().find(|(rect, _)| rect.contains(pos)) else { return false };
        let picked = self.matches[i].clone();
        self.set(&picked);
        true
    }

    /// Draw the field (underlined) in `field`, and under it, in `info`, what
    /// Tab found, else `error` (in red), else `help` (faded).
    pub fn render(&mut self, frame: &mut Frame, field: Rect, info: Rect, focused: bool, help: &str, error: Option<&str>) {
        // Long paths scroll to keep their end in view.
        let width = field.width as usize;
        let mut shown = self.value.as_str();
        while shown.width() + 1 > width && !shown.is_empty() {
            let mut chars = shown.chars();
            chars.next();
            shown = chars.as_str();
        }
        let pad = " ".repeat(width.saturating_sub(shown.width()));
        frame.render_widget(Paragraph::new(Span::styled(format!("{shown}{pad}"), Style::new().underlined())), field);
        if focused {
            let x = (field.x + shown.width() as u16).min(field.right().saturating_sub(1));
            frame.set_cursor_position(Position::new(x, field.y));
        }

        self.hits.clear();
        if self.matches.is_empty() {
            let line = match (&self.note, error) {
                (Some(note), _) => Line::from(note.clone().dim()),
                (None, Some(error)) => Line::from(error.to_string().red()),
                (None, None) => Line::from(help.to_string().dim()),
            };
            frame.render_widget(Paragraph::new(line), info);
            return;
        }
        // The matches by name, as many as fit, from one that keeps the
        // picked one in view.
        let names: Vec<&str> = self.matches.iter().map(|m| name(m)).collect();
        let fits = |from: usize, to: usize| -> bool {
            let used: usize = names[from..=to].iter().map(|n| n.width() + GAP.len()).sum();
            used <= info.width as usize
        };
        let mut first = 0;
        if let Some(picked) = self.picked {
            while first < picked && !fits(first, picked) {
                first += 1;
            }
        }
        let mut spans = Vec::new();
        let mut x = info.x;
        for (i, name) in names.iter().enumerate().skip(first) {
            let more = if i + 1 < names.len() { 1 } else { 0 };
            if (x - info.x) as usize + name.width() + more > info.width as usize {
                spans.push(Span::raw("…").dim());
                break;
            }
            let style = if Some(i) == self.picked { Style::new().reversed() } else { Style::new().dim() };
            spans.push(Span::styled(name.to_string(), style));
            spans.push(Span::raw(GAP));
            self.hits.push((Rect::new(x, info.y, name.width() as u16, 1), i));
            x += (name.width() + GAP.len()) as u16;
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), info);
    }
}

/// The last part of a path, keeping a folder's `/`.
fn name(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    let start = trimmed.rfind('/').map_or(0, |i| i + 1);
    &path[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(field: &mut PathField, code: KeyCode) {
        field.key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn tab_completes_then_goes_through_the_matches() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().to_str().unwrap();
        std::fs::create_dir(dir.path().join("posts")).unwrap();
        std::fs::write(dir.path().join("post-one.md"), "").unwrap();
        std::fs::write(dir.path().join("post-two.md"), "").unwrap();

        let mut field = PathField::new(&format!("{base}/p"));
        tab(&mut field, KeyCode::Tab);
        // As far as they all go, then listed.
        assert_eq!(field.value(), format!("{base}/post"));
        assert!(field.listing() && !field.picking());
        tab(&mut field, KeyCode::Tab);
        assert_eq!(field.value(), format!("{base}/post-one.md"));
        tab(&mut field, KeyCode::Tab);
        tab(&mut field, KeyCode::Tab);
        assert_eq!(field.value(), format!("{base}/posts/"));
        tab(&mut field, KeyCode::BackTab);
        assert_eq!(field.value(), format!("{base}/post-two.md"));
        // Typing keeps it and carries on.
        tab(&mut field, KeyCode::Backspace);
        assert!(!field.listing());
        assert_eq!(field.value(), format!("{base}/post-two.m"));
        // A single match is filled in; folders end in `/`.
        let mut field = PathField::new(&format!("{base}/posts"));
        tab(&mut field, KeyCode::Tab);
        assert_eq!(field.value(), format!("{base}/posts/"));
        tab(&mut field, KeyCode::Tab);
        assert_eq!(field.note.as_deref(), Some("Nothing here starts with that."));
        // Ctrl+W goes back a folder.
        field.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(field.value(), format!("{base}/"));
    }

    #[test]
    fn names_keep_a_folders_slash() {
        assert_eq!(name("~/a/b.md"), "b.md");
        assert_eq!(name("~/a/posts/"), "posts/");
        assert_eq!(name("x"), "x");
    }
}
