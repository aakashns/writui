//! The first screen: all posts, most recently updated first, under "New",
//! "Import" and a field that searches them, with "Sort" on its right.
//! The same screen also shows the Trash.

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

use super::context_menu::{ContextMenu, Entry};
use super::dialog::{Button, Dialog};
use super::file_dialog::{FileDialog, FormCmd, Outcome as FormOutcome};
use super::hints::{HintBar, hint};
use super::menu::Outcome as MenuOutcome;
use super::widgets::{COLUMN_WIDTH, column, truncate};
use super::{Action, ImportInto};
use crate::vault::{DELETED_RETENTION_DAYS, PostSummary};

/// The longest search that can be typed.
const MAX_FILTER: usize = 80;

/// The search field's icon: an emoji, or with a Nerd Font, its magnifier
/// (nf-fa-search), which takes the terminal's own colour.
const SEARCH_ICON: &str = "🔎 ";
const NERD_SEARCH_ICON: &str = "\u{f002} ";

/// Before the way of sorting, on the right of the search field.
const SORT_PREFIX: &str = "Sort: ";

/// How the posts are ordered: by when they were updated or created, or by
/// title, either way round.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sort {
    by: SortBy,
    /// Newest first, or Z to A.
    descending: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SortBy {
    Updated,
    Created,
    Title,
}

impl Sort {
    /// As listed in the "Sort" menu: each way of sorting, its usual
    /// direction first.
    const ALL: [Sort; 6] = [
        Sort { by: SortBy::Updated, descending: true },
        Sort { by: SortBy::Updated, descending: false },
        Sort { by: SortBy::Created, descending: true },
        Sort { by: SortBy::Created, descending: false },
        Sort { by: SortBy::Title, descending: false },
        Sort { by: SortBy::Title, descending: true },
    ];

    /// Most recently updated first.
    pub const DEFAULT: Sort = Sort::ALL[0];

    /// E.g. "Updated ↓": the arrow points down for newest first, or Z to A.
    fn label(self) -> &'static str {
        match (self.by, self.descending) {
            (SortBy::Updated, true) => "Updated ↓",
            (SortBy::Updated, false) => "Updated ↑",
            (SortBy::Created, true) => "Created ↓",
            (SortBy::Created, false) => "Created ↑",
            (SortBy::Title, false) => "Title ↑",
            (SortBy::Title, true) => "Title ↓",
        }
    }

    /// As remembered in the vault, e.g. "updated-desc".
    pub fn name(self) -> String {
        let by = match self.by {
            SortBy::Updated => "updated",
            SortBy::Created => "created",
            SortBy::Title => "title",
        };
        format!("{by}-{}", if self.descending { "desc" } else { "asc" })
    }

    /// Before 0.13.0 the vault kept only the field ("updated", "created",
    /// "title"), sorted its usual way: the first of it in `ALL`.
    pub fn from_name(name: &str) -> Sort {
        Sort::ALL
            .into_iter()
            .find(|sort| sort.name() == name || sort.name().split('-').next() == Some(name))
            .unwrap_or(Sort::DEFAULT)
    }
}

