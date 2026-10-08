//! The command menu (Ctrl+K in the editor): a box in the middle of the
//! screen listing every command with its shortcut, under a field that
//! filters them. ↑/↓ move the highlight (nothing is highlighted at first),
//! Enter runs the highlighted command, Esc closes, and commands can be
//! clicked.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::widgets::{centered, truncate};

const WIDTH: u16 = 48;

/// The longest filter that can be typed.
const MAX_FILTER: usize = 40;

pub struct Item<C> {
    pub label: &'static str,
    /// The command's own shortcut, e.g. "Ctrl+S".
    pub key: &'static str,
    pub cmd: C,
}

pub fn item<C>(label: &'static str, key: &'static str, cmd: C) -> Item<C> {
    Item { label, key, cmd }
}

pub enum Outcome<C> {
    None,
    Close,
    Run(C),
}

pub struct Menu<C> {
    items: Vec<Item<C>>,
    filter: String,
    /// The highlighted command, as an index into `matches()`.
    selected: Option<usize>,
    /// The first match on screen, when they don't all fit.
    offset: usize,
    /// Where the menu was drawn, and each command on screen (by index into
    /// `matches()`), for the mouse.
    area: Rect,
    hits: Vec<(Rect, usize)>,
}

impl<C: Copy> Menu<C> {
    pub fn new(items: Vec<Item<C>>) -> Self {
        Menu { items, filter: String::new(), selected: None, offset: 0, area: Rect::default(), hits: Vec::new() }
    }

    /// The commands whose names contain the filter (ignoring case).
    fn matches(&self) -> Vec<&Item<C>> {
        let filter = self.filter.to_lowercase();
        self.items.iter().filter(|item| item.label.to_lowercase().contains(&filter)).collect()
    }

