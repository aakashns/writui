//! The first screen: all posts, most recently updated first, under "New",
//! "Import" and a field that searches them. The same screen also shows the
//! Trash.

use jiff::Timestamp;
use jiff::tz::TimeZone;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List as ListWidget, ListItem, ListState, Paragraph};
use unicode_width::UnicodeWidthStr;

use std::path::PathBuf;

use super::dialog::{Button, Dialog};
use super::file_dialog::{FileDialog, FormCmd, Outcome as FormOutcome};
use super::{Action, ImportInto};
use super::hints::{HintBar, hint};
use super::widgets::{COLUMN_WIDTH, column, truncate};
use crate::vault::{DELETED_RETENTION_DAYS, PostSummary};

/// The longest search that can be typed.
const MAX_FILTER: usize = 80;

pub enum Mode {
    /// The normal list, with a "Trash" row if anything is there.
    Posts { deleted_count: usize },
    Trash,
}

#[derive(Clone, Copy)]
enum Cmd {
    Open,
    New,
    Import,
    /// Into the search field.
    Search,
    /// Empty the search field.
    ClearSearch,
    Settings,
    /// A hint of the import dialog.
    Form(FormCmd),
    /// The file was imported before: update that post, or make a new one.
    UpdateImported,
    ImportAsNew,
    Delete,
    /// Esc on the list: asks first.
    Quit,
    ConfirmQuit,
    Back,
    ConfirmDelete,
    CancelDialog,
}

#[derive(Clone, Copy, PartialEq)]
enum Row {
    New,
    Import,
    /// The search field: typing goes into it while it's highlighted.
    Search,
    Post(usize),
    Trash,
    Settings,
}

pub struct List {
    mode: Mode,
    posts: Vec<PostSummary>,
    rows: Vec<Row>,
    state: ListState,
    /// What's typed in the search field.
    filter: String,
    /// The posts (indexes into `posts`) that match `filter`, once the
    /// vault has said; `None` while not searching.
    matches: Option<Vec<usize>>,
    dialog: Option<(usize, Dialog<Cmd>)>,
    /// The import dialog, open.
    importing: Option<Box<FileDialog>>,
    /// Where the import dialog starts.
    import_folder: String,
    /// The file being imported was imported before, into this post: asking
    /// what to do.
    reimport: Option<(PathBuf, i64)>,
    notice: Option<String>,
    rows_area: Rect,
    hints: HintBar<Cmd>,
}

impl List {
    /// `select` is the post to highlight (e.g. the one just closed);
    /// otherwise the first post, so Enter picks up where you left off.
    pub fn new(mode: Mode, posts: Vec<PostSummary>, select: Option<i64>) -> Self {
        let mut list = List {
            mode,
            posts,
            rows: Vec::new(),
            state: ListState::default(),
            filter: String::new(),
            matches: None,
            dialog: None,
            importing: None,
            import_folder: String::new(),
            reimport: None,
            notice: None,
            rows_area: Rect::default(),
            hints: HintBar::default(),
        };
        list.rebuild_rows();
        let selected = select
            .and_then(|id| list.posts.iter().position(|p| p.id == id))
            .or(if list.posts.is_empty() { None } else { Some(0) })
            .and_then(|i| list.rows.iter().position(|r| *r == Row::Post(i)))
            .unwrap_or(0);
        list.state.select(Some(selected));
        list
    }

    /// The rows for the posts shown (all of them, or the search's matches),
    /// keeping the highlighted row if it's still there.
    fn rebuild_rows(&mut self) {
        let selected = self.selected_row();
        let mut rows = Vec::new();
        if matches!(self.mode, Mode::Posts { .. }) {
            rows.extend([Row::New, Row::Import, Row::Search]);
        }
        match &self.matches {
            Some(matches) => rows.extend(matches.iter().map(|&i| Row::Post(i))),
            None => rows.extend((0..self.posts.len()).map(Row::Post)),
        }
        if let Mode::Posts { deleted_count } = self.mode
            && !self.searching()
        {
            if deleted_count > 0 {
                rows.push(Row::Trash);
            }
            rows.push(Row::Settings);
        }
        self.rows = rows;
        let index = selected
            .and_then(|row| self.rows.iter().position(|r| *r == row))
            .or_else(|| self.rows.iter().position(|r| matches!(r, Row::Post(_))))
            .unwrap_or(0);
        self.state.select(Some(index));
    }

