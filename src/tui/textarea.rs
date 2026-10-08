//! A small box of editable lines (e.g. a post's front matter): typing,
//! Enter, Backspace / Delete, the arrows, Home / End, paste, clicks and
//! the scroll wheel. Lines don't wrap: the cursor's line scrolls sideways
//! to keep it in view, and others end in `…` if they're too long.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthChar;

use super::widgets::truncate;

/// What Tab types.
const TAB: &str = "  ";

/// The most text the box holds, in characters.
const MAX_CHARS: usize = 64 * 1024;

pub struct TextArea {
    lines: Vec<String>,
    /// The cursor: a line, and a character index into it.
    row: usize,
    col: usize,
    /// The first line on screen.
    top: usize,
    area: Rect,
}

impl TextArea {
    /// Starts with the cursor at the start.
    pub fn new(text: &str) -> Self {
        let mut lines: Vec<String> = text.lines().map(clean).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        TextArea { lines, row: 0, col: 0, top: 0, area: Rect::default() }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// The cursor is on the first line (so Up would leave the box).
    pub fn at_first_line(&self) -> bool {
        self.row == 0
    }

    /// The cursor is on the last line (so Down would leave the box).
    pub fn at_last_line(&self) -> bool {
        self.row + 1 == self.lines.len()
    }

    fn len(&self) -> usize {
        self.lines.iter().map(|line| line.chars().count() + 1).sum()
    }

    fn byte(&self) -> usize {
        byte_at(&self.lines[self.row], self.col)
    }

    /// Handle a key. Returns whether it was used.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let line_len = self.lines[self.row].chars().count();
        match key.code {
            KeyCode::Char(ch) if !ctrl && !alt => self.insert(&ch.to_string()),
            KeyCode::Tab => self.insert(TAB),
            KeyCode::Enter => self.insert("\n"),
            KeyCode::Backspace if self.col > 0 => {
                self.col -= 1;
                let at = self.byte();
                self.lines[self.row].remove(at);
            }
            KeyCode::Backspace if self.row > 0 => {
                let line = self.lines.remove(self.row);
                self.row -= 1;
                self.col = self.lines[self.row].chars().count();
                self.lines[self.row].push_str(&line);
            }
            KeyCode::Delete if self.col < line_len => {
                let at = self.byte();
                self.lines[self.row].remove(at);
            }
            KeyCode::Delete if self.row + 1 < self.lines.len() => {
                let next = self.lines.remove(self.row + 1);
                self.lines[self.row].push_str(&next);
            }
            KeyCode::Left if self.col > 0 => self.col -= 1,
            KeyCode::Left if self.row > 0 => {
                self.row -= 1;
                self.col = self.lines[self.row].chars().count();
            }
            KeyCode::Right if self.col < line_len => self.col += 1,
            KeyCode::Right if self.row + 1 < self.lines.len() => {
                self.row += 1;
                self.col = 0;
            }
            KeyCode::Up if self.row > 0 => self.go_to(self.row - 1, self.col),
            KeyCode::Down if self.row + 1 < self.lines.len() => self.go_to(self.row + 1, self.col),
            KeyCode::Home => self.col = 0,
            KeyCode::End => self.col = line_len,
            KeyCode::Backspace | KeyCode::Delete | KeyCode::Left | KeyCode::Right => {}
            _ => return false,
        }
        true
    }

    /// Insert `text` (which may have line breaks) at the cursor.
    pub fn insert(&mut self, text: &str) {
        let room = MAX_CHARS.saturating_sub(self.len());
        let text: String = text.replace("\r\n", "\n").chars().filter(|c| *c == '\n' || !c.is_control()).take(room).collect();
        let at = self.byte();
        let tail = self.lines[self.row].split_off(at);
        let mut parts = text.split('\n');
        if let Some(first) = parts.next() {
            self.lines[self.row].push_str(first);
            self.col += first.chars().count();
        }
        for part in parts {
            self.row += 1;
            self.lines.insert(self.row, part.to_string());
            self.col = part.chars().count();
        }
        self.lines[self.row].push_str(&tail);
    }

    fn go_to(&mut self, row: usize, col: usize) {
        self.row = row.min(self.lines.len() - 1);
        self.col = col.min(self.lines[self.row].chars().count());
    }

