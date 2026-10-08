//! A post's history: its versions, newest first. Open one to read its full
//! text, and restore it from there.
//!
//! It's shown by the editor, on top of the post, so the post (and its undo
//! history) is still there when you come back.

use jiff::Timestamp;
use jiff::tz::TimeZone;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use ropey::Rope;

use super::super::dialog::{Button, Dialog};
use super::super::hints::{HintBar, hint};
use super::super::list::{display_title, version_time};
use super::super::widgets::{COLUMN_WIDTH, column, truncate};
use super::markdown::Markup;
use super::row_line;
use super::wrap::{Row, layout};
use crate::vault::Version;

/// What the editor should do after an event.
pub enum Outcome {
    None,
    /// Back to the post.
    Close,
    /// Load this version's full text, to read it.
    Load(i64),
    /// Replace the post with this version.
    Restore(Version),
    Quit,
}

#[derive(Clone, Copy)]
enum Cmd {
    Open,
    Back,
    Restore,
    ConfirmRestore,
    CancelDialog,
    Quit,
}

pub struct History {
    title: String,
    versions: Vec<Version>,
    state: ListState,
    rows_area: Rect,
    /// The version being read, if one is open.
    reading: Option<Reading>,
    /// The post has changed since its latest version.
    changed: bool,
    dialog: Option<Dialog<Cmd>>,
    hints: HintBar<Cmd>,
}

/// A version's full text, laid out like the editor does.
struct Reading {
    version: Version,
    rope: Rope,
    markup: Markup,
    rows: Vec<Row>,
    width: usize,
    top: usize,
    height: usize,
}

impl History {
    pub fn new(title: String, versions: Vec<Version>, changed: bool) -> Self {
        let state = ListState::default().with_selected((!versions.is_empty()).then_some(0));
        History {
            title,
            versions,
            state,
            rows_area: Rect::default(),
            reading: None,
            changed,
            dialog: None,
            hints: HintBar::default(),
        }
    }

    /// Open a version (with its text) to read.
    pub fn read(&mut self, version: Version) {
        let rope = Rope::from_str(&version.body);
        let markup = Markup::new(&rope);
        self.reading = Some(Reading { version, rope, markup, rows: Vec::new(), width: 0, top: 0, height: 0 });
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [_, header, subheader, _, body, _, hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let now = Timestamp::now();
        let tz = TimeZone::system();

        let hints = if let Some(reading) = &mut self.reading {
            let name = version_name(&reading.version.name);
            let width = (COLUMN_WIDTH as usize).min(body.width.saturating_sub(1) as usize).max(1);
            let col = column(header, width as u16);
            frame.render_widget(Paragraph::new(Line::from(name.bold())), col);
            let when = format!("Saved {}", version_time(reading.version.created_at, now, &tz));
            frame.render_widget(Paragraph::new(when.dim()), column(subheader, width as u16));
            reading.render(frame, body, width);
            vec![
                hint("Enter", "restore", Cmd::Restore),
                hint("Esc", "back to versions", Cmd::Back),
                hint("Ctrl+Q", "quit", Cmd::Quit),
            ]
        } else {
            let col = column(body, COLUMN_WIDTH);
            frame.render_widget(Paragraph::new("History".bold()), column(header, COLUMN_WIDTH));
            let of = format!("Versions of “{}”", display_title(&self.title));
            let of = truncate(&of, COLUMN_WIDTH as usize);
            frame.render_widget(Paragraph::new(of.dim()), column(subheader, COLUMN_WIDTH));
            let width = col.width as usize;
            let items: Vec<ListItem> = self
                .versions
                .iter()
                .map(|version| ListItem::new(version_line(version, &version_time(version.created_at, now, &tz), width)))
                .collect();
            self.rows_area = Rect { height: (items.len() as u16).min(col.height), ..col };
            frame.render_stateful_widget(
                List::new(items).highlight_style(Style::new().reversed()),
                self.rows_area,
                &mut self.state,
            );
            vec![
                hint("Enter", "read", Cmd::Open),
                hint("Esc", "back to post", Cmd::Back),
                hint("Ctrl+Q", "quit", Cmd::Quit),
            ]
        };

        if let Some(dialog) = &mut self.dialog {
            dialog.render(frame);
            let hints = dialog.hints();
            self.hints.render(frame, hint_area, &hints);
        } else {
            self.hints.render(frame, hint_area, &hints);
        }
    }

    pub fn handle(&mut self, event: Event) -> Outcome {
        if let Some(dialog) = &mut self.dialog {
            let mut cmd = dialog.handle(event.clone());
            if cmd.is_none()
                && let Event::Mouse(mouse) = event
                && mouse.kind == MouseEventKind::Down(MouseButton::Left)
            {
                cmd = self.hints.hit(mouse.column, mouse.row);
            }
            return cmd.map_or(Outcome::None, |cmd| self.run(cmd));
        }
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Enter if self.reading.is_some() => self.run(Cmd::Restore),
                KeyCode::Enter => self.run(Cmd::Open),
                KeyCode::Esc => self.run(Cmd::Back),
                KeyCode::Up => self.scroll(-1),
                KeyCode::Down => self.scroll(1),
                KeyCode::PageUp => self.scroll(-self.page()),
                KeyCode::PageDown => self.scroll(self.page()),
                KeyCode::Home => self.scroll(isize::MIN / 2),
                KeyCode::End => self.scroll(isize::MAX / 2),
                _ => Outcome::None,
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll(-1),
                MouseEventKind::ScrollDown => self.scroll(1),
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(cmd) = self.hints.hit(mouse.column, mouse.row) {
                        return self.run(cmd);
                    }
                    if self.reading.is_some()
                        || !self.rows_area.contains(Position::new(mouse.column, mouse.row))
                    {
                        return Outcome::None;
                    }
                    // Like the post list: select on the first click, open
                    // on the second.
                    let index = self.state.offset() + (mouse.row - self.rows_area.y) as usize;
                    if self.state.selected() == Some(index) {
                        return self.run(Cmd::Open);
                    }
                    self.state.select(Some(index));
                    Outcome::None
                }
                _ => Outcome::None,
            },
            _ => Outcome::None,
        }
    }

    fn run(&mut self, cmd: Cmd) -> Outcome {
        match cmd {
            Cmd::Open => match self.state.selected().and_then(|i| self.versions.get(i)) {
                Some(version) => Outcome::Load(version.id),
                None => Outcome::None,
            },
            Cmd::Back if self.reading.is_some() => {
                self.reading = None;
                Outcome::None
            }
            Cmd::Back => Outcome::Close,
            Cmd::Restore => {
                if let Some(reading) = &self.reading {
                    self.dialog = Some(restore_dialog(&reading.version, self.changed));
                }
                Outcome::None
            }
            Cmd::ConfirmRestore => {
                self.dialog = None;
                match self.reading.take() {
                    Some(reading) => Outcome::Restore(reading.version),
                    None => Outcome::None,
                }
            }
            Cmd::CancelDialog => {
                self.dialog = None;
                Outcome::None
            }
            Cmd::Quit => Outcome::Quit,
        }
    }

    /// Scroll the text being read, or move through the list of versions.
    fn scroll(&mut self, by: isize) -> Outcome {
        match &mut self.reading {
            Some(reading) => reading.top = reading.top.saturating_add_signed(by).min(reading.max_top()),
            None => {
                let last = self.versions.len().saturating_sub(1);
                let current = self.state.selected().unwrap_or(0);
                self.state.select(Some(current.saturating_add_signed(by).min(last)));
            }
        }
        Outcome::None
    }

    fn page(&self) -> isize {
        let height = match &self.reading {
            Some(reading) => reading.height,
            None => self.rows_area.height as usize,
        };
        height.saturating_sub(1).max(1) as isize
    }
}

