//! A small menu that drops down where it's opened: the right-click menu in
//! the text (Cut, Copy, Paste and Select All, each faded and doing nothing
//! when it can't be done), and the list's "Sort". ↑/↓ and the mouse highlight,
//! Enter or a click runs one, Esc or a click outside closes it.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::menu::Outcome;

pub struct Entry<C> {
    pub label: &'static str,
    /// Dim, on the right: the command's own shortcut, e.g. "Ctrl+X" (or
    /// empty).
    pub key: &'static str,
    pub cmd: C,
    pub enabled: bool,
}

pub struct ContextMenu<C> {
    entries: Vec<Entry<C>>,
    /// Where it was opened: its top left corner, unless that doesn't fit.
    at: Position,
    selected: Option<usize>,
    /// Where it was drawn, for the mouse.
    area: Rect,
}

impl<C: Copy> ContextMenu<C> {
    pub fn new(entries: Vec<Entry<C>>, at: Position) -> Self {
        ContextMenu { entries, at, selected: None, area: Rect::default() }
    }

    /// With entry `i` highlighted, e.g. the choice already made.
    pub fn with_selected(mut self, i: usize) -> Self {
        self.selected = Some(i).filter(|&i| self.entries.get(i).is_some_and(|e| e.enabled));
        self
    }

    /// The highlighted entry's command, if it can be run.
    pub fn selected(&self) -> Option<C> {
        self.selected.and_then(|i| self.entries.get(i)).filter(|e| e.enabled).map(|e| e.cmd)
    }

    /// Opened at `at` instead.
    pub fn moved_to(mut self, at: Position) -> Self {
        self.at = at;
        self
    }

    /// How wide it's drawn, in columns.
    pub fn width(&self) -> u16 {
        let label_width = self.entries.iter().map(|e| e.label.width()).max().unwrap_or(0);
        let key_width = self.entries.iter().map(|e| e.key.width()).max().unwrap_or(0);
        // Borders, a space each side, and three between label and key
        // (if there are keys).
        let between = if key_width > 0 { 3 } else { 0 };
        (label_width + between + key_width + 4) as u16
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let screen = frame.area();
        let width = self.width();
        let height = self.entries.len() as u16 + 2;
        let x = self.at.x.min(screen.right().saturating_sub(width));
        let y = if self.at.y + height <= screen.bottom() { self.at.y } else { self.at.y.saturating_sub(height) };
        self.area = Rect::new(x, y, width, height).intersection(screen);
        frame.render_widget(Clear, self.area);
        frame.render_widget(Block::bordered(), self.area);
        let lines: Vec<Line> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let gap = width as usize - 4 - entry.label.width() - entry.key.width();
                let line = Line::from(vec![
                    Span::raw(format!(" {}{}", entry.label, " ".repeat(gap))),
                    Span::raw(entry.key).dim(),
                    Span::raw(" "),
                ]);
                if !entry.enabled {
                    line.style(Style::new().dim())
                } else if self.selected == Some(i) {
                    line.style(Style::new().reversed())
                } else {
                    line
                }
            })
            .collect();
        let inner = Rect {
            x: self.area.x + 1,
            y: self.area.y + 1,
            width: self.area.width.saturating_sub(2),
            height: self.area.height.saturating_sub(2),
        };
        frame.render_widget(Paragraph::new(lines), inner);
    }

    /// The entry on a row of the screen, if it's one of the menu's.
    fn entry_at(&self, pos: Position) -> Option<usize> {
        let inner = Rect { x: self.area.x + 1, y: self.area.y + 1, ..self.area };
        let inside = pos.x >= inner.x && pos.x < self.area.right().saturating_sub(1);
        let row = pos.y.checked_sub(inner.y)? as usize;
        (inside && row < self.entries.len()).then_some(row)
    }

    /// Move the highlight to the next entry that can be run, `by` 1 or -1.
    fn step(&mut self, by: isize) {
        let count = self.entries.len() as isize;
        let mut i = self.selected.map_or(if by > 0 { -1 } else { count }, |i| i as isize);
        for _ in 0..count {
            i = (i + by).rem_euclid(count);
            if self.entries[i as usize].enabled {
                self.selected = Some(i as usize);
                return;
            }
        }
    }

    fn run(&self, i: usize) -> Outcome<C> {
        match self.entries.get(i) {
            Some(entry) if entry.enabled => Outcome::Run(entry.cmd),
            _ => Outcome::None,
        }
    }

    pub fn handle(&mut self, event: Event) -> Outcome<C> {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Esc => Outcome::Close,
                KeyCode::Down | KeyCode::Tab => {
                    self.step(1);
                    Outcome::None
                }
                KeyCode::Up | KeyCode::BackTab => {
                    self.step(-1);
                    Outcome::None
                }
                KeyCode::Enter => self.selected.map_or(Outcome::None, |i| self.run(i)),
                _ => Outcome::None,
            },
            Event::Mouse(mouse) => {
                let pos = Position::new(mouse.column, mouse.row);
                let hit = self.entry_at(pos);
                match mouse.kind {
                    MouseEventKind::Moved => {
                        self.selected = hit.filter(|&i| self.entries[i].enabled);
                        Outcome::None
                    }
                    MouseEventKind::Down(MouseButton::Left) => match hit {
                        Some(i) => self.run(i),
                        None if self.area.contains(pos) => Outcome::None,
                        None => Outcome::Close,
                    },
                    MouseEventKind::Down(_) if !self.area.contains(pos) => Outcome::Close,
                    _ => Outcome::None,
                }
            }
            _ => Outcome::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers, MouseEvent};

    use super::*;

    fn sample() -> ContextMenu<u8> {
        let entry = |label, cmd, enabled| Entry { label, key: "Ctrl+X", cmd, enabled };
        ContextMenu::new(vec![entry("Cut", 1, false), entry("Copy", 2, false), entry("Paste", 3, true)], Position::new(10, 5))
    }

    fn press(menu: &mut ContextMenu<u8>, code: KeyCode) -> Outcome<u8> {
        menu.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
    }

    fn click(menu: &mut ContextMenu<u8>, column: u16, row: u16) -> Outcome<u8> {
        let kind = MouseEventKind::Down(MouseButton::Left);
        menu.handle(Event::Mouse(MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE }))
    }

    #[test]
    fn only_what_can_be_done_runs() {
        let mut menu = sample();
        menu.area = Rect::new(10, 5, 18, 5);
        // The keys skip what's faded.
        press(&mut menu, KeyCode::Down);
        assert!(matches!(press(&mut menu, KeyCode::Enter), Outcome::Run(3)));
        press(&mut menu, KeyCode::Down);
        assert_eq!(menu.selected, Some(2));
        // Clicking a faded one does nothing; Paste runs; outside closes.
        assert!(matches!(click(&mut menu, 12, 6), Outcome::None));
        assert!(matches!(click(&mut menu, 12, 8), Outcome::Run(3)));
        assert!(matches!(click(&mut menu, 1, 1), Outcome::Close));
        assert!(matches!(press(&mut menu, KeyCode::Esc), Outcome::Close));
    }
}