    /// The highlighted command.
    pub fn selected(&self) -> Option<C> {
        self.selected.and_then(|i| self.matches().get(i).map(|item| item.cmd))
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let matches: Vec<(&str, &str)> = self.matches().iter().map(|item| (item.label, item.key)).collect();
        let count = matches.len().max(1);
        // Borders, the filter and the line under it, then the commands.
        let area = centered(frame.area(), WIDTH, count as u16 + 4);
        self.area = area;
        frame.render_widget(Clear, area);
        frame.render_widget(Block::bordered(), area);
        let inner = Rect { x: area.x + 1, y: area.y + 1, width: area.width.saturating_sub(2), height: area.height.saturating_sub(2) };
        if inner.height < 2 || inner.width < 4 {
            self.hits.clear();
            return;
        }

        // The filter, with the cursor at its end.
        let field = Rect { x: inner.x + 1, width: inner.width - 2, height: 1, ..inner };
        let prompt = "> ";
        let room = (field.width as usize).saturating_sub(prompt.len() + 1);
        let mut shown = self.filter.as_str();
        while shown.width() > room {
            let mut chars = shown.chars();
            chars.next();
            shown = chars.as_str();
        }
        let text = if shown.is_empty() { Span::raw("type to filter").dim() } else { Span::raw(shown) };
        frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(prompt).dim(), text])), field);
        frame.set_cursor_position(Position::new(field.x + (prompt.len() + shown.width()) as u16, field.y));

        // A line under the filter, joined to the border.
        let rule = format!("├{}┤", "─".repeat(inner.width as usize));
        frame.render_widget(Paragraph::new(rule), Rect { x: area.x, y: inner.y + 1, width: area.width, height: 1 });

        let rows = Rect { y: inner.y + 2, height: inner.height - 2, ..inner };
        self.hits.clear();
        if matches.is_empty() {
            let text = Line::from(" No commands match.".dim());
            frame.render_widget(Paragraph::new(text), rows);
            return;
        }
        let visible = rows.height as usize;
        if let Some(selected) = self.selected {
            if selected < self.offset {
                self.offset = selected;
            } else if selected >= self.offset + visible {
                self.offset = selected + 1 - visible;
            }
        }
        self.offset = self.offset.min(matches.len().saturating_sub(visible));
        let width = rows.width as usize;
        let lines: Vec<Line> = matches
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(visible)
            .map(|(i, &(label, key))| {
                let label = truncate(label, width.saturating_sub(key.width() + 4));
                let gap = width.saturating_sub(label.width() + key.width() + 2);
                let line = Line::from(vec![
                    Span::raw(format!(" {label}{}", " ".repeat(gap))),
                    Span::raw(key).dim(),
                    Span::raw(" "),
                ]);
                if self.selected == Some(i) { line.style(Style::new().reversed()) } else { line }
            })
            .collect();
        for (row, i) in (self.offset..matches.len()).take(visible).enumerate() {
            self.hits.push((Rect { y: rows.y + row as u16, height: 1, ..rows }, i));
        }
        frame.render_widget(Paragraph::new(lines), rows);
    }

    pub fn handle(&mut self, event: Event) -> Outcome<C> {
        let count = self.matches().len();
        match event {
            Event::Key(key) => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Esc => return Outcome::Close,
                    // Ctrl+K again closes it.
                    KeyCode::Char('k') if ctrl => return Outcome::Close,
                    KeyCode::Enter => return self.selected().map_or(Outcome::None, Outcome::Run),
                    KeyCode::Down | KeyCode::Tab if count > 0 => {
                        self.selected = Some(self.selected.map_or(0, |i| (i + 1) % count));
                    }
                    KeyCode::Up | KeyCode::BackTab if count > 0 => {
                        self.selected = Some(self.selected.map_or(count - 1, |i| (i + count - 1) % count));
                    }
                    KeyCode::Char('u') if ctrl => self.set_filter(String::new()),
                    KeyCode::Char(ch) if !ctrl => {
                        let mut filter = self.filter.clone();
                        filter.push(ch);
                        self.set_filter(filter);
                    }
                    KeyCode::Backspace => {
                        let mut filter = self.filter.clone();
                        filter.pop();
                        self.set_filter(filter);
                    }
                    _ => {}
                }
            }
            Event::Paste(text) => {
                let line = text.lines().next().unwrap_or_default();
                self.set_filter(format!("{}{line}", self.filter));
            }
            Event::Mouse(mouse) => {
                let pos = Position::new(mouse.column, mouse.row);
                let hit = self.hits.iter().find(|(rect, _)| rect.contains(pos)).map(|(_, i)| *i);
                match mouse.kind {
                    MouseEventKind::Moved if hit.is_some() => self.selected = hit,
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(i) = hit {
                            self.selected = Some(i);
                            return self.selected().map_or(Outcome::None, Outcome::Run);
                        }
                        // A click outside closes it.
                        if !self.area.contains(pos) {
                            return Outcome::Close;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Outcome::None
    }

    /// Typing filters the commands and highlights the first that matches,
    /// so Enter runs it.
    fn set_filter(&mut self, filter: String) {
        let filter: String = filter.chars().filter(|ch| !ch.is_control()).take(MAX_FILTER).collect();
        if filter == self.filter {
            return;
        }
        self.filter = filter;
        self.offset = 0;
        self.selected = if self.filter.is_empty() || self.matches().is_empty() { None } else { Some(0) };
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyEvent;

    use super::*;

    fn sample() -> Menu<u8> {
        Menu::new(vec![item("Save version", "Ctrl+S", 1), item("History", "Ctrl+R", 2), item("Undo", "Ctrl+Z", 3)])
    }

    fn press(menu: &mut Menu<u8>, code: KeyCode) -> Option<u8> {
        match menu.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))) {
            Outcome::Run(cmd) => Some(cmd),
            _ => None,
        }
    }

    fn type_text(menu: &mut Menu<u8>, text: &str) {
        for ch in text.chars() {
            press(menu, KeyCode::Char(ch));
        }
    }

    #[test]
    fn nothing_is_highlighted_at_first() {
        let mut menu = sample();
        assert_eq!(menu.selected(), None);
        assert_eq!(press(&mut menu, KeyCode::Enter), None);
        assert_eq!(press(&mut menu, KeyCode::Down), None);
        assert_eq!(press(&mut menu, KeyCode::Enter), Some(1));
    }

    #[test]
    fn up_from_nothing_goes_to_the_last() {
        let mut menu = sample();
        press(&mut menu, KeyCode::Up);
        assert_eq!(menu.selected(), Some(3));
        press(&mut menu, KeyCode::Down);
        assert_eq!(menu.selected(), Some(1));
    }

    #[test]
    fn typing_filters_and_highlights_the_first_match() {
        let mut menu = sample();
        type_text(&mut menu, "HIS");
        assert_eq!(menu.matches().len(), 1);
        assert_eq!(press(&mut menu, KeyCode::Enter), Some(2));

        let mut menu = sample();
        type_text(&mut menu, "o");
        // "Save version", "History", "Undo".
        assert_eq!(menu.matches().len(), 3);
        type_text(&mut menu, "zz");
        assert!(menu.matches().is_empty());
        assert_eq!(press(&mut menu, KeyCode::Enter), None);
        press(&mut menu, KeyCode::Backspace);
        press(&mut menu, KeyCode::Backspace);
        press(&mut menu, KeyCode::Backspace);
        // An empty filter highlights nothing again.
        assert_eq!(menu.selected(), None);
    }

    #[test]
    fn esc_closes() {
        let mut menu = sample();
        let outcome = menu.handle(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(matches!(outcome, Outcome::Close));
    }
}