    /// Something is typed in the search field.
    fn searching(&self) -> bool {
        !self.filter.is_empty()
    }

    /// The posts matching the search `filter`, by id, as the vault found
    /// them (ignored if the search has changed since).
    pub fn show_matches(&mut self, filter: &str, ids: &[i64]) {
        if filter != self.filter {
            return;
        }
        let matches = ids.iter().filter_map(|id| self.posts.iter().position(|p| p.id == *id)).collect();
        self.matches = Some(matches);
        self.rebuild_rows();
    }

    /// Change the search; the app looks for the matches.
    fn set_filter(&mut self, filter: String) -> Action {
        let filter: String = filter.chars().filter(|ch| !ch.is_control()).take(MAX_FILTER).collect();
        if filter == self.filter {
            return Action::None;
        }
        self.filter = filter;
        if self.filter.trim().is_empty() {
            self.matches = None;
            self.rebuild_rows();
            return Action::None;
        }
        Action::Search(self.filter.clone())
    }

    /// A one-line message shown above the hint bar, e.g. after deleting.
    pub fn with_notice(mut self, notice: String) -> Self {
        self.notice = Some(notice);
        self
    }

    /// The folder the import dialog starts in, e.g. `~/blog/`.
    pub fn with_import_folder(mut self, folder: String) -> Self {
        self.import_folder = folder;
        self
    }

    /// Importing didn't work: say why in the import dialog.
    pub fn import_failed(&mut self, error: String) {
        self.dialog = None;
        self.reimport = None;
        match &mut self.importing {
            Some(form) => form.set_error(error),
            None => self.notice = Some(error),
        }
    }

    /// The file was imported before, into post `id`: ask whether to update
    /// that post or make a new one.
    pub fn ask_reimport(&mut self, path: PathBuf, id: i64, title: &str) {
        let title = truncate(display_title(title), 30);
        let dialog = Dialog::new(
            "Imported before",
            vec![
                Line::from(format!("“{title}” came from this file, or went to it.")),
                Line::from("Update it with the file's text? Its current text".dim()),
                Line::from("is saved as a version first, so you can go back.".dim()),
            ],
            vec![
                Button { label: "Update it", key: "u", cmd: Cmd::UpdateImported, danger: false },
                Button { label: "New post", key: "n", cmd: Cmd::ImportAsNew, danger: false },
                Button { label: "Cancel", key: "c", cmd: Cmd::CancelDialog, danger: false },
            ],
            0,
            2,
        );
        self.dialog = Some((0, dialog));
        self.reimport = Some((path, id));
    }

    pub fn trash_view(&self) -> bool {
        matches!(self.mode, Mode::Trash)
    }

    fn selected_row(&self) -> Option<Row> {
        self.state.selected().and_then(|i| self.rows.get(i)).copied()
    }

    fn selected_post(&self) -> Option<usize> {
        match self.selected_row() {
            Some(Row::Post(i)) => Some(i),
            _ => None,
        }
    }

    /// Typing goes into the search field.
    fn in_search(&self) -> bool {
        self.selected_row() == Some(Row::Search)
    }

