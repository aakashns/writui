//! A small modal dialog with buttons. ←/→/Tab move between buttons, Enter
//! presses the selected one, a button's key presses it directly, Esc
//! cancels, and buttons can be clicked.
//!
//! A dialog can also have a one-line text field. Then typing goes into the
//! field, so buttons have no keys: Tab moves between them and Enter presses
//! the selected one.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

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
    input: Option<Input>,
    hits: Vec<Rect>,
}

struct Input {
    value: String,
    placeholder: &'static str,
    max_chars: usize,
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
        Dialog { title: title.into(), lines, buttons, focus, cancel, input: None, hits: Vec::new() }
    }

    /// Add a text field (below the lines), holding at most `max_chars`.
    pub fn with_input(mut self, placeholder: &'static str, max_chars: usize) -> Self {
        self.input = Some(Input { value: String::new(), placeholder, max_chars });
        self
    }

    /// Start the text field off with `value`.
    pub fn with_value(mut self, value: &str) -> Self {
        if let Some(input) = &mut self.input {
            input.value.clear();
            input.push(value);
        }
        self
    }

    /// What's been typed into the text field.
    pub fn input(&self) -> &str {
        self.input.as_ref().map_or("", |input| input.value.as_str())
    }

    /// Hints for the hint bar while the dialog is open.
    pub fn hints(&self) -> Vec<Hint<C>> {
        if self.input.is_some() {
            let enter = &self.buttons[self.focus];
            let cancel = &self.buttons[self.cancel];
            let mut hints = vec![hint("Enter", enter.label.to_lowercase(), enter.cmd)];
            if self.focus != self.cancel {
                hints.push(hint("Esc", cancel.label.to_lowercase(), cancel.cmd));
            }
            return hints;
        }
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
        let input_rows = if self.input.is_some() { 2 } else { 0 };
        let height = self.lines.len() as u16 + input_rows + 4;
        let area = centered(frame.area(), WIDTH, height);
        let block = Block::bordered().title(format!(" {} ", self.title));
        let inner = block.inner(area);
        let inner = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let mut spans = Vec::new();
        self.hits.clear();
        let mut x = inner.x;
        let y = inner.y + self.lines.len() as u16 + input_rows + 1;
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
        if let Some(input) = &self.input {
            // An underlined field; long text scrolls to keep its end in view.
            let width = inner.width as usize;
            let mut shown = input.value.as_str();
            while shown.width() + 1 > width && !shown.is_empty() {
                let mut chars = shown.chars();
                chars.next();
                shown = chars.as_str();
            }
            let (text, style) = if shown.is_empty() {
                (input.placeholder, Style::new().dim())
            } else {
                (shown, Style::new())
            };
            let pad = " ".repeat(width.saturating_sub(text.width()));
            lines.push(Line::default());
            lines.push(Line::from(Span::styled(format!("{text}{pad}"), style.underlined())));
            let x = (inner.x + shown.width() as u16).min(inner.right().saturating_sub(1));
            frame.set_cursor_position(Position::new(x, inner.y + self.lines.len() as u16 + 1));
        }
        lines.push(Line::default());
        lines.push(Line::from(spans));
        frame.render_widget(Paragraph::new(lines), inner);
    }

    /// The command chosen by this event, if any.
    pub fn handle(&mut self, event: Event) -> Option<C> {
        let count = self.buttons.len();
        if let Some(input) = &mut self.input {
            match &event {
                Event::Key(key) => match key.code {
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        input.value.clear();
                        return None;
                    }
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        input.push(&ch.to_string());
                        return None;
                    }
                    KeyCode::Backspace => {
                        input.value.pop();
                        return None;
                    }
                    KeyCode::Left | KeyCode::Right => return None,
                    _ => {}
                },
                Event::Paste(text) => {
                    input.push(text.lines().next().unwrap_or_default());
                    return None;
                }
                _ => {}
            }
        }
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

impl Input {
    /// Add `text`, leaving out control characters, up to `max_chars`.
    fn push(&mut self, text: &str) {
        let room = self.max_chars.saturating_sub(self.value.chars().count());
        self.value.extend(text.chars().filter(|ch| !ch.is_control()).take(room));
    }
}
