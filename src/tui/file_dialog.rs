//! The dialog for exporting a post to a file, or importing one: a path
//! (Tab completes it), and for exports, the front matter to put at the top
//! (which can be turned off). ↑/↓ move between them, Enter in the path or
//! Ctrl+S anywhere goes ahead, Esc cancels, and everything can be clicked.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::hints::{Hint, hint};
use super::path_field::PathField;
use super::textarea::TextArea;
use super::widgets::centered;

const WIDTH: u16 = 72;

/// The most lines of front matter on screen at once.
const MAX_FRONT_MATTER_ROWS: u16 = 12;

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Path,
    Toggle,
    FrontMatter,
    Button(usize),
}

/// What the hint bar's hints do.
#[derive(Clone, Copy)]
pub enum FormCmd {
    Submit,
    Cancel,
    Complete,
    /// Keep the path Tab went to, or stop listing matches.
    Keep,
    Toggle,
    Next,
}

pub enum Outcome {
    None,
    Submit,
    Cancel,
}

struct FrontMatter {
    on: bool,
    text: TextArea,
}

pub struct FileDialog {
    title: &'static str,
    intro: Line<'static>,
    help: &'static str,
    /// The label of the button that goes ahead, e.g. "Export".
    submit: &'static str,
    path: PathField,
    front_matter: Option<FrontMatter>,
    focus: Focus,
    /// Why going ahead didn't work, shown under the path.
    error: Option<String>,
    path_area: Rect,
    toggle_area: Rect,
    buttons: Vec<Rect>,
}

impl FileDialog {
    pub fn new(title: &'static str, intro: &'static str, help: &'static str, submit: &'static str, path: &str) -> Self {
        FileDialog {
            title,
            intro: Line::from(intro),
            help,
            submit,
            path: PathField::new(path),
            front_matter: None,
            focus: Focus::Path,
            error: None,
            path_area: Rect::default(),
            toggle_area: Rect::default(),
            buttons: Vec::new(),
        }
    }

    /// Add the front matter, `on` or off at first.
    pub fn with_front_matter(mut self, text: &str, on: bool) -> Self {
        self.front_matter = Some(FrontMatter { on, text: TextArea::new(text) });
        self
    }

    pub fn path(&self) -> &str {
        self.path.value()
    }

    /// Whether the front matter is on, and what it says.
    pub fn front_matter(&self) -> Option<(bool, String)> {
        self.front_matter.as_ref().map(|f| (f.on, f.text.text()))
    }

