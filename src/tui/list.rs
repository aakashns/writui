//! The first screen: all posts, most recently updated first.

use jiff::Timestamp;
use jiff::tz::TimeZone;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List as ListWidget, ListItem, ListState, Paragraph};

use super::Action;
use super::hints::{HintBar, hint};
use super::widgets::{COLUMN_WIDTH, centered, column, truncate};
use crate::vault::PostSummary;

#[derive(Clone, Copy)]
enum Cmd {
    Open,
    New,
    Delete,
    Quit,
    ConfirmDelete,
    CancelDelete,
}

/// Row 0 is "New post"; row `i + 1` is `posts[i]`.
pub struct List {
    posts: Vec<PostSummary>,
    state: ListState,
    /// Index into `posts` of the post waiting for delete confirmation.
    confirm: Option<usize>,
    rows_area: Rect,
    buttons: Vec<(Rect, Cmd)>,
    hints: HintBar<Cmd>,
}

impl List {
    /// `select` is the post to highlight (e.g. the one just closed);
    /// otherwise the most recent post, so Enter picks up where you left off.
    pub fn new(posts: Vec<PostSummary>, select: Option<i64>) -> Self {
        let row = select
            .and_then(|id| posts.iter().position(|p| p.id == id))
            .map(|i| i + 1)
            .unwrap_or(if posts.is_empty() { 0 } else { 1 });
        List {
            posts,
            state: ListState::default().with_selected(Some(row)),
            confirm: None,
            rows_area: Rect::default(),
            buttons: Vec::new(),
            hints: HintBar::default(),
        }
    }

    fn selected_row(&self) -> usize {
        self.state.selected().unwrap_or(0)
    }

    fn selected_post(&self) -> Option<&PostSummary> {
        self.selected_row().checked_sub(1).and_then(|i| self.posts.get(i))
    }