pub enum Mode {
    /// The normal list, with a "Trash" row if anything is there.
    Posts {
        deleted_count: usize,
    },
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
    /// Open the "Sort" menu.
    SortMenu,
    /// Sort the next (or previous) way in the "Sort" menu.
    CycleSort(bool),
    /// The way highlighted in the "Sort" menu.
    PickSort,
    Sort(Sort),
    CloseSortMenu,
    Lock,
    /// A hint of the import dialog.
    Form(FormCmd),
    /// The file was imported before: update that post, or make a new one.
    UpdateImported,
    ImportAsNew,
    Delete,
    Quit,
    Back,
    ConfirmDelete,
    CancelDialog,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Row {
    New,
    Import,
    /// A blank line, above the search field.
    Gap,
    /// The search field, with "Sort" on its right.
    Search,
    Post(usize),
    Trash,
}

impl Row {
    /// Whether the highlight can be on it (↑/↓ pass over the others).
    fn selectable(self) -> bool {
        !matches!(self, Row::Gap)
    }
}

pub struct List {
    mode: Mode,
    posts: Vec<PostSummary>,
    rows: Vec<Row>,
    state: ListState,
    /// Typing goes into the search field. While something's typed, the
    /// highlight is on the first match (↑/↓ move it), so Enter opens it.
    typing: bool,
    /// What's typed in the search field.
    filter: String,
    sort: Sort,
    /// Where "Sort" was drawn, for the mouse.
    sort_area: Option<Rect>,
    /// The "Sort" menu, open.
    sort_menu: Option<ContextMenu<Cmd>>,
    /// A post was asked to be highlighted; otherwise it's the first listed.
    chosen: bool,
    /// The posts (indexes into `posts`) that match `filter`, once the
    /// vault has said; `None` while not searching.
    matches: Option<Vec<usize>>,
    dialog: Option<(usize, Dialog<Cmd>)>,
    /// `SEARCH_ICON`, or `NERD_SEARCH_ICON`.
    search_icon: &'static str,
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
    /// otherwise "New" (in the Trash, the first post).
    pub fn new(mode: Mode, posts: Vec<PostSummary>, select: Option<i64>) -> Self {
        let mut list = List {
            mode,
            posts,
            rows: Vec::new(),
            state: ListState::default(),
            typing: false,
            filter: String::new(),
            sort: Sort::DEFAULT,
            sort_area: None,
            sort_menu: None,
            chosen: false,
            matches: None,
            dialog: None,
            search_icon: SEARCH_ICON,
            importing: None,
            import_folder: String::new(),
            reimport: None,
            notice: None,
            rows_area: Rect::default(),
            hints: HintBar::default(),
        };
        list.rebuild_rows();
        let chosen = select.and_then(|id| list.posts.iter().position(|p| p.id == id));
        list.chosen = chosen.is_some();
        list.highlight(chosen);
        list
    }

    /// Highlight post `index`, or else "New", or else the first listed.
    fn highlight(&mut self, index: Option<usize>) {
        let row = match index {
            Some(i) => self.rows.iter().position(|r| *r == Row::Post(i)),
            None => self.rows.iter().position(|r| matches!(r, Row::New | Row::Post(_))),
        };
        self.state.select(Some(row.unwrap_or(0)));
    }

    /// Sorted this way (a post asked for stays highlighted).
    pub fn with_sort(mut self, sort: Sort) -> Self {
        self.sort = sort;
        self.rebuild_rows();
        if !self.chosen {
            self.highlight(None);
        }
        self
    }

    /// The posts (indexes into `posts`) in the order they're listed. The
    /// Trash is always most recently deleted first.
    fn ordered(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.posts.len()).collect();
        if self.trash_view() {
            return order;
        }
        match self.sort.by {
            // As the vault lists them.
            SortBy::Updated => {}
            SortBy::Created => order.sort_by_key(|&i| std::cmp::Reverse(self.posts[i].created_at)),
            SortBy::Title => {
                order.sort_by_cached_key(|&i| display_title(&self.posts[i].title).to_lowercase())
            }
        }
        // Dates sort newest first, titles A to Z; the other way round flips it.
        if self.sort.descending == (self.sort.by == SortBy::Title) {
            order.reverse();
        }
        order
    }

    /// The rows for the posts shown (all of them, or the search's matches),
    /// keeping the highlighted row if it's still there.
    fn rebuild_rows(&mut self) {
        let selected = self.selected_row();
        let mut rows = Vec::new();
        if matches!(self.mode, Mode::Posts { .. }) {
            rows.extend([Row::New, Row::Import, Row::Gap, Row::Search]);
        }
        let shown = |i: &usize| self.matches.as_ref().is_none_or(|m| m.contains(i));
        rows.extend(self.ordered().into_iter().filter(shown).map(Row::Post));
        if let Mode::Posts { deleted_count } = self.mode
            && !self.searching()
            && deleted_count > 0
        {
            rows.push(Row::Trash);
        }
        self.rows = rows;
        let index = selected
            .filter(|row| row.selectable())
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
        let matches =
            ids.iter().filter_map(|id| self.posts.iter().position(|p| p.id == *id)).collect();
        self.matches = Some(matches);
        self.rebuild_rows();
        if self.typing {
            self.highlight_first_match();
        }
    }

    /// While typing a search: the highlight on the first match (or the
    /// field, if nothing matches).
    fn highlight_first_match(&mut self) {
        let row = self.rows.iter().position(|r| matches!(r, Row::Post(_)));
        self.state.select(row.or_else(|| self.search_row()));
    }

    fn search_row(&self) -> Option<usize> {
        self.rows.iter().position(|r| *r == Row::Search)
    }