    /// The first post the search found, which Enter in the field opens.
    fn first_match(&self) -> Option<usize> {
        self.matches.as_ref().and_then(|m| m.first().copied())
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [_, header, subheader, _, rows, notice, _, hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let col = column(rows, COLUMN_WIDTH);

        if self.trash_view() {
            frame.render_widget(Paragraph::new("Trash".bold()), column(header, COLUMN_WIDTH));
            let note = format!("Posts here are deleted forever after {DELETED_RETENTION_DAYS} days.");
            frame.render_widget(Paragraph::new(note.dim()), column(subheader, COLUMN_WIDTH));
        } else {
            frame.render_widget(Paragraph::new("writui".bold()), column(header, COLUMN_WIDTH));
        }

        let now = Timestamp::now();
        let tz = TimeZone::system();
        let width = col.width as usize;
        let in_search = self.in_search();
        let items: Vec<ListItem> = self
            .rows
            .iter()
            .map(|row| match *row {
                Row::New => ListItem::new(Line::from("+ New".bold())),
                Row::Import => ListItem::new(Line::from("↓ Import".bold())),
                Row::Search => {
                    let shown = scrolled(&self.filter, width.saturating_sub(3));
                    let text = if shown.is_empty() { Span::raw("Type to filter…").dim() } else { Span::raw(shown) };
                    let line = Line::from(vec![Span::raw("/ ").bold(), text]);
                    // Underlined while typing goes into it.
                    ListItem::new(if in_search { line.underlined() } else { line })
                }
                Row::Trash => {
                    let Mode::Posts { deleted_count } = self.mode else { unreachable!() };
                    ListItem::new(Line::from(format!("Trash ({deleted_count})").dim()))
                }
                Row::Settings => ListItem::new(Line::from("Settings".dim())),
                Row::Post(i) => {
                    let post = &self.posts[i];
                    let right = match post.deleted_at {
                        Some(deleted_at) => days_left(deleted_at, now),
                        None => when(post.updated_at, now, &tz),
                    };
                    ListItem::new(post_line(&post.title, &right, width))
                }
            })
            .collect();

        let mut list_area = col;
        let shown_posts = self.matches.as_ref().map_or(self.posts.len(), Vec::len);
        if shown_posts == 0 {
            // Rows (if any), a blank line, then the message.
            let text = if self.searching() {
                if self.matches.is_some() { "No posts match." } else { "" }
            } else if self.trash_view() {
                "Nothing here."
            } else {
                "No posts yet."
            };
            let used = items.len() as u16;
            let y = col.y + if used == 0 { 0 } else { used + 1 };
            if y < col.bottom() {
                frame.render_widget(Paragraph::new(text.dim()), Rect { y, height: 1, ..col });
            }
            list_area.height = used.min(col.height);
        }
        self.rows_area = list_area;
        // The search field shows the cursor, not the highlight.
        let highlight = if in_search { Style::new() } else { Style::new().reversed() };
        frame.render_stateful_widget(ListWidget::new(items).highlight_style(highlight), list_area, &mut self.state);
        if in_search
            && self.dialog.is_none()
            && self.importing.is_none()
            && let Some(index) = self.state.selected()
            && index >= self.state.offset()
        {
            let y = list_area.y + (index - self.state.offset()) as u16;
            let x = col.x + 2 + scrolled(&self.filter, width.saturating_sub(3)).width() as u16;
            if y < list_area.bottom() {
                frame.set_cursor_position(Position::new(x.min(col.right().saturating_sub(1)), y));
            }
        }

        if let Some(text) = &self.notice {
            let text = truncate(text, COLUMN_WIDTH as usize);
            frame.render_widget(Paragraph::new(text.dim()), column(notice, COLUMN_WIDTH));
        }

        if let Some(form) = &mut self.importing {
            form.render(frame);
            if self.dialog.is_none() {
                let hints: Vec<_> = form.hints().into_iter().map(|h| hint(h.key, h.label, Cmd::Form(h.cmd))).collect();
                self.hints.render(frame, hint_area, &hints);
                return;
            }
        }
        if let Some((_, dialog)) = &mut self.dialog {
            dialog.render(frame);
            let hints = dialog.hints();
            self.hints.render(frame, hint_area, &hints);
            return;
        }
        let has_post = self.selected_post().is_some();
        let hints = if self.trash_view() {
            let mut hints = Vec::new();
            if has_post {
                hints.push(hint("Enter", "restore", Cmd::Open));
                hints.push(hint("d", "delete forever", Cmd::Delete));
            }
            hints.push(hint("Esc", "back to posts", Cmd::Back));
            hints
        } else if in_search {
            let mut hints = Vec::new();
            if self.first_match().is_some() {
                hints.push(hint("Enter", "open the first", Cmd::Open));
            }
            if self.searching() {
                hints.push(hint("Esc", "clear", Cmd::ClearSearch));
            } else {
                hints.push(hint("Esc", "done", Cmd::Back));
            }
            hints
        } else {
            let mut hints = vec![
                hint("Enter", "open", Cmd::Open),
                hint("n", "new", Cmd::New),
                hint("i", "import", Cmd::Import),
                hint("/", "search", Cmd::Search),
            ];
            if has_post {
                hints.push(hint("d", "delete", Cmd::Delete));
            }
            hints.push(hint("s", "settings", Cmd::Settings));
            if self.searching() {
                hints.push(hint("Esc", "clear search", Cmd::ClearSearch));
            } else {
                hints.push(hint("Esc", "quit", Cmd::Quit));
            }
            hints
        };
        self.hints.render(frame, hint_area, &hints);
    }

    pub fn handle(&mut self, event: Event) -> Action {
        if let Some((_, dialog)) = &mut self.dialog {
            let mut cmd = dialog.handle(event.clone());
            if cmd.is_none()
                && let Event::Mouse(mouse) = event
                && mouse.kind == MouseEventKind::Down(MouseButton::Left)
            {
                cmd = self.hints.hit(mouse.column, mouse.row);
            }
            return cmd.map_or(Action::None, |cmd| self.run(cmd));
        }
        if let Some(form) = &mut self.importing {
            if let Event::Mouse(mouse) = event
                && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && let Some(cmd) = self.hints.hit(mouse.column, mouse.row)
            {
                return self.run(cmd);
            }
            let outcome = form.handle(event);
            return self.form_outcome(outcome);
        }
        if let Event::Key(_) | Event::Mouse(_) = event {
            self.notice = None;
        }
        if self.in_search()
            && let Some(action) = self.search_field(&event)
        {
            return action;
        }
        match event {
            Event::Key(key) => {
                // Single keys, with or without Ctrl (as before 0.12.0).
                let plain = !key.modifiers.intersects(KeyModifiers::ALT | KeyModifiers::SUPER);
                match key.code {
                    KeyCode::Enter => self.run(Cmd::Open),
                    KeyCode::Esc if self.searching() => self.run(Cmd::ClearSearch),
                    KeyCode::Esc if self.trash_view() => self.run(Cmd::Back),
                    KeyCode::Esc => self.run(Cmd::Quit),
                    KeyCode::Char('n') if plain => self.run(Cmd::New),
                    KeyCode::Char('i') if plain => self.run(Cmd::Import),
                    KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => self.run(Cmd::Import),
                    KeyCode::Char('d') if plain => self.run(Cmd::Delete),
                    KeyCode::Char('s') if plain => self.run(Cmd::Settings),
                    KeyCode::Char('/') if plain => self.run(Cmd::Search),
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
                    if !self.rows_area.contains(Position::new(mouse.column, mouse.row)) {
                        return Action::None;
                    }
                    let index = self.state.offset() + (mouse.row - self.rows_area.y) as usize;
                    let Some(&row) = self.rows.get(index) else {
                        return Action::None;
                    };
                    // Buttons act on the first click; posts open on the second.
                    // In the Trash, clicks only select (restore is Enter).
                    let act = match row {
                        Row::Post(_) => !self.trash_view() && self.selected_row() == Some(row),
                        Row::Search => false,
                        _ => true,
                    };
                    self.state.select(Some(index));
                    if act { self.run(Cmd::Open) } else { Action::None }
                }
                _ => Action::None,
            },
            _ => Action::None,
        }
    }

    /// Keys while typing in the search field. `None` for the ones it
    /// leaves to the list (e.g. ↑/↓, clicks).
    fn search_field(&mut self, event: &Event) -> Option<Action> {
        match event {
            Event::Key(key) => {
                let ctrl = key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER);
                match key.code {
                    KeyCode::Esc if self.searching() => Some(self.run(Cmd::ClearSearch)),
                    KeyCode::Esc => Some(self.run(Cmd::Back)),
                    KeyCode::Char('u') if ctrl => Some(self.set_filter(String::new())),
                    KeyCode::Char(ch) if !ctrl => Some(self.set_filter(format!("{}{ch}", self.filter))),
                    KeyCode::Backspace => {
                        let mut filter = self.filter.clone();
                        filter.pop();
                        Some(self.set_filter(filter))
                    }
                    KeyCode::Delete => Some(Action::None),
                    _ => None,
                }
            }
            Event::Paste(text) => {
                let line = text.lines().next().unwrap_or_default();
                Some(self.set_filter(format!("{}{line}", self.filter)))
            }
            _ => None,
        }
    }