impl Reading {
    fn render(&mut self, frame: &mut Frame, body: Rect, width: usize) {
        if width != self.width {
            self.width = width;
            self.rows = layout(&self.rope, width);
        }
        self.height = body.height as usize;
        self.top = self.top.min(self.max_top());
        let left = body.x + body.width.saturating_sub(width as u16) / 2;
        let area = Rect::new(left, body.y, (width as u16 + 1).min(body.right() - left), body.height);
        let lines: Vec<Line> =
            self.rows.iter().skip(self.top).take(self.height).map(|row| row_line(&self.rope, &self.markup, row, None)).collect();
        frame.render_widget(Paragraph::new(lines), area);
    }

    /// The furthest it scrolls: the last row at the bottom.
    fn max_top(&self) -> usize {
        self.rows.len().saturating_sub(self.height)
    }
}

/// A version's name, or a placeholder if it was saved without one.
fn version_name(name: &str) -> Span<'static> {
    if name.is_empty() { Span::raw("No name").italic() } else { Span::raw(name.to_string()) }
}

/// A row in the list: the name on the left, `right` (a time) right-aligned
/// and dim.
fn version_line(version: &Version, right: &str, width: usize) -> Line<'static> {
    let right_width = right.chars().count();
    let room = width.saturating_sub(right_width + 2);
    let name = if version.name.is_empty() {
        version_name("").dim()
    } else {
        Span::raw(truncate(&version.name, room))
    };
    let pad = width.saturating_sub(name.width() + right_width);
    Line::from(vec![name, Span::raw(" ".repeat(pad)), Span::raw(right.to_string()).dim()])
}

fn restore_dialog(version: &Version, changed: bool) -> Dialog<Cmd> {
    let name = if version.name.is_empty() { "this version".to_string() } else { format!("“{}”", truncate(&version.name, 30)) };
    let mut lines = vec![Line::from(format!("Replace the post with {name}?"))];
    if changed {
        lines.push(Line::from("Its current text isn't saved as a version.".dim()));
    }
    lines.push(Line::from("Ctrl+Z in the editor undoes this.".dim()));
    Dialog::new(
        "Restore",
        lines,
        vec![
            Button { label: "Restore", key: "y", cmd: Cmd::ConfirmRestore, danger: false },
            Button { label: "Cancel", key: "n", cmd: Cmd::CancelDialog, danger: false },
        ],
        0,
        1,
    )
}
