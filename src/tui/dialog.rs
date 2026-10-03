//! A small modal dialog with buttons. ←/→/Tab move between buttons, Enter
//! presses the selected one, a button's key presses it directly, Esc
//! cancels, and buttons can be clicked.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::hints::{Hint, hint};
use super::widgets::centered;

const WIDTH: u16 = 56;

pub struct Button<C> {
    pub label: &'static str,
    /// A single-character shortcut, e.g. "y".
    pub key: &'static str,
    pub cmd: C,
    /// Destructive buttons are drawn in red.
    pub danger: bool,
}

pub struct Dialog<C> {
    title: String,
    lines: Vec<Line<'static>>,
    buttons: Vec<Button<C>>,
    focus: usize,
    /// Index of the button Esc presses.
    cancel: usize,
    hits: Vec<Rect>,
}

impl<C: Copy> Dialog<C> {
    /// `focus` is the initially selected button; `cancel` is the button
    /// Esc presses.
    pub fn new(
        title: impl Into<String>,
        lines: Vec<Line<'static>>,
        buttons: Vec<Button<C>>,
        focus: usize,
        cancel: usize,
    ) -> Self {
        Dialog { title: title.into(), lines, buttons, focus, cancel, hits: Vec::new() }
    }

    /// Hints for the hint bar while the dialog is open.
    pub fn hints(&self) -> Vec<Hint<C>> {
        let mut hints: Vec<Hint<C>> = self
            .buttons
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != self.cancel)
            .map(|(_, b)| hint(b.key, b.label.to_lowercase(), b.cmd))
            .collect();
        let cancel = &self.buttons[self.cancel];
        hints.push(hint("Esc", cancel.label.to_lowercase(), cancel.cmd));
        hints
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let height = self.lines.len() as u16 + 4;
        let area = centered(frame.area(), WIDTH, height);
        let block = Block::bordered().title(format!(" {} ", self.title));
        let inner = block.inner(area);
        let inner = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let mut spans = Vec::new();
        self.hits.clear();
        let mut x = inner.x;
        let y = inner.y + self.lines.len() as u16 + 1;
        for (i, button) in self.buttons.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  "));
                x += 2;
            }
            let text = format!("[ {} ]", button.label);
            let width = Span::raw(text.as_str()).width() as u16;
            let mut style = Style::new();
            if button.danger {
                style = style.red();
            }
            if i == self.focus {
                style = style.reversed().bold();
            }
            spans.push(Span::styled(text, style));
            self.hits.push(Rect::new(x, y, width, 1));
            x += width;
        }
        let mut lines = self.lines.clone();
        lines.push(Line::default());
        lines.push(Line::from(spans));
        frame.render_widget(Paragraph::new(lines), inner);
    }

    /// The command chosen by this event, if any.
    pub fn handle(&mut self, event: Event) -> Option<C> {
        let count = self.buttons.len();
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Left | KeyCode::BackTab => {
                    self.focus = (self.focus + count - 1) % count;
                    None
                }
                KeyCode::Right | KeyCode::Tab => {
                    self.focus = (self.focus + 1) % count;
                    None
                }
                KeyCode::Enter => Some(self.buttons[self.focus].cmd),
                KeyCode::Esc => Some(self.buttons[self.cancel].cmd),
                KeyCode::Char(ch) => self
                    .buttons
                    .iter()
                    .find(|b| b.key.chars().eq([ch.to_ascii_lowercase()]))
                    .map(|b| b.cmd),
                _ => None,
            },
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                let pos = Position::new(mouse.column, mouse.row);
                let index = self.hits.iter().position(|rect| rect.contains(pos))?;
                Some(self.buttons[index].cmd)
            }
            _ => None,
        }
    }
}