    fn move_by(&mut self, delta: isize) -> Action {
        let last = self.rows.len() as isize - 1;
        let current = self.state.selected().unwrap_or(0) as isize;
        self.state.select(Some(current.saturating_add(delta).clamp(0, last.max(0)) as usize));
        Action::None
    }

    /// The post to highlight after `index` disappears from the list.
    fn neighbour_of(&self, index: usize) -> Option<i64> {
        self.posts
            .get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|i| self.posts.get(i)))
            .map(|p| p.id)
    }

    fn run(&mut self, cmd: Cmd) -> Action {
        match cmd {
            Cmd::Quit => {
                let dialog = Dialog::new(
                    "Quit",
                    vec![Line::from("Quit writui?"), Line::from("Your writing is saved.".dim())],
                    vec![
                        Button { label: "Quit", key: "y", cmd: Cmd::ConfirmQuit, danger: false },
                        Button { label: "Cancel", key: "n", cmd: Cmd::CancelDialog, danger: false },
                    ],
                    0,
                    1,
                );
                self.dialog = Some((0, dialog));
                Action::None
            }
            Cmd::ConfirmQuit => Action::Quit,
            Cmd::New if !self.trash_view() => Action::NewPost,
            Cmd::Settings if !self.trash_view() => Action::ShowSettings,
            Cmd::Search if !self.trash_view() => {
                if let Some(index) = self.rows.iter().position(|r| *r == Row::Search) {
                    self.state.select(Some(index));
                }
                Action::None
            }
            Cmd::ClearSearch => self.set_filter(String::new()),
            Cmd::New | Cmd::Settings | Cmd::Search => Action::None,
            Cmd::Import if !self.trash_view() => {
                self.importing = Some(Box::new(FileDialog::new(
                    "Import",
                    "Makes a post of a markdown or text file, front matter and all.",
                    "Tab completes folder and file names.",
                    "Import",
                    &self.import_folder,
                )));
                Action::None
            }
            Cmd::Import => Action::None,
            Cmd::Form(cmd) => match &mut self.importing {
                Some(form) => {
                    let outcome = form.run(cmd);
                    self.form_outcome(outcome)
                }
                None => Action::None,
            },
            Cmd::UpdateImported | Cmd::ImportAsNew => {
                self.dialog = None;
                match self.reimport.take() {
                    Some((path, id)) => {
                        let into = if matches!(cmd, Cmd::UpdateImported) { ImportInto::Post(id) } else { ImportInto::New };
                        Action::Import { path, into }
                    }
                    None => Action::None,
                }
            }
            Cmd::Back if self.trash_view() => Action::BackToList(None),
            Cmd::Back => {
                // Out of the search field, to the posts.
                if self.in_search() {
                    let next = self.state.selected().map_or(0, |i| i + 1);
                    self.state.select(Some(next.min(self.rows.len().saturating_sub(1))));
                }
                Action::None
            }
            Cmd::Open => match (self.selected_row(), self.trash_view()) {
                (Some(Row::New), _) => Action::NewPost,
                (Some(Row::Import), _) => self.run(Cmd::Import),
                (Some(Row::Search), _) => match self.first_match() {
                    Some(i) => Action::OpenPost(self.posts[i].id),
                    None => Action::None,
                },
                (Some(Row::Trash), _) => Action::ShowTrash,
                (Some(Row::Settings), _) => Action::ShowSettings,
                (Some(Row::Post(i)), false) => Action::OpenPost(self.posts[i].id),
                (Some(Row::Post(i)), true) => {
                    Action::RestorePost { id: self.posts[i].id, select: self.neighbour_of(i) }
                }
                (None, _) => Action::None,
            },
            Cmd::Delete => {
                if let Some(index) = self.selected_post() {
                    self.dialog = Some((index, self.delete_dialog(index)));
                }
                Action::None
            }
            Cmd::ConfirmDelete => match self.dialog.take() {
                Some((index, _)) => {
                    let id = self.posts[index].id;
                    let select = self.neighbour_of(index);
                    if self.trash_view() {
                        Action::DeletePostForever { id, select }
                    } else {
                        Action::DeletePost { id, select }
                    }
                }
                None => Action::None,
            },
            Cmd::CancelDialog => {
                self.dialog = None;
                self.reimport = None;
                Action::None
            }
        }
    }

    fn form_outcome(&mut self, outcome: FormOutcome) -> Action {
        let Some(form) = &mut self.importing else { return Action::None };
        match outcome {
            FormOutcome::None => Action::None,
            FormOutcome::Cancel => {
                self.importing = None;
                Action::None
            }
            FormOutcome::Submit => match crate::files::resolve_import(form.path()) {
                Ok(path) => Action::Import { path, into: ImportInto::Check },
                Err(error) => {
                    form.set_error(error);
                    Action::None
                }
            },
        }
    }

    fn delete_dialog(&self, index: usize) -> Dialog<Cmd> {
        let title = display_title(&self.posts[index].title);
        let title = truncate(title, 40);
        if self.trash_view() {
            Dialog::new(
                "Delete forever",
                vec![
                    Line::from(format!("Delete “{title}” forever?")),
                    Line::from("This can't be undone.".dim()),
                ],
                vec![
                    Button { label: "Delete forever", key: "y", cmd: Cmd::ConfirmDelete, danger: true },
                    Button { label: "Cancel", key: "n", cmd: Cmd::CancelDialog, danger: false },
                ],
                1,
                1,
            )
        } else {
            Dialog::new(
                "Delete post",
                vec![
                    Line::from(format!("Move “{title}” to the Trash?")),
                    Line::from(format!("You can restore it for {DELETED_RETENTION_DAYS} days.").dim()),
                ],
                vec![
                    Button { label: "Delete", key: "y", cmd: Cmd::ConfirmDelete, danger: true },
                    Button { label: "Cancel", key: "n", cmd: Cmd::CancelDialog, danger: false },
                ],
                0,
                1,
            )
        }
    }
}