    /// Put the cursor at the end of the first or last line.
    pub fn enter_at(&mut self, last: bool) {
        let row = if last { self.lines.len() - 1 } else { 0 };
        self.go_to(row, usize::MAX);
    }

    /// Put the cursor where the box was clicked. Returns whether the click
    /// was in the box.
    pub fn click(&mut self, column: u16, row: u16) -> bool {
        if !self.area.contains(Position::new(column, row)) {
            return false;
        }
        let line = self.top + (row - self.area.y) as usize;
        if line >= self.lines.len() {
            self.enter_at(true);
            return true;
        }
        let (skip, _) = self.window(line);
        let target = (column - self.area.x) as usize;
        let mut x = 0;
        let mut col = skip;
        for ch in self.lines[line].chars().skip(skip) {
            let w = ch.width().unwrap_or(0);
            if x + w > target {
                break;
            }
            x += w;
            col += 1;
        }
        self.go_to(line, col);
        true
    }

    /// Scroll by `lines` (up if negative) when the wheel turns over the box.
    pub fn scroll(&mut self, column: u16, row: u16, lines: isize) -> bool {
        if !self.area.contains(Position::new(column, row)) {
            return false;
        }
        let max = self.lines.len().saturating_sub(self.area.height as usize);
        self.top = self.top.saturating_add_signed(lines).min(max);
        true
    }

    /// For line `i`: how many characters scroll off to the left, and the
    /// cursor's x on screen (if it's on that line).
    fn window(&self, i: usize) -> (usize, usize) {
        if i != self.row {
            return (0, 0);
        }
        let width = (self.area.width as usize).max(1);
        let chars: Vec<char> = self.lines[i].chars().collect();
        let mut skip = 0;
        let mut x: usize = chars[..self.col].iter().map(|c| c.width().unwrap_or(0)).sum();
        while x + 1 > width && skip < self.col {
            x -= chars[skip].width().unwrap_or(0);
            skip += 1;
        }
        (skip, x)
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, focused: bool, follow: bool) {
        self.area = area;
        let height = area.height as usize;
        if follow {
            if self.row < self.top {
                self.top = self.row;
            } else if self.row >= self.top + height {
                self.top = self.row + 1 - height;
            }
        }
        let width = area.width as usize;
        let lines: Vec<Line> = (self.top..self.top + height)
            .filter_map(|i| self.lines.get(i).map(|line| (i, line)))
            .map(|(i, line)| {
                if i == self.row && focused {
                    let (skip, _) = self.window(i);
                    let rest: String = line.chars().skip(skip).collect();
                    Line::from(truncate(&rest, width))
                } else {
                    Line::from(truncate(line, width))
                }
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), area);
        if focused && (self.top..self.top + height).contains(&self.row) {
            let (_, x) = self.window(self.row);
            frame.set_cursor_position(Position::new(area.x + x as u16, area.y + (self.row - self.top) as u16));
        }
    }
}

fn clean(line: &str) -> String {
    line.chars().filter(|c| !c.is_control() || *c == '\t').map(|c| if c == '\t' { ' ' } else { c }).collect()
}

fn byte_at(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(area: &mut TextArea, code: KeyCode) {
        area.key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn edits_across_lines() {
        let mut area = TextArea::new("title: Hi\ndate: x");
        press(&mut area, KeyCode::End);
        press(&mut area, KeyCode::Enter);
        area.insert("tags:\n- a");
        assert_eq!(area.text(), "title: Hi\ntags:\n- a\ndate: x");
        press(&mut area, KeyCode::Down);
        press(&mut area, KeyCode::Home);
        press(&mut area, KeyCode::Backspace);
        assert_eq!(area.text(), "title: Hi\ntags:\n- adate: x");
        press(&mut area, KeyCode::Delete);
        press(&mut area, KeyCode::Tab);
        assert_eq!(area.text(), "title: Hi\ntags:\n- a  ate: x");
        assert!(area.at_last_line());
        press(&mut area, KeyCode::Up);
        press(&mut area, KeyCode::Up);
        press(&mut area, KeyCode::Up);
        assert!(area.at_first_line());
        press(&mut area, KeyCode::Right);
        press(&mut area, KeyCode::Char('é'));
        assert_eq!(area.text(), "title:é Hi\ntags:\n- a  ate: x");
        assert_eq!(TextArea::new("").text(), "");
    }
}