    /// Say why going ahead didn't work, and go back to the path.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.focus = Focus::Path;
    }

    /// The parts that can have the focus, in order.
    fn order(&self) -> Vec<Focus> {
        let mut order = vec![Focus::Path];
        if let Some(front_matter) = &self.front_matter {
            order.push(Focus::Toggle);
            if front_matter.on {
                order.push(Focus::FrontMatter);
            }
        }
        order.extend([Focus::Button(0), Focus::Button(1)]);
        order
    }

    /// Move the focus to the next part (or the one before, if `back`).
    fn step(&mut self, back: bool) {
        let order = self.order();
        let at = order.iter().position(|f| *f == self.focus).unwrap_or(0);
        let next = if back { at.saturating_sub(1) } else { (at + 1).min(order.len() - 1) };
        self.focus_on(order[next], back);
    }

    fn focus_on(&mut self, focus: Focus, from_below: bool) {
        if focus == Focus::FrontMatter
            && self.focus != Focus::FrontMatter
            && let Some(front_matter) = &mut self.front_matter
        {
            front_matter.text.enter_at(from_below);
        }
        if focus != Focus::Path {
            self.path.done();
        }
        self.focus = focus;
    }

    fn toggle(&mut self) {
        if let Some(front_matter) = &mut self.front_matter {
            front_matter.on = !front_matter.on;
            self.focus = Focus::Toggle;
        }
    }

    pub fn hints(&self) -> Vec<Hint<FormCmd>> {
        let submit = self.submit.to_lowercase();
        let cancel = hint("Esc", "cancel", FormCmd::Cancel);
        match self.focus {
            Focus::Path if self.path.picking() => {
                vec![hint("Enter", "keep", FormCmd::Keep), hint("Tab", "next", FormCmd::Complete), hint("Esc", "close list", FormCmd::Keep)]
            }
            Focus::Path if self.path.listing() => {
                vec![hint("Tab", "pick", FormCmd::Complete), hint("Enter", submit, FormCmd::Submit), hint("Esc", "close list", FormCmd::Keep)]
            }
            Focus::Path => {
                let mut hints = vec![hint("Tab", "complete", FormCmd::Complete), hint("Enter", submit, FormCmd::Submit)];
                if self.front_matter.is_some() {
                    hints.push(hint("↓", "front matter", FormCmd::Next));
                }
                hints.push(cancel);
                hints
            }
            Focus::Toggle => vec![hint("Space", "on/off", FormCmd::Toggle), hint("Ctrl+S", submit, FormCmd::Submit), cancel],
            Focus::FrontMatter => vec![hint("Ctrl+S", submit, FormCmd::Submit), cancel],
            Focus::Button(i) => {
                let label = if i == 0 { submit } else { "cancel".into() };
                let cmd = if i == 0 { FormCmd::Submit } else { FormCmd::Cancel };
                vec![hint("Enter", label, cmd), hint("Esc", "cancel", FormCmd::Cancel)]
            }
        }
    }

    /// Do what a hint does.
    pub fn run(&mut self, cmd: FormCmd) -> Outcome {
        match cmd {
            FormCmd::Submit => return Outcome::Submit,
            FormCmd::Cancel => return Outcome::Cancel,
            FormCmd::Complete => {
                self.focus = Focus::Path;
                self.path.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            }
            FormCmd::Keep => self.path.done(),
            FormCmd::Toggle => self.toggle(),
            FormCmd::Next => self.step(false),
        }
        Outcome::None
    }

    pub fn handle(&mut self, event: Event) -> Outcome {
        match event {
            Event::Key(key) => self.key(key),
            Event::Paste(text) => {
                match self.focus {
                    Focus::FrontMatter => {
                        if let Some(front_matter) = &mut self.front_matter {
                            front_matter.text.insert(&text);
                        }
                    }
                    _ => {
                        self.focus = Focus::Path;
                        self.error = None;
                        self.path.paste(&text);
                    }
                }
                Outcome::None
            }
            Event::Mouse(mouse) => {
                let (column, row) = (mouse.column, mouse.row);
                match mouse.kind {
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                        let lines = if mouse.kind == MouseEventKind::ScrollUp { -1 } else { 1 };
                        if let Some(front_matter) = &mut self.front_matter {
                            front_matter.text.scroll(column, row, lines);
                        }
                    }
                    MouseEventKind::Down(MouseButton::Left) => return self.click(column, row),
                    _ => {}
                }
                Outcome::None
            }
            _ => Outcome::None,
        }
    }

    fn click(&mut self, column: u16, row: u16) -> Outcome {
        let pos = Position::new(column, row);
        if let Some(i) = self.buttons.iter().position(|rect| rect.contains(pos)) {
            return if i == 0 { Outcome::Submit } else { Outcome::Cancel };
        }
        if self.path.click(column, row) || self.path_area.contains(pos) {
            self.focus = Focus::Path;
        } else if self.toggle_area.contains(pos) {
            self.toggle();
        } else if let Some(front_matter) = &mut self.front_matter
            && front_matter.on
            && front_matter.text.click(column, row)
        {
            self.path.done();
            self.focus = Focus::FrontMatter;
        }
        Outcome::None
    }

    fn key(&mut self, key: KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc if self.path.listing() => {
                self.path.done();
                return Outcome::None;
            }
            KeyCode::Esc => return Outcome::Cancel,
            // Like Ctrl+S, but in terminals that report Ctrl+Shift+S.
            KeyCode::Char('s' | 'S') if ctrl => return Outcome::Submit,
            _ => {}
        }
        match self.focus {
            Focus::Path => match key.code {
                KeyCode::Enter if self.path.picking() => self.path.done(),
                KeyCode::Enter => return Outcome::Submit,
                KeyCode::Down => self.step(false),
                _ => {
                    if self.path.key(key) {
                        self.error = None;
                    }
                }
            },
            Focus::Toggle => match key.code {
                KeyCode::Char(' ') | KeyCode::Enter => self.toggle(),
                KeyCode::Up | KeyCode::BackTab => self.step(true),
                KeyCode::Down | KeyCode::Tab => self.step(false),
                _ => {}
            },
            Focus::FrontMatter => {
                let front_matter = self.front_matter.as_mut().expect("front matter has the focus");
                match key.code {
                    KeyCode::Up if front_matter.text.at_first_line() => self.step(true),
                    KeyCode::Down if front_matter.text.at_last_line() => self.step(false),
                    KeyCode::BackTab => self.step(true),
                    _ => {
                        front_matter.text.key(key);
                    }
                }
            }
            Focus::Button(i) => match key.code {
                KeyCode::Enter => return if i == 0 { Outcome::Submit } else { Outcome::Cancel },
                KeyCode::Left | KeyCode::Right | KeyCode::Tab => self.focus = Focus::Button(1 - i),
                KeyCode::Up | KeyCode::BackTab => self.focus_on(
                    match &self.front_matter {
                        Some(front_matter) if front_matter.on => Focus::FrontMatter,
                        Some(_) => Focus::Toggle,
                        None => Focus::Path,
                    },
                    true,
                ),
                _ => {}
            },
        }
        Outcome::None
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let screen = frame.area();
        // Borders, blank, intro, blank, path, its info, blank; the front
        // matter's switch, `---`, its lines, `---`, blank; the buttons.
        let fixed: u16 = 2 + 1 + 1 + 1 + 2 + 1 + 1;
        let mut text_rows = 0;
        let mut extra = 0;
        if let Some(front_matter) = &self.front_matter {
            extra = 2;
            if front_matter.on {
                let room = screen.height.saturating_sub(fixed + extra + 2).max(1);
                text_rows = (front_matter.text.line_count() as u16 + 1).clamp(3, MAX_FRONT_MATTER_ROWS).min(room);
                extra += 2 + text_rows;
            }
        }
        let area = centered(screen, WIDTH, fixed + extra);
        frame.render_widget(Clear, area);
        frame.render_widget(Block::bordered().title(format!(" {} ", self.title)), area);
        let inner = Rect { x: area.x + 2, y: area.y + 1, width: area.width.saturating_sub(4), height: area.height.saturating_sub(2) };
        let row = |offset: u16| Rect { y: inner.y + offset, height: 1, ..inner }.intersection(inner);

        frame.render_widget(Paragraph::new(self.intro.clone()), row(1));
        self.path_area = row(3);
        let error = self.error.as_deref();
        self.path.render(frame, row(3), row(4), self.focus == Focus::Path, self.help, error);

        let mut y = 6;
        self.toggle_area = Rect::default();
        if let Some(front_matter) = &mut self.front_matter {
            let mark = Span::styled(
                if front_matter.on { "[x]" } else { "[ ]" },
                if self.focus == Focus::Toggle { Style::new().reversed() } else { Style::new() },
            );
            let note = if front_matter.on { "  goes first, in place of the # title line" } else { "" };
            let line = Line::from(vec![mark, Span::raw(" Front matter"), Span::raw(note).dim()]);
            self.toggle_area = row(y);
            frame.render_widget(Paragraph::new(line), row(y));
            y += 1;
            if front_matter.on {
                frame.render_widget(Paragraph::new("---".dim()), row(y));
                let text_area = Rect { y: inner.y + y + 1, height: text_rows, ..inner }.intersection(inner);
                let focused = self.focus == Focus::FrontMatter;
                front_matter.text.render(frame, text_area, focused, focused);
                frame.render_widget(Paragraph::new("---".dim()), row(y + 1 + text_rows));
                y += 2 + text_rows;
            }
            y += 1;
        }

        // The buttons.
        self.buttons.clear();
        let mut spans = Vec::new();
        let mut x = inner.x;
        // On a short screen, the buttons stay on the bottom row.
        let buttons_row = row(y.min(inner.height.saturating_sub(1)));
        for (i, label) in [self.submit, "Cancel"].into_iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  "));
                x += 2;
            }
            let text = format!("[ {label} ]");
            let width = text.chars().count() as u16;
            let style = if self.focus == Focus::Button(i) { Style::new().reversed().bold() } else { Style::new() };
            spans.push(Span::styled(text, style));
            self.buttons.push(Rect::new(x, buttons_row.y, width, 1).intersection(buttons_row));
            x += width;
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), buttons_row);
    }
}