/// The end of `text` that fits in `width` columns, as a field scrolls to
/// keep what's being typed in view.
fn scrolled(text: &str, width: usize) -> &str {
    let mut shown = text;
    while shown.width() > width {
        let mut chars = shown.chars();
        chars.next();
        shown = chars.as_str();
    }
    shown
}

pub fn display_title(title: &str) -> &str {
    if title.is_empty() { "Untitled" } else { title }
}

/// A post row: title on the left, `right` (a time) right-aligned and dim.
fn post_line(title: &str, right: &str, width: usize) -> Line<'static> {
    let right_width = right.chars().count();
    let title_width = width.saturating_sub(right_width + 2);
    let title = if title.is_empty() {
        Span::raw("Untitled").dim().italic()
    } else {
        Span::raw(truncate(title, title_width))
    };
    let pad = width.saturating_sub(title.width() + right_width);
    Line::from(vec![title, Span::raw(" ".repeat(pad)), Span::raw(right.to_string()).dim()])
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

/// When a version was saved, to the minute: "just now", "5 min ago", "2:05 PM",
/// "Oct 2, 2:05 PM", "Oct 2, 2025, 2:05 PM".
pub fn version_time(ts: Timestamp, now: Timestamp, tz: &TimeZone) -> String {
    let secs = now.duration_since(ts).as_secs();
    if secs < 60 * 60 {
        return when(ts, now, tz);
    }
    let then = ts.to_zoned(tz.clone());
    let today = now.to_zoned(tz.clone()).date();
    if then.date() == today {
        then.strftime("%-I:%M %p").to_string()
    } else if then.year() == today.year() {
        then.strftime("%b %-d, %-I:%M %p").to_string()
    } else {
        then.strftime("%b %-d, %Y, %-I:%M %p").to_string()
    }
}