    fn row_count(&self) -> usize {
        self.posts.len() + 1
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [_, header, _, rows, hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let col = column(rows, COLUMN_WIDTH);
        frame.render_widget(Paragraph::new("writui".bold()), column(header, COLUMN_WIDTH));

        let now = Timestamp::now();
        let tz = TimeZone::system();
        let width = col.width as usize;
        let mut items = vec![ListItem::new(Line::from("+ New post".bold()))];
        items.extend(self.posts.iter().map(|post| {
            let when = when(post.updated_at, now, &tz);
            let title_width = width.saturating_sub(when.chars().count() + 2);
            let title = if post.title.is_empty() {
                Span::raw("Untitled").dim().italic()
            } else {
                Span::raw(truncate(&post.title, title_width))
            };
            let pad = width.saturating_sub(title.width() + when.chars().count());
            ListItem::new(Line::from(vec![title, Span::raw(" ".repeat(pad)), Span::raw(when).dim()]))
        }));
        let mut list_area = col;
        if self.posts.is_empty() {
            let [first, _, empty] = Layout::vertical([Constraint::Length(1); 3]).areas(col);
            frame.render_widget(Paragraph::new("No posts yet.".dim()), empty);
            list_area = first;
        }
        self.rows_area = list_area;
        frame.render_stateful_widget(
            ListWidget::new(items).highlight_style(Style::new().reversed()),
            list_area,
            &mut self.state,
        );

        if let Some(index) = self.confirm {
            self.render_confirm(frame, index);
            self.hints.render(
                frame,
                hint_area,
                &[hint("y", "delete", Cmd::ConfirmDelete), hint("n", "cancel", Cmd::CancelDelete)],
            );
        } else {
            self.buttons.clear();
            let mut hints = vec![
                hint("Enter", "open", Cmd::Open),
                hint("Ctrl+N", "new post", Cmd::New),
            ];
            if self.selected_post().is_some() {
                hints.push(hint("Ctrl+D", "delete", Cmd::Delete));
            }
            hints.push(hint("Ctrl+Q", "quit", Cmd::Quit));
            self.hints.render(frame, hint_area, &hints);
        }
    }

    fn render_confirm(&mut self, frame: &mut Frame, index: usize) {
        let title = match self.posts[index].title.as_str() {
            "" => "Untitled",
            title => title,
        };
        let area = centered(frame.area(), 52, 6);
        let inner = Block::bordered().title(" Delete post ").inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(Block::bordered().title(" Delete post "), area);

        let delete = "[ Delete ]";
        let cancel = "[ Cancel ]";
        let lines = vec![
            Line::from(format!("Delete “{}”?", truncate(title, inner.width as usize - 10))),
            Line::from("This can't be undone.".dim()),
            Line::default(),
            Line::from(vec![Span::raw(delete).red().bold(), Span::raw("  "), Span::raw(cancel)]),
        ];
        let inner = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
        frame.render_widget(Paragraph::new(lines), inner);
        let y = inner.y + 3;
        self.buttons = vec![
            (Rect::new(inner.x, y, delete.len() as u16, 1), Cmd::ConfirmDelete),
            (Rect::new(inner.x + delete.len() as u16 + 2, y, cancel.len() as u16, 1), Cmd::CancelDelete),
        ];
    }

    pub fn handle(&mut self, event: Event) -> Action {
        if self.confirm.is_some() {
            return self.handle_confirm(event);
        }
        match event {
            Event::Key(key) => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Enter => self.run(Cmd::Open),
                    KeyCode::Char('n') if ctrl => self.run(Cmd::New),
                    KeyCode::Char('d') if ctrl => self.run(Cmd::Delete),
                    KeyCode::Delete => self.run(Cmd::Delete),
                    KeyCode::Up => self.move_by(-1),
                    KeyCode::Down => self.move_by(1),
                    KeyCode::PageUp => self.move_by(-(self.rows_area.height as isize)),
                    KeyCode::PageDown => self.move_by(self.rows_area.height as isize),
                    KeyCode::Home => self.move_by(isize::MIN / 2),
                    KeyCode::End => self.move_by(isize::MAX / 2),
                    _ => Action::None,
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.move_by(-1),
                MouseEventKind::ScrollDown => self.move_by(1),
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(cmd) = self.hints.hit(mouse.column, mouse.row) {
                        return self.run(cmd);
                    }
                    let pos = Position::new(mouse.column, mouse.row);
                    if !self.rows_area.contains(pos) {
                        return Action::None;
                    }
                    let row = self.state.offset() + (mouse.row - self.rows_area.y) as usize;
                    if row >= self.row_count() {
                        return Action::None;
                    }
                    // "New post" is a button; posts open on the second click.
                    if row == 0 || row == self.selected_row() {
                        self.state.select(Some(row));
                        self.run(Cmd::Open)
                    } else {
                        self.state.select(Some(row));
                        Action::None
                    }
                }
                _ => Action::None,
            },
            _ => Action::None,
        }
    }

    fn handle_confirm(&mut self, event: Event) -> Action {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Char('y') => self.run(Cmd::ConfirmDelete),
                KeyCode::Char('n') | KeyCode::Esc => self.run(Cmd::CancelDelete),
                _ => Action::None,
            },
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                let pos = Position::new(mouse.column, mouse.row);
                let cmd = self
                    .buttons
                    .iter()
                    .find(|(rect, _)| rect.contains(pos))
                    .map(|(_, cmd)| *cmd)
                    .or_else(|| self.hints.hit(mouse.column, mouse.row));
                match cmd {
                    Some(cmd) => self.run(cmd),
                    None => Action::None,
                }
            }
            _ => Action::None,
        }
    }

    fn move_by(&mut self, delta: isize) -> Action {
        let last = self.row_count() as isize - 1;
        let row = (self.selected_row() as isize).saturating_add(delta).clamp(0, last);
        self.state.select(Some(row as usize));
        Action::None
    }

    fn run(&mut self, cmd: Cmd) -> Action {
        match cmd {
            Cmd::Quit => Action::Quit,
            Cmd::New => Action::NewPost,
            Cmd::Open => match self.selected_post() {
                Some(post) => Action::OpenPost(post.id),
                None => Action::NewPost,
            },
            Cmd::Delete => {
                self.confirm = self.selected_row().checked_sub(1);
                Action::None
            }
            Cmd::ConfirmDelete => match self.confirm.take() {
                Some(index) => Action::DeletePost(self.posts[index].id),
                None => Action::None,
            },
            Cmd::CancelDelete => {
                self.confirm = None;
                Action::None
            }
        }
    }
}

/// Short, human "last updated" text: "just now", "5 min ago", "2:05 PM",
/// "Yesterday", "Oct 2", "Oct 2, 2025".
fn when(ts: Timestamp, now: Timestamp, tz: &TimeZone) -> String {
    let secs = now.duration_since(ts).as_secs();
    if secs < 60 {
        return "just now".into();
    }
    if secs < 60 * 60 {
        return format!("{} min ago", secs / 60);
    }
    let then = ts.to_zoned(tz.clone());
    let today = now.to_zoned(tz.clone()).date();
    if then.date() == today {
        then.strftime("%-I:%M %p").to_string()
    } else if today.yesterday().is_ok_and(|y| y == then.date()) {
        "Yesterday".into()
    } else if then.year() == today.year() {
        then.strftime("%b %-d").to_string()
    } else {
        then.strftime("%b %-d, %Y").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_reads_naturally() {
        let tz = TimeZone::UTC;
        let now: Timestamp = "2026-10-03T14:30:00Z".parse().unwrap();
        let at = |s: &str| s.parse::<Timestamp>().unwrap();
        assert_eq!(when(at("2026-10-03T14:29:30Z"), now, &tz), "just now");
        assert_eq!(when(at("2026-10-03T14:05:00Z"), now, &tz), "25 min ago");
        assert_eq!(when(at("2026-10-03T09:05:00Z"), now, &tz), "9:05 AM");
        assert_eq!(when(at("2026-10-02T23:00:00Z"), now, &tz), "Yesterday");
        assert_eq!(when(at("2026-03-14T10:00:00Z"), now, &tz), "Mar 14");
        assert_eq!(when(at("2025-03-14T10:00:00Z"), now, &tz), "Mar 14, 2025");
    }
}