    /// Change the search; the app looks for the matches.
    fn set_filter(&mut self, filter: String) -> Action {
        let filter: String =
            filter.chars().filter(|ch| !ch.is_control()).take(MAX_FILTER).collect();
        if filter == self.filter {
            return Action::None;
        }
        self.filter = filter;
        if self.filter.trim().is_empty() {
            self.matches = None;
            self.rebuild_rows();
            if self.typing {
                self.state.select(self.search_row());
            }
            return Action::None;
        }
        Action::Search(self.filter.clone())
    }

    /// A one-line message shown above the hint bar, e.g. after deleting.
    pub fn with_notice(mut self, notice: String) -> Self {
        self.notice = Some(notice);
        self
    }

    /// The terminal's font is a Nerd Font: use its icons.
    pub fn with_nerd_font(mut self, nerd_font: bool) -> Self {
        self.search_icon = if nerd_font { NERD_SEARCH_ICON } else { SEARCH_ICON };
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
        self.typing
    }

    /// Open the "Sort" menu under where it's drawn, the current way
    /// highlighted.
    fn open_sort_menu(&mut self) {
        let Some(area) = self.sort_area else { return };
        let entries = Sort::ALL
            .iter()
            .map(|&sort| Entry {
                label: sort.label(),
                key: "",
                cmd: Cmd::Sort(sort),
                enabled: true,
            })
            .collect();
        let current = Sort::ALL.iter().position(|&sort| sort == self.sort).unwrap_or(0);
        let menu = ContextMenu::new(entries, Position::default()).with_selected(current);
        // Right-aligned with "Sort", just below it.
        let x = area.right().saturating_sub(menu.width());
        self.sort_menu = Some(menu.moved_to(Position::new(x, area.y + 1)));
    }

    /// Change how the posts are sorted; the app remembers it.
    fn set_sort(&mut self, sort: Sort) -> Action {
        if sort == self.sort {
            return Action::None;
        }
        self.sort = sort;
        self.rebuild_rows();
        if self.typing && self.searching() {
            self.highlight_first_match();
        }
        Action::SortBy(sort)
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
            let note =
                format!("Posts here are deleted forever after {DELETED_RETENTION_DAYS} days.");
            frame.render_widget(Paragraph::new(note.dim()), column(subheader, COLUMN_WIDTH));
        } else {
            frame.render_widget(Paragraph::new("writui".bold()), column(header, COLUMN_WIDTH));
        }