/// How long until a deleted post is gone for good: "30 days left".
fn days_left(deleted_at: Timestamp, now: Timestamp) -> String {
    let days_gone = now.duration_since(deleted_at).as_secs() / (24 * 60 * 60);
    match (DELETED_RETENTION_DAYS - days_gone).max(1) {
        1 => "1 day left".into(),
        n => format!("{n} days left"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyEvent;

    fn screen(list: &mut List) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| list.render(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..24).map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    fn press(list: &mut List, code: KeyCode, modifiers: KeyModifiers) -> Action {
        list.handle(Event::Key(KeyEvent::new(code, modifiers)))
    }

    #[test]
    fn import_is_under_new_post() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("post.md");
        std::fs::write(&file, "# Post").unwrap();
        let folder = format!("{}/", dir.path().display());
        let mut list = List::new(Mode::Posts { deleted_count: 0 }, Vec::new(), None).with_import_folder(folder.clone());
        let text = screen(&mut list);
        assert!(text.contains("+ New") && text.contains("↓ Import"));
        assert!(text.contains("i import"));
        // Down to it, Enter: the dialog, starting in the folder.
        press(&mut list, KeyCode::Down, KeyModifiers::NONE);
        press(&mut list, KeyCode::Enter, KeyModifiers::NONE);
        let text = screen(&mut list);
        assert!(text.contains("markdown or text file") && text.contains(&folder), "{text}");
        assert!(text.contains("Tab complete"));
        // A folder isn't a file; Tab completes the one file in it.
        assert!(matches!(press(&mut list, KeyCode::Enter, KeyModifiers::NONE), Action::None));
        assert!(screen(&mut list).contains("That's a folder"));
        press(&mut list, KeyCode::Tab, KeyModifiers::NONE);
        let Action::Import { path, into: ImportInto::Check } = press(&mut list, KeyCode::Enter, KeyModifiers::NONE) else {
            panic!("not imported")
        };
        assert_eq!(path, file);
        // Imported before: asks.
        list.ask_reimport(file, 7, "Post");
        let text = screen(&mut list);
        assert!(text.contains("Imported before") && text.contains("Update it"));
        assert!(matches!(press(&mut list, KeyCode::Char('u'), KeyModifiers::NONE), Action::Import { into: ImportInto::Post(7), .. }));
        // Esc closes the dialog; i opens it again (and Ctrl+O, as before).
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        assert!(list.importing.is_none());
        press(&mut list, KeyCode::Char('i'), KeyModifiers::NONE);
        assert!(list.importing.is_some());
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        press(&mut list, KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert!(list.importing.is_some());
    }

    fn post(id: i64, title: &str) -> PostSummary {
        PostSummary { id, title: title.into(), updated_at: Timestamp::now(), deleted_at: None }
    }

    #[test]
    fn single_keys_and_esc_asks_before_quitting() {
        let posts = vec![post(1, "One"), post(2, "Two")];
        let mut list = List::new(Mode::Posts { deleted_count: 0 }, posts, None);
        let text = screen(&mut list);
        assert!(text.contains("n new") && text.contains("/ search") && text.contains("Esc quit"), "{text}");
        assert!(!text.contains("Ctrl+Q"));
        assert!(matches!(press(&mut list, KeyCode::Char('n'), KeyModifiers::NONE), Action::NewPost));
        assert!(matches!(press(&mut list, KeyCode::Char('s'), KeyModifiers::NONE), Action::ShowSettings));
        press(&mut list, KeyCode::Char('d'), KeyModifiers::NONE);
        assert!(screen(&mut list).contains("Move “One” to the Trash?"));
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        // Esc: asks; n keeps going, y quits.
        assert!(matches!(press(&mut list, KeyCode::Esc, KeyModifiers::NONE), Action::None));
        assert!(screen(&mut list).contains("Quit writui?"));
        assert!(matches!(press(&mut list, KeyCode::Char('n'), KeyModifiers::NONE), Action::None));
        assert!(list.dialog.is_none());
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        assert!(matches!(press(&mut list, KeyCode::Char('y'), KeyModifiers::NONE), Action::Quit));
    }

    #[test]
    fn typing_in_the_search_field_searches() {
        let posts = vec![post(1, "One"), post(2, "Two"), post(3, "Three")];
        let mut list = List::new(Mode::Posts { deleted_count: 2 }, posts, None);
        assert!(screen(&mut list).contains("Type to filter…"));
        // / goes into the field; letters are typed there, not shortcuts.
        press(&mut list, KeyCode::Char('/'), KeyModifiers::NONE);
        assert!(matches!(press(&mut list, KeyCode::Char('n'), KeyModifiers::NONE), Action::Search(f) if f == "n"));
        let Action::Search(filter) = press(&mut list, KeyCode::Char('e'), KeyModifiers::NONE) else { panic!() };
        assert_eq!(filter, "ne");
        // Until the vault answers, nothing is said; then only the matches.
        list.show_matches("ne", &[3, 1]);
        let text = screen(&mut list);
        assert!(text.contains("/ ne") && text.contains("Three") && text.contains("One"), "{text}");
        assert!(!text.contains("Two") && !text.contains("Trash") && !text.contains("Settings"), "{text}");
        assert!(text.contains("Enter open the first"));
        // A stale answer is ignored.
        list.show_matches("n", &[1, 2, 3]);
        assert!(!screen(&mut list).contains("Two"));
        // Enter opens the first match.
        assert!(matches!(press(&mut list, KeyCode::Enter, KeyModifiers::NONE), Action::OpenPost(3)));
        // No matches.
        press(&mut list, KeyCode::Char('x'), KeyModifiers::NONE);
        list.show_matches("nex", &[]);
        assert!(screen(&mut list).contains("No posts match."));
        assert!(matches!(press(&mut list, KeyCode::Enter, KeyModifiers::NONE), Action::None));
        // Esc clears it, then leaves the field; everything's back.
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        assert!(list.filter.is_empty() && list.in_search());
        let text = screen(&mut list);
        assert!(text.contains("Two") && text.contains("Trash (2)") && text.contains("Settings"));
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(list.selected_post(), Some(0));
        // ↓ from the field goes to the results, where letters are shortcuts.
        press(&mut list, KeyCode::Char('/'), KeyModifiers::NONE);
        press(&mut list, KeyCode::Char('t'), KeyModifiers::NONE);
        list.show_matches("t", &[2, 3]);
        press(&mut list, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(list.selected_post(), Some(1));
        assert!(matches!(press(&mut list, KeyCode::Char('n'), KeyModifiers::NONE), Action::NewPost));
        // Esc there clears the search first, keeping the post highlighted.
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        assert!(list.filter.is_empty());
        assert_eq!(list.selected_post(), Some(1));
    }

    #[test]
    fn when_reads_naturally() {
        let tz = TimeZone::UTC;
        let now = at("2026-10-03T14:30:00Z");
        assert_eq!(when(at("2026-10-03T14:29:30Z"), now, &tz), "just now");
        assert_eq!(when(at("2026-10-03T14:05:00Z"), now, &tz), "25 min ago");
        assert_eq!(when(at("2026-10-03T09:05:00Z"), now, &tz), "9:05 AM");
        assert_eq!(when(at("2026-10-02T23:00:00Z"), now, &tz), "Yesterday");
        assert_eq!(when(at("2026-03-14T10:00:00Z"), now, &tz), "Mar 14");
        assert_eq!(when(at("2025-03-14T10:00:00Z"), now, &tz), "Mar 14, 2025");
    }

    #[test]
    fn version_times_are_exact() {
        let tz = TimeZone::UTC;
        let now = at("2026-10-03T14:30:00Z");
        assert_eq!(version_time(at("2026-10-03T14:05:00Z"), now, &tz), "25 min ago");
        assert_eq!(version_time(at("2026-10-03T09:05:00Z"), now, &tz), "9:05 AM");
        assert_eq!(version_time(at("2026-10-02T23:00:00Z"), now, &tz), "Oct 2, 11:00 PM");
        assert_eq!(version_time(at("2025-03-14T10:00:00Z"), now, &tz), "Mar 14, 2025, 10:00 AM");
    }

    #[test]
    fn days_left_counts_down() {
        let now = at("2026-10-03T14:30:00Z");
        assert_eq!(days_left(at("2026-10-03T14:00:00Z"), now), "30 days left");
        assert_eq!(days_left(at("2026-10-01T14:00:00Z"), now), "28 days left");
        assert_eq!(days_left(at("2026-09-04T15:00:00Z"), now), "2 days left");
        assert_eq!(days_left(at("2026-09-03T15:00:00Z"), now), "1 day left");
    }
}