        let now = Timestamp::now();
        let tz = TimeZone::system();
        let width = col.width as usize;
        // The search field, then "Sort: Updated ↓" right-aligned; the
        // field is as wide whichever way it's sorted.
        let longest_sort = Sort::ALL.iter().map(|sort| sort.label().width()).max().unwrap_or(0);
        let field_width = width.saturating_sub(SORT_PREFIX.width() + longest_sort + 2);
        let sort_width = (SORT_PREFIX.width() + self.sort.label().width()).min(width);
        let in_search = self.in_search();
        let items: Vec<ListItem> = self
            .rows
            .iter()
            .map(|row| match *row {
                // With their shortcuts, which the hint bar leaves out.
                Row::New => ListItem::new(Line::from(vec!["+ New".bold(), " (n)".dim()])),
                Row::Import => ListItem::new(Line::from(vec!["» Import".bold(), " (i)".dim()])),
                Row::Gap => ListItem::new(Line::default()),
                Row::Search => {
                    let shown = scrolled(
                        &self.filter,
                        field_width.saturating_sub(self.search_icon.width()),
                    );
                    let text = if shown.is_empty() { "Type to filter (/)" } else { shown };
                    let pad = field_width.saturating_sub(self.search_icon.width() + text.width());
                    // Underlined, so it reads as a field; dim, all of it (so
                    // the underline's one colour), until typing goes into it.
                    let style = if in_search {
                        Style::new().underlined()
                    } else {
                        Style::new().underlined().dim()
                    };
                    let field =
                        [Span::raw(self.search_icon), Span::raw(text), Span::raw(" ".repeat(pad))]
                            .map(|span| span.style(style));
                    let gap = width.saturating_sub(field_width + sort_width);
                    let sort = [
                        Span::raw(" ".repeat(gap)),
                        Span::raw(SORT_PREFIX).dim(),
                        Span::raw(self.sort.label()).bold(),
                    ];
                    ListItem::new(Line::from(field.into_iter().chain(sort).collect::<Vec<_>>()))
                }
                Row::Trash => {
                    let Mode::Posts { deleted_count } = self.mode else { unreachable!() };
                    ListItem::new(Line::from(format!("Trash ({deleted_count})").dim()))
                }
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
        let on_field = self.selected_row() == Some(Row::Search);
        let highlight = if on_field { Style::new() } else { Style::new().reversed() };
        frame.render_stateful_widget(
            ListWidget::new(items).highlight_style(highlight),
            list_area,
            &mut self.state,
        );
        // Where rows ended up, now the list has scrolled.
        let row_rect = |index: usize| -> Option<Rect> {
            let y = list_area.y + index.checked_sub(self.state.offset())? as u16;
            (y < list_area.bottom()).then_some(Rect { y, height: 1, ..list_area })
        };
        self.sort_area = self.search_row().and_then(row_rect).map(|rect| {
            let width = sort_width as u16;
            Rect { x: rect.right().saturating_sub(width), width, ..rect }
        });
        if in_search
            && self.dialog.is_none()
            && self.importing.is_none()
            && self.sort_menu.is_none()
            && let Some(rect) = self.search_row().and_then(row_rect)
        {
            let icon = self.search_icon.width();
            let x = rect.x
                + (icon + scrolled(&self.filter, field_width.saturating_sub(icon)).width()) as u16;
            frame.set_cursor_position(Position::new(x.min(col.right().saturating_sub(1)), rect.y));
        }

        if let Some(text) = &self.notice {
            let text = truncate(text, COLUMN_WIDTH as usize);
            frame.render_widget(Paragraph::new(text.dim()), column(notice, COLUMN_WIDTH));
        }

        if let Some(form) = &mut self.importing {
            form.render(frame);
            if self.dialog.is_none() {
                let hints: Vec<_> = form
                    .hints()
                    .into_iter()
                    .map(|h| hint(h.key, h.label, Cmd::Form(h.cmd)))
                    .collect();
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
        if let Some(menu) = &mut self.sort_menu {
            menu.render(frame);
            let hints =
                [hint("Enter", "sort", Cmd::PickSort), hint("Esc", "close", Cmd::CloseSortMenu)];
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
            if has_post {
                hints.push(hint("Enter", "open", Cmd::Open));
            }
            if self.searching() {
                hints.push(hint("Esc", "clear", Cmd::ClearSearch));
            } else {
                hints.push(hint("Esc", "done", Cmd::Back));
            }
            hints.push(hint("Tab", "sort", Cmd::CycleSort(true)));
            hints
        } else {
            // n and i are shown on "New" and "Import".
            let mut hints =
                vec![hint("Enter", "open", Cmd::Open), hint("/", "filter", Cmd::Search)];
            if has_post {
                hints.push(hint("d", "delete", Cmd::Delete));
            }
            hints.push(hint("s", "settings", Cmd::Settings));
            if self.searching() {
                hints.push(hint("Esc", "clear search", Cmd::ClearSearch));
            } else {
                hints.push(hint("Esc", "quit", Cmd::Quit));
            }
            hints.push(hint("Ctrl+O", "lock", Cmd::Lock));
            // Last, so it's the first left out on a narrow screen: "Sort
            // by" can be clicked too.
            hints.push(hint("Tab", "sort", Cmd::CycleSort(true)));
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
        if let Some(menu) = &mut self.sort_menu {
            if let Event::Mouse(mouse) = event
                && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && let Some(cmd) = self.hints.hit(mouse.column, mouse.row)
            {
                return self.run(cmd);
            }
            return match menu.handle(event) {
                MenuOutcome::None => Action::None,
                MenuOutcome::Close => self.run(Cmd::CloseSortMenu),
                MenuOutcome::Run(cmd) => self.run(cmd),
            };
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
                    KeyCode::Char('d') if plain => self.run(Cmd::Delete),
                    KeyCode::Char('s') if plain => self.run(Cmd::Settings),
                    KeyCode::Char('/') if plain => self.run(Cmd::Search),
                    KeyCode::Delete => self.run(Cmd::Delete),
                    KeyCode::Tab => self.run(Cmd::CycleSort(true)),
                    KeyCode::BackTab => self.run(Cmd::CycleSort(false)),
                    KeyCode::Up => self.move_by(-1),
                    KeyCode::Down => self.move_by(1),
                    // Unshown; in the search field they're typed.
                    KeyCode::Char('k') if plain => self.move_over_field(-1),
                    KeyCode::Char('j') if plain => self.move_over_field(1),
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
                    if self.sort_area.is_some_and(|area| area.contains(pos)) {
                        return self.run(Cmd::SortMenu);
                    }
                    if !self.rows_area.contains(pos) {
                        return Action::None;
                    }
                    let index = self.state.offset() + (mouse.row - self.rows_area.y) as usize;
                    let Some(&row) = self.rows.get(index).filter(|row| row.selectable()) else {
                        return Action::None;
                    };
                    // Buttons act on the first click; posts open on the second.
                    // In the Trash, clicks only select (restore is Enter).
                    let act = match row {
                        Row::Post(_) => !self.trash_view() && self.selected_row() == Some(row),
                        Row::Search => false,
                        _ => true,
                    };
                    // A click in the field types there; anywhere else, not.
                    self.typing = row == Row::Search;
                    if self.typing && self.searching() {
                        self.highlight_first_match();
                        return Action::None;
                    }
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
                    KeyCode::Char(ch) if !ctrl => {
                        Some(self.set_filter(format!("{}{ch}", self.filter)))
                    }
                    KeyCode::Backspace => {
                        let mut filter = self.filter.clone();
                        filter.pop();
                        Some(self.set_filter(filter))
                    }
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

    /// Move the highlight, passing over rows it can't be on. While a
    /// search is typed, it moves among the matches and typing still goes
    /// into the field; the empty field is just passed through.
    fn move_by(&mut self, delta: isize) -> Action {
        let current = self.state.selected().unwrap_or(0);
        let stops: Vec<usize> = if self.typing && self.searching() {
            (0..self.rows.len()).filter(|&i| matches!(self.rows[i], Row::Post(_))).collect()
        } else {
            (0..self.rows.len()).filter(|&i| self.rows[i].selectable()).collect()
        };
        let Some(at) = stops.iter().position(|&i| i == current).or(if stops.is_empty() {
            None
        } else {
            Some(0)
        }) else {
            return Action::None;
        };
        let next =
            stops[(at as isize).saturating_add(delta).clamp(0, stops.len() as isize - 1) as usize];
        self.state.select(Some(next));
        if !(self.typing && self.searching()) {
            self.typing = self.rows[next] == Row::Search;
        }
        Action::None
    }

    /// j / k: like ↓ / ↑, but over the empty search field, so the next
    /// j / k isn't typed into it.
    fn move_over_field(&mut self, delta: isize) -> Action {
        self.move_by(delta);
        if self.selected_row() == Some(Row::Search) {
            self.move_by(delta);
        }
        Action::None
    }

    /// The post to highlight after post `index` disappears from the list:
    /// the one listed after it, or else before it.
    fn neighbour_of(&self, index: usize) -> Option<i64> {
        let listed: Vec<usize> = self
            .rows
            .iter()
            .filter_map(|r| if let Row::Post(i) = r { Some(*i) } else { None })
            .collect();
        let at = listed.iter().position(|&i| i == index)?;
        listed
            .get(at + 1)
            .or_else(|| at.checked_sub(1).and_then(|a| listed.get(a)))
            .map(|&i| self.posts[i].id)
    }

    fn run(&mut self, cmd: Cmd) -> Action {
        match cmd {
            Cmd::Quit => Action::Quit,
            Cmd::Lock => Action::Lock,
            Cmd::New if !self.trash_view() => Action::NewPost,
            Cmd::Settings if !self.trash_view() => Action::ShowSettings,
            Cmd::Search if !self.trash_view() => {
                self.typing = true;
                if self.searching() {
                    self.highlight_first_match();
                } else {
                    self.state.select(self.search_row());
                }
                Action::None
            }
            Cmd::ClearSearch => self.set_filter(String::new()),
            Cmd::SortMenu if !self.trash_view() => {
                self.open_sort_menu();
                Action::None
            }
            Cmd::CycleSort(forward) if !self.trash_view() => {
                let current = Sort::ALL.iter().position(|&sort| sort == self.sort).unwrap_or(0);
                let step = if forward { 1 } else { Sort::ALL.len() - 1 };
                self.set_sort(Sort::ALL[(current + step) % Sort::ALL.len()])
            }
            Cmd::PickSort => match self.sort_menu.as_ref().and_then(ContextMenu::selected) {
                Some(cmd) => self.run(cmd),
                None => Action::None,
            },
            Cmd::Sort(sort) => {
                self.sort_menu = None;
                self.set_sort(sort)
            }
            Cmd::CloseSortMenu => {
                self.sort_menu = None;
                Action::None
            }
            Cmd::New | Cmd::Settings | Cmd::Search | Cmd::SortMenu | Cmd::CycleSort(_) => {
                Action::None
            }
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
                        let into = if matches!(cmd, Cmd::UpdateImported) {
                            ImportInto::Post(id)
                        } else {
                            ImportInto::New
                        };
                        Action::Import { path, into }
                    }
                    None => Action::None,
                }
            }
            Cmd::Back if self.trash_view() => Action::BackToList(None),
            Cmd::Back => {
                // Out of the search field, to the first post.
                if self.typing {
                    self.typing = false;
                    if self.selected_row() == Some(Row::Search) {
                        let first = self.rows.iter().position(|r| matches!(r, Row::Post(_)));
                        self.state.select(first.or(self.search_row()));
                    }
                }
                Action::None
            }
            Cmd::Open => match (self.selected_row(), self.trash_view()) {
                (Some(Row::New), _) => Action::NewPost,
                (Some(Row::Import), _) => self.run(Cmd::Import),
                (Some(Row::Search | Row::Gap), _) => Action::None,
                (Some(Row::Trash), _) => Action::ShowTrash,
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
                    Button {
                        label: "Delete forever",
                        key: "y",
                        cmd: Cmd::ConfirmDelete,
                        danger: true,
                    },
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
                    Line::from(
                        format!("You can restore it for {DELETED_RETENTION_DAYS} days.").dim(),
                    ),
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
        (0..24)
            .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
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
        let mut list = List::new(Mode::Posts { deleted_count: 0 }, Vec::new(), None)
            .with_import_folder(folder.clone());
        let text = screen(&mut list);
        assert!(text.contains("+ New (n)") && text.contains("» Import (i)"), "{text}");
        assert!(!text.contains("i import"));
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
        let Action::Import { path, into: ImportInto::Check } =
            press(&mut list, KeyCode::Enter, KeyModifiers::NONE)
        else {
            panic!("not imported")
        };
        assert_eq!(path, file);
        // Imported before: asks.
        list.ask_reimport(file, 7, "Post");
        let text = screen(&mut list);
        assert!(text.contains("Imported before") && text.contains("Update it"));
        assert!(matches!(
            press(&mut list, KeyCode::Char('u'), KeyModifiers::NONE),
            Action::Import { into: ImportInto::Post(7), .. }
        ));
        // Esc closes the dialog; i opens it again.
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        assert!(list.importing.is_none());
        press(&mut list, KeyCode::Char('i'), KeyModifiers::NONE);
        assert!(list.importing.is_some());
    }

    fn post(id: i64, title: &str) -> PostSummary {
        PostSummary {
            id,
            title: title.into(),
            created_at: at(&format!("2026-01-0{id}T00:00:00Z")),
            updated_at: Timestamp::now(),
            deleted_at: None,
        }
    }

    #[test]
    fn single_keys_and_esc_quits() {
        let posts = vec![post(1, "One"), post(2, "Two")];
        let mut list = List::new(Mode::Posts { deleted_count: 0 }, posts, None);
        let text = screen(&mut list);
        assert!(text.contains("/ filter") && text.contains("Esc quit"), "{text}");
        assert!(!text.contains("Ctrl+Q") && !text.contains("n new"));
        // "New" is highlighted at first; j / k move like ↓ / ↑.
        assert_eq!(list.selected_row(), Some(Row::New));
        // They pass over the search field.
        press(&mut list, KeyCode::Char('j'), KeyModifiers::NONE);
        press(&mut list, KeyCode::Char('j'), KeyModifiers::NONE);
        assert!(!list.in_search());
        press(&mut list, KeyCode::Char('j'), KeyModifiers::NONE);
        press(&mut list, KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(list.selected_post(), Some(0));
        assert!(matches!(
            press(&mut list, KeyCode::Char('n'), KeyModifiers::NONE),
            Action::NewPost
        ));
        assert!(matches!(
            press(&mut list, KeyCode::Char('s'), KeyModifiers::NONE),
            Action::ShowSettings
        ));
        press(&mut list, KeyCode::Char('d'), KeyModifiers::NONE);
        assert!(screen(&mut list).contains("Move “One” to the Trash?"));
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        assert!(list.dialog.is_none());
        // Esc quits straight away: the writing is already saved.
        assert!(matches!(press(&mut list, KeyCode::Esc, KeyModifiers::NONE), Action::Quit));
    }

    #[test]
    fn typing_in_the_search_field_searches() {
        let posts = vec![post(1, "One"), post(2, "Two"), post(3, "Three")];
        let mut list = List::new(Mode::Posts { deleted_count: 2 }, posts, None);
        let text = screen(&mut list);
        assert!(
            text.contains("🔎")
                && text.contains("Type to filter (/)")
                && text.contains("Sort: Updated ↓"),
            "{text}"
        );
        assert!(!text.contains("Settings\n") && !text.contains("  Settings  "), "{text}");
        // / goes into the field; letters are typed there, not shortcuts.
        press(&mut list, KeyCode::Char('/'), KeyModifiers::NONE);
        assert!(
            matches!(press(&mut list, KeyCode::Char('j'), KeyModifiers::NONE), Action::Search(f) if f == "j")
        );
        press(&mut list, KeyCode::Backspace, KeyModifiers::NONE);
        assert!(
            matches!(press(&mut list, KeyCode::Char('n'), KeyModifiers::NONE), Action::Search(f) if f == "n")
        );
        let Action::Search(filter) = press(&mut list, KeyCode::Char('e'), KeyModifiers::NONE)
        else {
            panic!()
        };
        assert_eq!(filter, "ne");
        // When the vault answers: only the matches, the first highlighted,
        // and typing still goes into the field.
        list.show_matches("ne", &[3, 1]);
        let text = screen(&mut list);
        assert!(text.contains(" ne ") && text.contains("Three") && text.contains("One"), "{text}");
        assert!(!text.contains("Two") && !text.contains("Trash"), "{text}");
        assert!(text.contains("Enter open") && !text.contains("open the first"));
        assert_eq!(list.selected_post(), Some(0)); // "One": listed most recently updated first
        // ↓ moves among the matches, still typing.
        press(&mut list, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(list.selected_post(), Some(2));
        assert!(list.in_search());
        // A stale answer is ignored.
        list.show_matches("n", &[1, 2, 3]);
        assert!(!screen(&mut list).contains("Two"));
        assert!(matches!(
            press(&mut list, KeyCode::Enter, KeyModifiers::NONE),
            Action::OpenPost(3)
        ));
        // Typing more highlights the first match again.
        press(&mut list, KeyCode::Char('x'), KeyModifiers::NONE);
        list.show_matches("nex", &[]);
        assert!(screen(&mut list).contains("No posts match."));
        assert!(matches!(press(&mut list, KeyCode::Enter, KeyModifiers::NONE), Action::None));
        // Esc clears it, then leaves the field; everything's back.
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        assert!(list.filter.is_empty() && list.in_search());
        assert!(screen(&mut list).contains("Two") && screen(&mut list).contains("Trash (2)"));
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!list.in_search());
        assert_eq!(list.selected_post(), Some(0));
        assert!(matches!(
            press(&mut list, KeyCode::Char('n'), KeyModifiers::NONE),
            Action::NewPost
        ));
        // ↑/↓ pass through the empty field.
        press(&mut list, KeyCode::Up, KeyModifiers::NONE);
        assert!(list.in_search());
        press(&mut list, KeyCode::Up, KeyModifiers::NONE);
        assert!(!list.in_search() && list.selected_row() == Some(Row::Import));
        press(&mut list, KeyCode::Down, KeyModifiers::NONE);
        press(&mut list, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(list.selected_post(), Some(0));
    }

    fn click(list: &mut List, column: u16, row: u16) -> Action {
        let kind = MouseEventKind::Down(MouseButton::Left);
        list.handle(Event::Mouse(ratatui::crossterm::event::MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }))
    }

    #[test]
    fn posts_sort_either_way_from_the_sort_by_menu() {
        // As the vault lists them: most recently updated first.
        let mut posts = vec![post(2, "beta"), post(3, "Alpha"), post(1, "")];
        for (i, post) in posts.iter_mut().enumerate() {
            post.updated_at = at(&format!("2026-02-0{}T00:00:00Z", 3 - i));
        }
        let titles = |list: &mut List| -> Vec<String> {
            list.rows
                .iter()
                .filter_map(|r| {
                    if let Row::Post(i) = r {
                        Some(display_title(&list.posts[*i].title).to_string())
                    } else {
                        None
                    }
                })
                .collect()
        };
        let by = |by, descending| Sort { by, descending };
        let mut list = List::new(Mode::Posts { deleted_count: 0 }, posts.clone(), None);
        assert_eq!(titles(&mut list), ["beta", "Alpha", "Untitled"]);
        // "Sort" is on the right of the search field, on the same row.
        let text = screen(&mut list);
        let row = text.lines().find(|line| line.contains("Type to filter (/)")).unwrap();
        assert!(row.trim_end().ends_with("Sort: Updated ↓"), "{row}");
        // Tab sorts the next way, Shift+Tab the one before, round and
        // round; the app remembers it.
        assert!(
            matches!(press(&mut list, KeyCode::Tab, KeyModifiers::NONE), Action::SortBy(s) if s == by(SortBy::Updated, false))
        );
        assert!(list.sort_menu.is_none());
        assert_eq!(titles(&mut list), ["Untitled", "Alpha", "beta"]);
        assert!(
            matches!(press(&mut list, KeyCode::BackTab, KeyModifiers::SHIFT), Action::SortBy(s) if s == Sort::DEFAULT)
        );
        assert!(
            matches!(press(&mut list, KeyCode::BackTab, KeyModifiers::SHIFT), Action::SortBy(s) if s == by(SortBy::Title, true))
        );
        assert!(
            matches!(press(&mut list, KeyCode::Tab, KeyModifiers::NONE), Action::SortBy(s) if s == Sort::DEFAULT)
        );
        // In the search field too.
        press(&mut list, KeyCode::Char('/'), KeyModifiers::NONE);
        assert!(
            matches!(press(&mut list, KeyCode::Tab, KeyModifiers::NONE), Action::SortBy(s) if s == by(SortBy::Updated, false))
        );
        assert!(list.filter.is_empty());
        press(&mut list, KeyCode::BackTab, KeyModifiers::SHIFT);
        press(&mut list, KeyCode::Esc, KeyModifiers::NONE);
        // Clicking "Sort" opens the menu on the current way; ↓ Enter
        // picks the next.
        screen(&mut list);
        let area = list.sort_area.unwrap();
        assert!(matches!(click(&mut list, area.x, area.y), Action::None));
        let text = screen(&mut list);
        assert!(
            text.contains("Updated ↑")
                && text.contains("Title ↓")
                && !text.contains("oldest first"),
            "{text}"
        );
        assert!(text.contains("Enter sort") && text.contains("Esc close"), "{text}");
        press(&mut list, KeyCode::Down, KeyModifiers::NONE);
        assert!(
            matches!(press(&mut list, KeyCode::Enter, KeyModifiers::NONE), Action::SortBy(s) if s == by(SortBy::Updated, false))
        );
        assert!(list.sort_menu.is_none());
        assert_eq!(titles(&mut list), ["Untitled", "Alpha", "beta"]);
        // Esc just closes it.
        screen(&mut list);
        click(&mut list, area.x, area.y);
        assert!(matches!(press(&mut list, KeyCode::Esc, KeyModifiers::NONE), Action::None));
        assert!(list.sort_menu.is_none() && list.dialog.is_none());
        // Or click "Sort", then a way of sorting.
        screen(&mut list);
        let area = list.sort_area.unwrap();
        assert!(matches!(click(&mut list, area.x, area.y), Action::None));
        let text = screen(&mut list);
        let line = text.lines().position(|line| line.contains("Title ↓")).unwrap() as u16;
        assert!(
            matches!(click(&mut list, area.right() - 2, line), Action::SortBy(s) if s == by(SortBy::Title, true))
        );
        assert_eq!(titles(&mut list), ["Untitled", "beta", "Alpha"]);
        assert!(screen(&mut list).contains("Sort: Title ↓"));
        list.set_sort(by(SortBy::Title, false));
        assert_eq!(titles(&mut list), ["Alpha", "beta", "Untitled"]);
        list.set_sort(by(SortBy::Created, true));
        assert_eq!(titles(&mut list), ["Alpha", "beta", "Untitled"]);
        list.set_sort(by(SortBy::Created, false));
        assert_eq!(titles(&mut list), ["Untitled", "beta", "Alpha"]);
        // Opening sorted: "New" is highlighted, or the post asked for.
        let title = by(SortBy::Title, false);
        let list =
            List::new(Mode::Posts { deleted_count: 0 }, posts.clone(), None).with_sort(title);
        assert_eq!(list.selected_row(), Some(Row::New));
        let list = List::new(Mode::Posts { deleted_count: 0 }, posts, Some(1)).with_sort(title);
        assert_eq!(list.selected_post(), Some(2));
        // Remembered by name; what earlier versions kept still reads.
        for sort in Sort::ALL {
            assert_eq!(Sort::from_name(&sort.name()), sort);
        }
        assert_eq!(Sort::from_name("title"), title);
        assert_eq!(Sort::from_name("created"), by(SortBy::Created, true));
        assert_eq!(Sort::from_name("nonsense"), Sort::DEFAULT);
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
