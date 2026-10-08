//! The post editing screen: the text in a centred column, soft-wrapped, with
//! the cursor, scrolling, the mouse, undo, and versions.

mod autosave;
mod browser;
mod buffer;
mod clipboard;
mod history;
mod markdown;
mod undo;
mod wrap;

use std::ops::Range;
use std::time::{Duration, Instant};

use ratatui::Frame;
use jiff::tz::TimeZone;
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ropey::Rope;

use super::Action;
use super::dialog::{Button, Dialog};
use super::file_dialog::{FileDialog, FormCmd, Outcome as FormOutcome};
use super::hints::{HintBar, hint};
use super::list::display_title;
use super::menu::{self, Menu, item};
use super::widgets::{COLUMN_WIDTH, truncate};
use crate::files::{self, front_matter};
use crate::vault::{ExportSettings, Post, Version, TITLE_PREFIX, title_from_first_line};
use autosave::Autosave;
use buffer::Buffer;
use clipboard::Clipboard;
use history::{History, Outcome};
use markdown::Markup;
use undo::{Kind, Undo};
use wrap::{Row, TAB_WIDTH, layout, pos_at_x, row_of, x_of};

/// What the Tab key types.
const TAB: &str = "  ";

/// Clicks this close together (in time, and in place) make a double or
/// triple click.
const MULTI_CLICK: Duration = Duration::from_millis(500);

/// The longest name a version can have.
const MAX_VERSION_NAME: usize = 80;

/// Rows kept between the cursor and the top or bottom edge when scrolling to
/// follow it. The text can also scroll this far past its last row.
const SCROLL_MARGIN: usize = 3;

/// Blank rows above the title, at the top of the post. They scroll away
/// with the text, so further down the text uses the whole screen.
const PAD_TOP: usize = 1;

#[derive(Clone, Copy)]
enum Cmd {
    Back,
    SaveVersion,
    ConfirmSaveVersion,
    CancelDialog,
    History,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    Deselect,
    OpenLink,
    SelectAll,
    Menu,
    CloseMenu,
    RunMenu,
    CopyMarkdown,
    Export,
    /// A hint of the export dialog.
    Form(FormCmd),
    ConfirmReplace,
    Quit,
}

/// How posts were last exported (or imported), to start the next export
/// from.
#[derive(Default)]
pub struct Exports {
    pub this_post: Option<ExportSettings>,
    pub latest: Option<ExportSettings>,
}

struct Exporting {
    form: FileDialog,
    /// The front matter writui suggested, remembered with the export.
    suggested: String,
}

#[derive(Clone, Copy)]
struct Click {
    at: Instant,
    column: u16,
    row: u16,
    /// 1 for a click, 2 for a double click, 3 for a triple click.
    count: u8,
}

pub struct Editor {
    post_id: i64,
    /// When the post was created, as shown in the hint bar.
    created: String,
    /// Words in the post, counted when the hint bar first needs them after
    /// an edit.
    words: Option<usize>,
    /// The text as last stored in the vault.
    stored: String,
    /// The post's most recent version, with its text.
    last_version: Option<Version>,
    buffer: Buffer,
    undo: Undo,
    autosave: Autosave,
    /// The rows on screen for the current text and width.
    rows: Vec<Row>,
    /// How the text is formatted, and its links.
    markup: Markup,
    /// `rows` and `markup` need rebuilding (the text or the width changed).
    stale: bool,
    /// Wrap width in cells, from the last render.
    width: usize,
    /// Where the text was drawn on the last render. Rows start at its left
    /// edge; it's one cell wider than `width`, for the cursor (or a hanging
    /// space) at the end of a full row.
    text_area: Rect,
    /// The first row on screen.
    top: usize,
    /// Scroll to the cursor on the next render.
    follow: bool,
    /// Not rendered yet: the first render puts the cursor mid-screen.
    opening: bool,
    /// The column kept while moving up and down past shorter rows.
    goal: Option<usize>,
    clipboard: Clipboard,
    /// The last left click, to tell double and triple clicks.
    last_click: Option<Click>,
    /// The left button is held down after a single click: moving selects.
    dragging: bool,
    error: Option<String>,
    /// A passing message (e.g. "Copied."), until the next key or click.
    notice: Option<String>,
    /// Naming a version, or asking before an export replaces a file.
    dialog: Option<Dialog<Cmd>>,
    /// The export dialog, open.
    exporting: Option<Exporting>,
    /// Exporting would replace this file: asking first.
    replacing: Option<std::path::PathBuf>,
    /// How this post, and the latest of any, were last exported.
    exports: Exports,
    /// The command menu (Ctrl+K).
    menu: Option<Menu<Cmd>>,
    /// The post's history, shown instead of the post while open.
    history: Option<History>,
    hints: HintBar<Cmd>,
    /// Show the hint bar; otherwise only the writing (and passing messages)
    /// are on screen.
    pub show_chrome: bool,
    /// The hint bar shows only while Ctrl is held: the text
    /// takes the whole screen, and they're drawn over its bottom rows.
    pub zen: bool,
}

impl Editor {
    /// Opens with the cursor where it was left (or at the end of the post).
    pub fn new(post: Post, last_version: Option<Version>, exports: Exports) -> Self {
        let created = post.created_at.to_zoned(TimeZone::system()).strftime("%b %-d, %Y, %-I:%M %p").to_string();
        Editor {
            last_version,
            post_id: post.id,
            created,
            words: None,
            buffer: Buffer::new(&post.body, post.cursor),
            stored: post.body,
            undo: Undo::default(),
            autosave: Autosave::default(),
            rows: Vec::new(),
            markup: Markup::default(),
            stale: true,
            width: COLUMN_WIDTH as usize,
            text_area: Rect::default(),
            top: 0,
            follow: true,
            opening: true,
            goal: None,
            clipboard: Clipboard::default(),
            last_click: None,
            dragging: false,
            error: None,
            notice: None,
            dialog: None,
            exporting: None,
            replacing: None,
            exports,
            menu: None,
            history: None,
            hints: HintBar::default(),
            show_chrome: true,
            zen: false,
        }
    }

    pub fn post_id(&self) -> i64 {
        self.post_id
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    /// The cursor, as a char index into `text()`.
    pub fn cursor(&self) -> usize {
        self.buffer.cursor()
    }

    /// The text differs from what's stored in the vault.
    pub fn changed(&self) -> bool {
        self.buffer.text() != self.stored
    }

    /// The text differs from the most recent version (or there are none).
    fn changed_since_version(&self) -> bool {
        self.last_version.as_ref().is_none_or(|v| *self.buffer.rope() != v.body.as_str())
    }

    /// A version of the post was just saved.
    pub fn version_saved(&mut self, version: Version) {
        self.notice = Some(match version.name.as_str() {
            "" => "Saved a version.".to_string(),
            name => format!("Saved version “{name}”."),
        });
        self.last_version = Some(version);
    }

    /// A passing message, e.g. where the post was imported from.
    pub fn set_notice(&mut self, notice: String) {
        self.notice = Some(notice);
    }

    /// Something went wrong outside of storing the post, e.g. saving a version.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// The post's title, for the terminal window ("Untitled" if it has none).
    pub fn title(&self) -> String {
        let title = self.raw_title();
        // Control characters could make the terminal do things.
        display_title(&title).chars().filter(|c| !c.is_control()).collect()
    }

    /// Show the post's history (newest version first).
    pub fn show_history(&mut self, versions: Vec<Version>) {
        self.history = Some(History::new(self.raw_title(), versions, self.changed_since_version()));
    }

    /// Open a version (with its text) in the history, to read.
    pub fn show_version(&mut self, version: Version) {
        if let Some(history) = &mut self.history {
            history.read(version);
        }
    }

    /// When the post should next be stored, if it has unstored edits.
    pub fn autosave_due(&self) -> Option<Instant> {
        self.autosave.due()
    }

    /// The post in the vault is now `text` (or already was, if `None`).
    pub fn mark_stored(&mut self, text: Option<String>) {
        if let Some(text) = text {
            self.stored = text;
        }
        self.autosave.stored();
        self.error = None;
    }

    /// Storing the post failed; say so above the hint bar, and try again
    /// a little later.
    pub fn store_failed(&mut self, error: String) {
        self.autosave.failed(Instant::now());
        self.error = Some(error);
    }

    pub fn render(&mut self, frame: &mut Frame) {
        if let Some(history) = &mut self.history {
            history.render(frame);
            return;
        }
        let [above, status, hint_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1)])
                .areas(frame.area());
        let body = if self.zen { frame.area() } else { above };

        let width = (COLUMN_WIDTH as usize).min(body.width.saturating_sub(1) as usize).max(1);
        if width != self.width {
            self.width = width;
            self.stale = true;
            self.follow = true;
        }
        self.refresh();
        let left = body.x + body.width.saturating_sub(width as u16) / 2;
        self.text_area = Rect::new(left, body.y, (width as u16 + 1).min(body.right() - left), body.height);

        let height = body.height as usize;
        let (cursor_row, cursor_x) = self.cursor_cell();
        if self.opening {
            // As close to the middle as the text allows (at the end of the
            // post, that's the usual margin above the bottom edge).
            self.opening = false;
            self.follow = false;
            self.top = cursor_row.saturating_sub(height / 2).min(self.max_top());
        }
        if self.follow {
            self.follow = false;
            let margin = margin(height);
            if cursor_row < self.top + margin {
                self.top = cursor_row.saturating_sub(margin);
            } else if cursor_row + margin + 1 > self.top + height {
                self.top = cursor_row + margin + 1 - height;
            }
        }

        let rope = self.buffer.rope();
        let selection = self.buffer.selection();
        let lines: Vec<Line> = (self.top..self.top + height)
            .map(|i| match i.checked_sub(PAD_TOP).and_then(|i| self.rows.get(i)) {
                Some(row) => row_line(rope, &self.markup, row, selection.as_ref()),
                None => Line::default(),
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), self.text_area);

        // In zen, the status and hint rows are drawn over the text, only
        // while there's something to show there.
        let chrome = self.show_chrome || self.dialog.is_some() || self.exporting.is_some() || self.menu.is_some();
        let message = self.error.is_some() || self.notice.is_some();
        let status_covered = self.zen && message;
        let hints_covered = self.zen && chrome;
        for (area, cover) in [(status, status_covered), (hint_area, hints_covered)] {
            if cover {
                frame.render_widget(Clear, area);
            }
        }
        if (self.top..self.top + height).contains(&cursor_row) {
            let y = body.y + (cursor_row - self.top) as u16;
            let hidden = (status_covered && y == status.y) || (hints_covered && y == hint_area.y);
            if !hidden {
                frame.set_cursor_position(Position::new(left + cursor_x as u16, y));
            }
        }

        // The status row: a passing message or an error.
        let area = Rect { x: left, width: width as u16, ..status };
        let message = match (&self.error, &self.notice) {
            (Some(error), _) => Some(truncate(error, width).red()),
            (None, Some(notice)) => Some(truncate(notice, width).dim()),
            (None, None) => None,
        };
        if let Some(message) = message {
            frame.render_widget(Paragraph::new(message), area);
        }

        // The hint bar, as wide as the text.
        let bar = Rect { x: left, width: width as u16, ..hint_area };
        if let Some(exporting) = &mut self.exporting {
            exporting.form.render(frame);
            if self.dialog.is_none() {
                let hints: Vec<_> =
                    exporting.form.hints().into_iter().map(|h| hint(h.key, h.label, Cmd::Form(h.cmd))).collect();
                self.hints.render(frame, bar, &hints);
                return;
            }
        }
        if let Some(dialog) = &mut self.dialog {
            dialog.render(frame);
            let hints = dialog.hints();
            self.hints.render(frame, bar, &hints);
            return;
        }
        if let Some(menu) = &mut self.menu {
            menu.render(frame);
            let mut hints = Vec::new();
            if menu.selected().is_some() {
                hints.push(hint("Enter", "run", Cmd::RunMenu));
            }
            hints.push(hint("Esc", "close", Cmd::CloseMenu));
            self.hints.render(frame, bar, &hints);
            return;
        }
        if !chrome {
            self.hints.hide();
            return;
        }
        // The menu on the left; on the right, how long the post is and when
        // it was started, as much of that as fits.
        let used = self.hints.render_left(frame, bar, &[hint("Ctrl+K", "menu", Cmd::Menu)]);
        let words = *self.words.get_or_insert_with(|| count_words(self.buffer.rope()));
        let words = match words {
            1 => "1 word".to_string(),
            n => format!("{} words", thousands(n)),
        };
        let room = (width as u16).saturating_sub(used + 3) as usize;
        let full = format!("{words} · {}", self.created);
        let stats = [full, words].into_iter().find(|text| text.chars().count() <= room);
        if let Some(stats) = stats {
            frame.render_widget(Paragraph::new(stats.dim()).right_aligned(), bar);
        }
    }

    /// The commands in the menu, in order.
    fn menu_items() -> Vec<menu::Item<Cmd>> {
        vec![
            item("Save version", "Ctrl+S", Cmd::SaveVersion),
            item("History", "Ctrl+R", Cmd::History),
            item("Copy as markdown", "Ctrl+Shift+C", Cmd::CopyMarkdown),
            item("Export as markdown", "Ctrl+Shift+S", Cmd::Export),
            item("Open link", "Ctrl+O", Cmd::OpenLink),
            item("Undo", "Ctrl+Z", Cmd::Undo),
            item("Redo", "Ctrl+Y", Cmd::Redo),
            item("Copy", "Ctrl+C", Cmd::Copy),
            item("Cut", "Ctrl+X", Cmd::Cut),
            item("Paste", "Ctrl+V", Cmd::Paste),
            item("Select all", "Ctrl+A", Cmd::SelectAll),
            item("Back to posts", "Esc", Cmd::Back),
            item("Quit", "Ctrl+Q", Cmd::Quit),
        ]
    }

    pub fn handle(&mut self, event: Event) -> Action {
        if let Some(history) = &mut self.history {
            return match history.handle(event) {
                Outcome::None => Action::None,
                Outcome::Close => {
                    self.history = None;
                    Action::None
                }
                Outcome::Load(id) => Action::LoadVersion(id),
                Outcome::Restore(version) => {
                    self.history = None;
                    self.restore(version);
                    Action::None
                }
                Outcome::Quit => Action::Quit,
            };
        }
        if let Some(menu) = &mut self.menu {
            if let Event::Mouse(mouse) = event
                && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && let Some(cmd) = self.hints.hit(mouse.column, mouse.row)
            {
                return self.run(cmd);
            }
            return match menu.handle(event) {
                menu::Outcome::None => Action::None,
                menu::Outcome::Close => self.run(Cmd::CloseMenu),
                menu::Outcome::Run(cmd) => {
                    self.menu = None;
                    self.run(cmd)
                }
            };
        }
        if let Some(dialog) = &mut self.dialog {
            let mut cmd = dialog.handle(event.clone());
            if cmd.is_none()
                && let Event::Mouse(mouse) = event
                && mouse.kind == MouseEventKind::Down(MouseButton::Left)
            {
                cmd = self.hints.hit(mouse.column, mouse.row);
            }
            return cmd.map_or(Action::None, |cmd| self.run(cmd));
        }
        if let Some(exporting) = &mut self.exporting {
            if let Event::Mouse(mouse) = event
                && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && let Some(cmd) = self.hints.hit(mouse.column, mouse.row)
            {
                return self.run(cmd);
            }
            let outcome = exporting.form.handle(event);
            return self.form_outcome(outcome);
        }
        if let Event::Key(_) | Event::Mouse(MouseEvent { kind: MouseEventKind::Down(_), .. }) = event {
            self.notice = None;
        }
        match event {
            Event::Key(key) => return self.key(key),
            Event::Mouse(mouse) => return self.mouse(mouse),
            Event::Paste(text) => self.edit(Kind::Other, |b| b.paste(&text)),
            _ => {}
        }
        Action::None
    }

    fn key(&mut self, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        // Cmd, on macOS (elsewhere the key is the system's).
        let cmd = cfg!(target_os = "macos") && key.modifiers.contains(KeyModifiers::SUPER);
        // Elsewhere the Super (Windows) key is the system's: keys with it do
        // nothing here.
        if key.modifiers.contains(KeyModifiers::SUPER) && !cmd {
            return Action::None;
        }
        let moving = match key.code {
            KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown => true,
            // Option+Left / Option+Right, in terminals that send them so.
            KeyCode::Char('b' | 'f') => alt,
            _ => false,
        };
        if moving {
            let selection = self.buffer.selection();
            if shift {
                self.buffer.begin_select();
            } else if let (Some(range), KeyCode::Left | KeyCode::Right) = (selection, key.code)
                && !ctrl
                && !alt
                && !cmd
            {
                // Left and Right just drop the selection, at its near end.
                self.step(|b| b.set_cursor(if key.code == KeyCode::Left { range.start } else { range.end }));
                self.buffer.clear_selection();
                return Action::None;
            } else {
                self.buffer.clear_selection();
            }
        }
        match key.code {
            KeyCode::Esc if self.buffer.selection().is_some() => return self.run(Cmd::Deselect),
            KeyCode::Esc => return self.run(Cmd::Back),
            // Ctrl+Shift+Z also redoes, in terminals that tell it apart
            // from Ctrl+Z.
            KeyCode::Char('z' | 'Z') if ctrl && shift => return self.run(Cmd::Redo),
            // (The kitty keyboard protocol reports it as Ctrl+"Z".)
            KeyCode::Char('Z') if ctrl => return self.run(Cmd::Redo),
            KeyCode::Char('z') if ctrl => return self.run(Cmd::Undo),
            KeyCode::Char('y') if ctrl => return self.run(Cmd::Redo),
            // Ctrl+Shift+C and Ctrl+Shift+S, in terminals that tell them
            // apart (the kitty keyboard protocol reports them as Ctrl+"C"
            // and Ctrl+"S").
            KeyCode::Char('C') if ctrl => return self.run(Cmd::CopyMarkdown),
            KeyCode::Char('c') if ctrl && shift => return self.run(Cmd::CopyMarkdown),
            KeyCode::Char('S') if ctrl => return self.run(Cmd::Export),
            KeyCode::Char('s') if ctrl && shift => return self.run(Cmd::Export),
            KeyCode::Char('s') if ctrl => return self.run(Cmd::SaveVersion),
            KeyCode::Char('r') if ctrl => return self.run(Cmd::History),
            KeyCode::Char('c') if ctrl => return self.run(Cmd::Copy),
            KeyCode::Char('x') if ctrl => return self.run(Cmd::Cut),
            KeyCode::Char('v') if ctrl => return self.run(Cmd::Paste),
            KeyCode::Char('o') if ctrl => return self.run(Cmd::OpenLink),
            KeyCode::Char('k') if ctrl => return self.run(Cmd::Menu),
            KeyCode::Char('a') if ctrl => return self.run(Cmd::SelectAll),
            KeyCode::Char(ch) if !ctrl && !alt => {
                let kind = Kind::Type { space: ch.is_whitespace() };
                self.edit(kind, |b| b.insert(ch.encode_utf8(&mut [0; 4])))
            }
            KeyCode::Enter => self.edit(Kind::Other, |b| b.insert("\n")),
            KeyCode::Tab => self.edit(Kind::Other, |b| b.insert(TAB)),
            KeyCode::Backspace => self.edit(Kind::Backspace, Buffer::backspace),
            KeyCode::Delete => self.edit(Kind::Delete, Buffer::delete),
            // Like Home and End.
            KeyCode::Left if ctrl || cmd => self.row_start(),
            KeyCode::Right if ctrl || cmd => self.row_end(),
            KeyCode::Left if alt => self.step(Buffer::word_left),
            KeyCode::Right if alt => self.step(Buffer::word_right),
            KeyCode::Char('b') => self.step(Buffer::word_left),
            KeyCode::Char('f') => self.step(Buffer::word_right),
            KeyCode::Up if ctrl || alt => self.step(Buffer::paragraph_left),
            KeyCode::Down if ctrl || alt => self.step(Buffer::paragraph_right),
            KeyCode::Left => self.step(Buffer::left),
            KeyCode::Right => self.step(Buffer::right),
            KeyCode::Up => self.move_rows(-1),
            KeyCode::Down => self.move_rows(1),
            KeyCode::PageUp => self.page(-1),
            KeyCode::PageDown => self.page(1),
            KeyCode::Home if ctrl => self.step(|b| b.set_cursor(0)),
            KeyCode::End if ctrl => self.step(|b| b.set_cursor(b.len())),
            KeyCode::Home => self.row_start(),
            KeyCode::End => self.row_end(),
            _ => {}
        }
        if moving && shift {
            self.buffer.settle();
        }
        Action::None
    }

    fn mouse(&mut self, mouse: MouseEvent) -> Action {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.top = self.top.saturating_sub(1),
            MouseEventKind::ScrollDown => {
                self.refresh();
                if self.top < self.max_top() {
                    self.top += 1;
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(cmd) = self.hints.hit(mouse.column, mouse.row) {
                    return self.run(cmd);
                }
                // Ctrl+click on a link opens it.
                if mouse.modifiers.contains(KeyModifiers::CONTROL) && self.in_text(mouse.row) {
                    let pos = self.pos_at(mouse.column, mouse.row);
                    if let Some(link) = self.markup.link_at(pos) {
                        let url = link.url.clone();
                        self.open(&url);
                        return Action::None;
                    }
                }
                self.click(mouse.column, mouse.row);
            }
            MouseEventKind::Drag(MouseButton::Left) if self.dragging => self.drag(mouse.column, mouse.row),
            MouseEventKind::Up(MouseButton::Left) => self.dragging = false,
            _ => {}
        }
        Action::None
    }

    fn run(&mut self, cmd: Cmd) -> Action {
        match cmd {
            Cmd::Back => return Action::BackToList(Some(self.post_id)),
            Cmd::Undo => {
                if let Some(prev) = self.undo.undo(self.buffer.clone()) {
                    self.replace(prev);
                }
            }
            Cmd::Redo => {
                if let Some(next) = self.undo.redo(self.buffer.clone()) {
                    self.replace(next);
                }
            }
            Cmd::SaveVersion if !self.changed_since_version() => {
                self.notice = Some("Nothing has changed since the last version.".into());
            }
            Cmd::SaveVersion => {
                let dialog = Dialog::new(
                    "Save version",
                    vec![
                        Line::from("Your post is saved automatically as you type."),
                        Line::from("A version is a snapshot of it you can read later,".dim()),
                        Line::from("and go back to. Name it like a commit message.".dim()),
                    ],
                    vec![
                        Button { label: "Save", key: "", cmd: Cmd::ConfirmSaveVersion, danger: false },
                        Button { label: "Cancel", key: "", cmd: Cmd::CancelDialog, danger: false },
                    ],
                    0,
                    1,
                );
                self.dialog = Some(dialog.with_input("e.g. Ready for feedback", MAX_VERSION_NAME));
            }
            Cmd::ConfirmSaveVersion => {
                if let Some(dialog) = self.dialog.take() {
                    return Action::SaveVersion { name: dialog.input().trim().to_string() };
                }
            }
            Cmd::CancelDialog => {
                self.dialog = None;
                self.replacing = None;
            }
            Cmd::History if self.last_version.is_none() => {
                self.notice = Some("No versions yet. Ctrl+S saves one.".into());
            }
            Cmd::History => return Action::ShowHistory,
            Cmd::Copy => match self.buffer.selected_text() {
                Some(text) => {
                    self.clipboard.copy(&text);
                    self.notice = Some("Copied.".into());
                }
                None => self.notice = Some("Select some text to copy.".into()),
            },
            Cmd::Cut => match self.buffer.selected_text() {
                Some(text) => {
                    self.clipboard.copy(&text);
                    self.edit(Kind::Other, |b| {
                        b.delete_selection();
                    });
                    self.notice = Some("Cut. Ctrl+V pastes it.".into());
                }
                None => self.notice = Some("Select some text to cut.".into()),
            },
            Cmd::Paste => {
                let text = self.clipboard.paste();
                if !text.is_empty() {
                    self.edit(Kind::Other, |b| b.paste(&text));
                }
            }
            Cmd::Deselect => self.buffer.clear_selection(),
            Cmd::OpenLink => match self.markup.link_at(self.buffer.cursor()) {
                Some(link) => {
                    let url = link.url.clone();
                    self.open(&url);
                }
                None => self.notice = Some("Put the cursor on a link to open it.".into()),
            },
            Cmd::SelectAll => self.step(Buffer::select_all),
            Cmd::Menu => self.menu = Some(Menu::new(Editor::menu_items())),
            Cmd::CloseMenu => self.menu = None,
            Cmd::RunMenu => {
                if let Some(cmd) = self.menu.take().and_then(|menu| menu.selected()) {
                    return self.run(cmd);
                }
            }
            Cmd::CopyMarkdown => {
                self.clipboard.copy(&self.buffer.text());
                self.notice = Some("Copied the whole post as markdown.".into());
            }
            Cmd::Export => self.open_export(),
            Cmd::Form(cmd) => {
                if let Some(exporting) = &mut self.exporting {
                    let outcome = exporting.form.run(cmd);
                    return self.form_outcome(outcome);
                }
            }
            Cmd::ConfirmReplace => {
                self.dialog = None;
                if let Some(path) = self.replacing.take() {
                    return self.export(&path);
                }
            }
            Cmd::Quit => return Action::Quit,
        }
        Action::None
    }

    /// The post's title as typed (empty if it has none).
    fn raw_title(&self) -> String {
        title_from_first_line(&self.buffer.rope().line(0).to_string())
    }

    /// Open the export dialog, starting from how the post was last
    /// exported (or another post was, if it hasn't been).
    fn open_export(&mut self) {
        let title = self.raw_title();
        let today = jiff::Zoned::now().date();
        let suggested = front_matter::suggest(&title, &self.buffer.text(), today);
        let (last, same_post) = match &self.exports.this_post {
            Some(this_post) => (Some(this_post), true),
            None => (self.exports.latest.as_ref(), false),
        };
        let text = front_matter::front_matter(&suggested, last, same_post);
        let on = last.is_none_or(|last| last.with_front_matter);
        let path = files::default_path(&title, self.exports.this_post.as_ref(), self.exports.latest.as_ref());
        let form = FileDialog::new(
            "Export as markdown",
            "Writes the post to a markdown file.",
            "A file, or a folder to put it in. Tab completes.",
            "Export",
            &path,
        )
        .with_front_matter(&text, on);
        self.exporting = Some(Exporting { form, suggested });
    }

    fn form_outcome(&mut self, outcome: FormOutcome) -> Action {
        match outcome {
            FormOutcome::None => {}
            FormOutcome::Cancel => self.exporting = None,
            FormOutcome::Submit => {
                let title = self.raw_title();
                let Some(exporting) = &mut self.exporting else { return Action::None };
                match files::resolve(exporting.form.path(), &title) {
                    Err(error) => exporting.form.set_error(error),
                    Ok(path) if path.exists() => {
                        let dialog = Dialog::new(
                            "Replace file?",
                            vec![
                                Line::from(truncate(&format!("{} already exists.", files::display(&path)), 50)),
                                Line::from("Exporting replaces it with this post.".dim()),
                            ],
                            vec![
                                Button { label: "Replace", key: "y", cmd: Cmd::ConfirmReplace, danger: true },
                                Button { label: "Cancel", key: "n", cmd: Cmd::CancelDialog, danger: false },
                            ],
                            1,
                            1,
                        );
                        self.dialog = Some(dialog);
                        self.replacing = Some(path);
                    }
                    Ok(path) => return self.export(&path),
                }
            }
        }
        Action::None
    }

    /// Write the post to `path`, as the export dialog says.
    fn export(&mut self, path: &std::path::Path) -> Action {
        let body = self.buffer.text();
        let Some(exporting) = &mut self.exporting else { return Action::None };
        let (on, front_matter) = exporting.form.front_matter().unwrap_or_default();
        let contents = files::contents(&body, on.then_some(front_matter.as_str()));
        if let Err(error) = files::write(path, &contents) {
            exporting.form.set_error(error);
            return Action::None;
        }
        let settings = ExportSettings {
            path: path.to_path_buf(),
            front_matter,
            suggested: std::mem::take(&mut exporting.suggested),
            with_front_matter: on,
        };
        self.exporting = None;
        self.notice = Some(format!("Exported to {}", files::display(path)));
        self.exports.this_post = Some(settings.clone());
        self.exports.latest = Some(settings.clone());
        Action::RecordExport(settings)
    }

    /// Open a link in the browser. Only web and email links: anything else
    /// (a file, an app) could run something.
    fn open(&mut self, url: &str) {
        let result = browser::open(url);
        match result {
            Ok(()) => self.notice = Some(format!("Opened {}", truncate(url, self.width.saturating_sub(7)))),
            Err(error) => self.error = Some(error),
        }
    }

    /// Replace the text with a version's, as one step that can be undone.
    fn restore(&mut self, version: Version) {
        let before = self.buffer.clone();
        self.buffer = Buffer::new(&version.body, Some(before.cursor()));
        if self.buffer.rope() != before.rope() {
            self.undo.edited(Kind::Other, before, &self.buffer);
            self.changed_text();
        }
        self.notice = Some(match version.name.as_str() {
            "" => "Restored the version. Ctrl+Z undoes it.".to_string(),
            name => format!("Restored “{name}”. Ctrl+Z undoes it."),
        });
    }

    /// Change the text (recording it for undo), then re-wrap and scroll to
    /// the cursor.
    fn edit(&mut self, kind: Kind, f: impl FnOnce(&mut Buffer)) {
        let before = self.buffer.clone();
        // Replacing a selection is a step of its own.
        let replacing = before.selection().is_some();
        let kind = if replacing { Kind::Other } else { kind };
        f(&mut self.buffer);
        // Every edit changes the length, unless it replaced a selection; if
        // not, nothing happened (e.g. Backspace at the very start).
        if replacing || self.buffer.len() != before.len() {
            self.undo.edited(kind, before, &self.buffer);
            self.changed_text();
        }
    }

    /// Swap in an earlier or later version of the text, for undo and redo.
    fn replace(&mut self, buffer: Buffer) {
        self.buffer = buffer;
        self.changed_text();
    }

    fn changed_text(&mut self) {
        self.autosave.edited(Instant::now());
        self.words = None;
        self.stale = true;
        self.follow = true;
        self.goal = None;
    }

    /// Move the cursor within the text, then scroll to it.
    fn step(&mut self, f: impl FnOnce(&mut Buffer)) {
        f(&mut self.buffer);
        self.follow = true;
        self.goal = None;
    }

    /// Move the cursor up (negative) or down `by` rows, keeping its column.
    /// Past the first or last row, go to the start or end of the text.
    fn move_rows(&mut self, by: isize) {
        self.refresh();
        let rope = self.buffer.rope();
        let pos = self.buffer.cursor();
        let row = row_of(&self.rows, pos);
        let goal = *self.goal.get_or_insert_with(|| x_of(rope, self.rows[row], pos));
        let pos = match row.checked_add_signed(by).and_then(|r| self.rows.get(r)) {
            Some(&target) => pos_at_x(rope, target, goal),
            None if by < 0 => 0,
            None => self.buffer.len(),
        };
        self.buffer.set_cursor(pos);
        self.follow = true;
    }

    /// Scroll a screenful, taking the cursor along.
    fn page(&mut self, dir: isize) {
        self.refresh();
        let by = self.text_area.height.saturating_sub(1).max(1) as isize * dir;
        self.top = self.top.saturating_add_signed(by).min(self.max_top());
        self.move_rows(by);
    }

    fn row_start(&mut self) {
        self.refresh();
        let row = self.rows[row_of(&self.rows, self.buffer.cursor())];
        self.step(|b| b.set_cursor(row.start));
    }

    /// The end of the row the cursor is on. On a row that continues on the
    /// next one, that's just before its last character.
    fn row_end(&mut self) {
        self.refresh();
        let row = self.rows[row_of(&self.rows, self.buffer.cursor())];
        self.step(|b| b.set_cursor(if row.last { row.end } else { b.prev_boundary(row.end) }));
    }

    /// Put the cursor where the text was clicked. Beside a row means its
    /// start or end; below the text, the very end. Double-click selects a
    /// word, triple-click a paragraph; otherwise dragging selects.
    fn click(&mut self, column: u16, row: u16) {
        if !self.in_text(row) {
            return;
        }
        let pos = self.pos_at(column, row);
        let now = Instant::now();
        let count = match self.last_click {
            Some(last) if now - last.at < MULTI_CLICK && (last.column, last.row) == (column, row) => last.count % 3 + 1,
            _ => 1,
        };
        self.last_click = Some(Click { at: now, column, row, count });
        self.goal = None;
        match count {
            1 => {
                self.buffer.clear_selection();
                self.buffer.set_cursor(pos);
                self.dragging = true;
            }
            2 => {
                let word = self.buffer.word_at(pos);
                self.buffer.select(word.start, word.end);
            }
            _ => {
                let line = self.buffer.line_at(pos);
                self.buffer.select(line.start, line.end);
            }
        }
    }

    /// Extend the selection to the dragged-to cell. Dragging past the top
    /// or bottom of the text scrolls it.
    fn drag(&mut self, column: u16, row: u16) {
        let area = self.text_area;
        self.refresh();
        let row = if row < area.y {
            self.top = self.top.saturating_sub(1);
            area.y
        } else if row >= area.bottom() {
            if self.top < self.max_top() {
                self.top += 1;
            }
            area.bottom() - 1
        } else {
            row
        };
        let pos = self.pos_at(column, row);
        self.buffer.begin_select();
        self.buffer.set_cursor(pos);
        self.buffer.settle();
        self.goal = None;
    }

    /// The text position at a cell of the text area (below the text: its end).
    fn pos_at(&mut self, column: u16, row: u16) -> usize {
        self.refresh();
        // The blank rows above the title count as its first row.
        let index = (self.top + row.saturating_sub(self.text_area.y) as usize).saturating_sub(PAD_TOP);
        match self.rows.get(index) {
            Some(&r) => pos_at_x(self.buffer.rope(), r, column.saturating_sub(self.text_area.x) as usize),
            None => self.buffer.len(),
        }
    }

    fn in_text(&self, row: u16) -> bool {
        (self.text_area.y..self.text_area.bottom()).contains(&row)
    }

    fn refresh(&mut self) {
        if self.stale {
            self.rows = layout(self.buffer.rope(), self.width);
            self.markup = Markup::new(self.buffer.rope());
            self.stale = false;
        }
    }

    /// The cursor's row (counting the blank rows above the title) and
    /// column on screen. Just after a space hanging off the end of a full
    /// row, it shows at the start of the next row, where the next word will
    /// go.
    fn cursor_cell(&self) -> (usize, usize) {
        let pos = self.buffer.cursor();
        let row = row_of(&self.rows, pos);
        let x = x_of(self.buffer.rope(), self.rows[row], pos);
        let (row, x) = if x > self.width { (row + 1, self.rows.get(row + 1).map_or(0, |r| r.indent)) } else { (row, x) };
        (PAD_TOP + row, x)
    }

    /// The furthest the text can scroll: its last row a margin above the
    /// bottom edge.
    fn max_top(&self) -> usize {
        let height = self.text_area.height as usize;
        (PAD_TOP + self.rows.len()).saturating_sub(height.saturating_sub(margin(height)))
    }
}

/// Words in the text: runs of non-space with a letter or digit in them (so
/// markdown's `#`, `-` and `---` don't count).
fn count_words(rope: &Rope) -> usize {
    let mut count = 0;
    let mut counted = false;
    for ch in rope.chars() {
        if ch.is_whitespace() {
            counted = false;
        } else if !counted && ch.is_alphanumeric() {
            counted = true;
            count += 1;
        }
    }
    count
}

/// A number with commas between the thousands: 12,345.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn margin(height: usize) -> usize {
    SCROLL_MARGIN.min(height.saturating_sub(1) / 2)
}

/// One row of text, formatted, with the selected part reversed. The title
/// has a dim placeholder while empty.
fn row_line(rope: &Rope, markup: &Markup, row: &Row, selection: Option<&Range<usize>>) -> Line<'static> {
    let text = |range: Range<usize>| rope.slice(range).to_string().replace('\t', &" ".repeat(TAB_WIDTH));
    let selected = selection
        .map(|s| s.start.max(row.start)..s.end.min(row.end))
        .filter(|s| s.start < s.end);
    let mut spans = vec![Span::raw(" ".repeat(row.indent))];
    for (piece, style) in markup.styles(row.start..row.end) {
        // Split each piece where the selection starts and ends.
        let mut cuts = vec![piece.start, piece.end];
        if let Some(s) = &selected {
            cuts.extend([s.start, s.end].into_iter().filter(|at| piece.contains(at)));
            cuts.sort();
        }
        for part in cuts.windows(2).map(|w| w[0]..w[1]).filter(|p| p.start < p.end) {
            let inside = selected.as_ref().is_some_and(|s| s.contains(&part.start));
            spans.push(Span::styled(text(part), if inside { style.reversed() } else { style }));
        }
    }
    // A selection that runs on past the end of the line takes the newline,
    // which shows as a selected space (so blank lines show up too).
    if row.last && selection.is_some_and(|s| s.start <= row.end && s.end > row.end) {
        spans.push(Span::styled(" ", Style::new().reversed()));
    }
    if row.line == 0 && row.last && rope.slice(row.start..row.end) == TITLE_PREFIX {
        spans.push("Title".dim());
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyEventKind;

    use super::*;

    /// An editor on an 80x12 screen: the text column starts at x = 6 and
    /// the text area spans rows 1..=9.
    struct Harness {
        editor: Editor,
        terminal: Terminal<TestBackend>,
    }

    impl Harness {
        fn new(body: &str) -> Self {
            Harness::at(body, None)
        }

        fn at(body: &str, cursor: Option<usize>) -> Self {
            let post = Post { id: 1, body: body.into(), cursor, created_at: jiff::Timestamp::UNIX_EPOCH };
            let editor = Editor::new(post, None, Exports::default());
            let mut h = Harness { editor, terminal: Terminal::new(TestBackend::new(80, 12)).unwrap() };
            h.draw();
            h
        }

        fn draw(&mut self) {
            self.terminal.draw(|frame| self.editor.render(frame)).unwrap();
        }

        fn key(&mut self, code: KeyCode) {
            let key = KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Press);
            self.editor.handle(Event::Key(key));
            self.draw();
        }

        fn ctrl(&mut self, ch: char) -> Action {
            let key = KeyEvent::new_with_kind(KeyCode::Char(ch), KeyModifiers::CONTROL, KeyEventKind::Press);
            let action = self.editor.handle(Event::Key(key));
            self.draw();
            action
        }

        fn press(&mut self, code: KeyCode) -> Action {
            let key = KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Press);
            let action = self.editor.handle(Event::Key(key));
            self.draw();
            action
        }

        fn screen(&self) -> String {
            (0..12).map(|y| self.screen_row(y)).collect::<Vec<_>>().join("\n")
        }

        fn mouse(&mut self, kind: MouseEventKind, column: u16, row: u16) {
            let mouse = MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE };
            self.editor.handle(Event::Mouse(mouse));
            self.draw();
        }

        fn typed(&mut self, text: &str) {
            for ch in text.chars() {
                self.key(if ch == '\n' { KeyCode::Enter } else { KeyCode::Char(ch) });
            }
        }

        fn cursor(&mut self) -> (u16, u16) {
            let pos = self.terminal.get_cursor_position().unwrap();
            (pos.x, pos.y)
        }

        /// Everything on screen, however tall it is.
        fn all(&self) -> String {
            let height = self.terminal.backend().buffer().area.height;
            (0..height).map(|y| self.screen_row(y)).collect::<Vec<_>>().join("\n")
        }

        fn form(&self) -> &FileDialog {
            &self.editor.exporting.as_ref().expect("the export dialog is open").form
        }

        fn screen_row(&self, y: u16) -> String {
            let buffer = self.terminal.backend().buffer();
            (0..buffer.area.width).map(|x| buffer[(x, y)].symbol()).collect::<String>().trim_end().to_string()
        }
    }

    const LEFT: u16 = 6;

    #[test]
    fn text_wraps_in_a_centred_column() {
        let mut h = Harness::new("# ");
        assert_eq!(h.cursor(), (LEFT + 2, 1));
        h.typed("Title\n");
        let sentence = "All work and no play makes Jack a dull boy. ";
        h.typed(&sentence.repeat(2));
        assert_eq!(h.screen_row(1), format!("{}# Title", " ".repeat(LEFT as usize)));
        assert_eq!(
            h.screen_row(2),
            format!("{}All work and no play makes Jack a dull boy. All work and no play", " ".repeat(6))
        );
        assert_eq!(h.screen_row(3), format!("{}makes Jack a dull boy.", " ".repeat(6)));
        assert_eq!(h.cursor(), (LEFT + 23, 3));
        assert_eq!(h.editor.text(), format!("# Title\n{}", sentence.repeat(2)));
    }

    #[test]
    fn up_and_down_keep_the_column() {
        let mut h = Harness::new("# Title\nThe longest line of them all\nshort\nanother long line here");
        assert_eq!(h.cursor(), (LEFT + 22, 4));
        h.key(KeyCode::Up);
        assert_eq!(h.cursor(), (LEFT + 5, 3)); // end of "short"
        h.key(KeyCode::Up);
        assert_eq!(h.cursor(), (LEFT + 22, 2)); // back to the column it started in
        h.key(KeyCode::Up);
        assert_eq!(h.cursor(), (LEFT + 7, 1));
        h.key(KeyCode::Up); // past the top: the start of the title
        assert_eq!(h.cursor(), (LEFT + 2, 1));
        h.key(KeyCode::End);
        h.key(KeyCode::Down);
        assert_eq!(h.cursor(), (LEFT + 7, 2));
    }

    #[test]
    fn clicks_place_the_cursor() {
        let mut h = Harness::new("# Title\nHello there\n\nWorld");
        let click = MouseEventKind::Down(MouseButton::Left);
        h.mouse(click, LEFT + 3, 2);
        assert_eq!(h.cursor(), (LEFT + 3, 2));
        h.mouse(click, 70, 2); // right of the text: end of the row
        assert_eq!(h.cursor(), (LEFT + 11, 2));
        h.mouse(click, 0, 1); // left of the title: just after "# "
        assert_eq!(h.cursor(), (LEFT + 2, 1));
        h.mouse(click, 30, 8); // below the text: the end
        assert_eq!(h.cursor(), (LEFT + 5, 4));
    }

    #[test]
    fn scrolls_to_follow_the_cursor() {
        let body: String = (1..=30).map(|n| format!("\nline {n}")).collect();
        let mut h = Harness::new(&format!("# Title{body}"));
        // The cursor starts at the end, with a margin below it.
        assert_eq!(h.screen_row(6), format!("{}line 30", " ".repeat(6)));
        assert_eq!(h.cursor(), (LEFT + 7, 6));
        h.key(KeyCode::Home);
        for _ in 0..10 {
            h.key(KeyCode::Up);
        }
        // Scrolled up, keeping a margin above the cursor. The blank row
        // above the title has scrolled away: the text starts on the first.
        assert_eq!(h.screen_row(3), format!("{}line 20", " ".repeat(6)));
        assert_eq!(h.screen_row(0), format!("{}line 17", " ".repeat(6)));
        assert_eq!(h.cursor(), (LEFT, 3));
        // The wheel scrolls without moving the cursor.
        h.mouse(MouseEventKind::ScrollUp, 40, 5);
        assert_eq!(h.cursor(), (LEFT, 4));
        h.key(KeyCode::Char('!'));
        assert_eq!(h.editor.text().lines().nth(20), Some("!line 20"));
    }

    #[test]
    fn reopens_where_the_cursor_was_left() {
        let body: String = (1..=30).map(|n| format!("\nline {n}")).collect();
        let text = format!("# Title{body}");
        let pos = text.find("line 15").unwrap() + 2;
        let mut h = Harness::at(&text, Some(pos));
        // In the middle of the screen.
        assert_eq!(h.screen_row(5), format!("{}line 15", " ".repeat(6)));
        assert_eq!(h.cursor(), (LEFT + 2, 5));
        assert_eq!(h.editor.cursor(), pos);
        // Near the top, it can't be mid-screen.
        let mut h = Harness::at(&text, Some(9));
        assert_eq!(h.screen_row(1), format!("{}# Title", " ".repeat(6)));
        assert_eq!(h.cursor(), (LEFT + 1, 2));
    }

    #[test]
    fn tab_types_two_spaces() {
        let mut h = Harness::new("# Title\n");
        h.key(KeyCode::Tab);
        assert_eq!(h.editor.text(), "# Title\n  ");
    }

    #[test]
    fn the_title_prefix_stays() {
        let mut h = Harness::new("# Hi\nthere");
        h.key(KeyCode::Down); // at the end already: stays
        for _ in 0..20 {
            h.key(KeyCode::Backspace);
        }
        assert_eq!(h.editor.text(), "# ");
        assert!(h.editor.changed());
        h.key(KeyCode::Delete);
        assert_eq!(h.editor.text(), "# ");
    }

    #[test]
    fn undo_and_redo_with_keys_and_hints() {
        let mut h = Harness::new("# Title\n");
        h.typed("Hello world");
        h.ctrl('z');
        assert_eq!(h.editor.text(), "# Title\nHello ");
        assert_eq!(h.cursor(), (LEFT + 6, 2));
        h.ctrl('z');
        h.ctrl('z'); // nothing more to undo
        assert_eq!(h.editor.text(), "# Title\n");
        h.ctrl('y');
        assert_eq!(h.editor.text(), "# Title\nHello ");
        // Also from the menu.
        h.ctrl('k');
        h.typed("redo\n");
        assert_eq!(h.editor.text(), "# Title\nHello world");
    }

    #[test]
    fn edits_are_due_for_autosave() {
        let mut h = Harness::new("# Title");
        assert_eq!(h.editor.autosave_due(), None);
        h.key(KeyCode::Right); // moving isn't an edit
        h.key(KeyCode::Backspace);
        h.key(KeyCode::Backspace);
        assert_eq!(h.editor.text(), "# Tit");
        let due = h.editor.autosave_due().unwrap();
        assert!(due > Instant::now() && due <= Instant::now() + autosave::PAUSE);
        h.editor.mark_stored(Some(h.editor.text()));
        assert_eq!(h.editor.autosave_due(), None);
        assert!(!h.editor.changed());
        h.ctrl('z'); // undo is an edit too
        assert!(h.editor.changed());
        assert!(h.editor.autosave_due().is_some());
    }

    impl Harness {
        fn with(&mut self, code: KeyCode, modifiers: KeyModifiers) {
            let key = KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press);
            self.editor.handle(Event::Key(key));
            self.draw();
        }

        fn shift(&mut self, code: KeyCode) {
            self.with(code, KeyModifiers::SHIFT);
        }
    }

    fn selected(h: &Harness) -> Option<String> {
        h.editor.buffer.selected_text()
    }

    #[test]
    fn shift_movement_selects() {
        let mut h = Harness::new("# Title\nHello world");
        h.shift(KeyCode::Left);
        h.shift(KeyCode::Left);
        assert_eq!(selected(&h).as_deref(), Some("ld"));
        h.shift(KeyCode::Right);
        assert_eq!(selected(&h).as_deref(), Some("d"));
        h.with(KeyCode::Left, KeyModifiers::SHIFT | KeyModifiers::ALT);
        assert_eq!(selected(&h).as_deref(), Some("world"));
        // The selection is drawn reversed.
        let buffer = h.terminal.backend().buffer();
        assert!(buffer[(LEFT + 6, 2)].modifier.contains(ratatui::style::Modifier::REVERSED));
        assert!(!buffer[(LEFT + 5, 2)].modifier.contains(ratatui::style::Modifier::REVERSED));
        // Plain Left drops it, at its start.
        h.key(KeyCode::Left);
        assert_eq!(selected(&h), None);
        assert_eq!(h.cursor(), (LEFT + 6, 2));
        h.shift(KeyCode::Up);
        assert_eq!(selected(&h).as_deref(), Some("e\nHello "));
        // Typing replaces the selection, as one undo step.
        h.typed("x");
        assert_eq!(h.editor.text(), "# Titlxworld");
        h.ctrl('z');
        assert_eq!(h.editor.text(), "# Title\nHello world");
    }

    #[test]
    fn ctrl_or_cmd_left_and_right_go_to_the_line_ends() {
        let mut h = Harness::new("# Title\nHello world");
        h.with(KeyCode::Left, KeyModifiers::CONTROL);
        assert_eq!(h.cursor(), (LEFT, 2));
        h.with(KeyCode::Right, KeyModifiers::CONTROL);
        assert_eq!(h.cursor(), (LEFT + 11, 2));
        h.with(KeyCode::Left, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        assert_eq!(selected(&h).as_deref(), Some("Hello world"));
        // Option still moves by word.
        h.with(KeyCode::Right, KeyModifiers::ALT);
        assert_eq!(h.cursor(), (LEFT + 5, 2));
        // Cmd only on macOS: elsewhere it's the Windows key, left alone.
        h.with(KeyCode::Right, KeyModifiers::SUPER);
        let end = if cfg!(target_os = "macos") { LEFT + 11 } else { LEFT + 5 };
        assert_eq!(h.cursor(), (end, 2));
    }

    #[test]
    fn select_all_cut_and_paste() {
        let mut h = Harness::new("# Title\nHello world");
        h.ctrl('a');
        h.ctrl('c');
        assert!(h.screen_row(10).contains("Copied."));
        h.key(KeyCode::Backspace);
        assert_eq!(h.editor.text(), "# ");
        h.ctrl('v'); // the copy included the "# ", which Backspace kept
        assert_eq!(h.editor.text(), "# Title\nHello world"); // not "# # Title"
        // Cut a word and put it back somewhere else.
        h.with(KeyCode::Left, KeyModifiers::ALT | KeyModifiers::SHIFT);
        h.ctrl('x');
        assert_eq!(h.editor.text(), "# Title\nHello ");
        h.with(KeyCode::Up, KeyModifiers::ALT);
        h.ctrl('v');
        assert_eq!(h.editor.text(), "# Title\nworldHello ");
        // Paste over a selection.
        h.shift(KeyCode::Right);
        h.ctrl('v');
        assert_eq!(h.editor.text(), "# Title\nworldworldello ");
    }

    #[test]
    fn esc_deselects_before_leaving() {
        let mut h = Harness::new("# Title");
        h.ctrl('a');
        assert!(matches!(h.press(KeyCode::Esc), Action::None));
        assert_eq!(selected(&h), None);
        assert!(matches!(h.press(KeyCode::Esc), Action::BackToList(_)));
    }

    #[test]
    fn mouse_selects_by_drag_word_and_paragraph() {
        let mut h = Harness::new("# Title\nHello there world\nsecond");
        let down = MouseEventKind::Down(MouseButton::Left);
        h.mouse(down, LEFT + 6, 2);
        h.mouse(MouseEventKind::Drag(MouseButton::Left), LEFT + 11, 2);
        h.mouse(MouseEventKind::Up(MouseButton::Left), LEFT + 11, 2);
        assert_eq!(selected(&h).as_deref(), Some("there"));
        // A plain click clears it; two more make a double click on a word.
        h.mouse(down, LEFT + 1, 2);
        assert_eq!(selected(&h), None);
        h.mouse(MouseEventKind::Up(MouseButton::Left), LEFT + 1, 2);
        h.mouse(down, LEFT + 1, 2);
        assert_eq!(selected(&h).as_deref(), Some("Hello"));
        h.mouse(down, LEFT + 1, 2); // triple
        assert_eq!(selected(&h).as_deref(), Some("Hello there world"));
        // Dragging down past the text selects to its end.
        h.mouse(MouseEventKind::Up(MouseButton::Left), LEFT + 1, 2);
        h.mouse(down, LEFT + 2, 1);
        h.mouse(MouseEventKind::Drag(MouseButton::Left), 30, 8);
        assert_eq!(selected(&h).as_deref(), Some("Title\nHello there world\nsecond"));
    }

    fn version(id: i64, name: &str, body: &str) -> Version {
        Version { id, name: name.into(), created_at: jiff::Timestamp::now(), body: body.into() }
    }

    #[test]
    fn ctrl_s_saves_a_named_version() {
        let mut h = Harness::new("# Title\nSome text");
        h.ctrl('s');
        assert!(h.screen().contains("saved automatically as you type"));
        h.typed("First pass");
        let action = h.press(KeyCode::Enter);
        assert!(matches!(action, Action::SaveVersion { ref name } if name == "First pass"));
        // The app records it and tells the editor.
        h.editor.version_saved(version(1, "First pass", "# Title\nSome text"));
        h.draw();
        assert!(h.screen_row(10).contains("Saved version “First pass”."));
        // Nothing new to save.
        h.ctrl('s');
        assert!(h.screen_row(10).contains("Nothing has changed since the last version."));
        h.typed("!");
        // Esc cancels naming a version.
        h.ctrl('s');
        assert!(matches!(h.press(KeyCode::Esc), Action::None));
        assert!(!h.screen().contains("saved automatically"));
    }

    #[test]
    fn history_restores_a_version_and_undo_takes_it_back() {
        let first = version(1, "First pass", "# Title\nOld text");
        let editor = Editor::new(Post { id: 1, body: "# Title\nNew text".into(), cursor: None, created_at: jiff::Timestamp::UNIX_EPOCH }, Some(first.clone()), Exports::default());
        let mut h = Harness { editor, terminal: Terminal::new(TestBackend::new(80, 12)).unwrap() };
        h.draw();
        assert!(matches!(h.ctrl('r'), Action::ShowHistory));
        h.editor.show_history(vec![first.clone()]);
        h.draw();
        assert!(h.screen().contains("Versions of “Title”"));
        assert!(h.screen().contains("First pass"));
        assert!(matches!(h.press(KeyCode::Enter), Action::LoadVersion(1)));
        h.editor.show_version(first);
        h.draw();
        assert!(h.screen().contains("Old text"));
        h.press(KeyCode::Enter);
        assert!(h.screen().contains("Replace the post with “First pass”?"));
        assert!(h.screen().contains("Its current text isn't saved as a version."));
        h.press(KeyCode::Char('y'));
        assert_eq!(h.editor.text(), "# Title\nOld text");
        assert!(h.screen_row(10).contains("Restored “First pass”."));
        h.ctrl('z');
        assert_eq!(h.editor.text(), "# Title\nNew text");
    }

    #[test]
    fn history_needs_a_version() {
        let mut h = Harness::new("# Title");
        assert!(matches!(h.ctrl('r'), Action::None));
        assert!(h.screen_row(10).contains("No versions yet."));
    }

    #[test]
    fn hints_hide_until_ctrl_is_held() {
        let mut h = Harness::new("# Title\nSome text");
        h.editor.show_chrome = false;
        h.draw();
        assert_eq!(h.screen_row(10), "");
        assert_eq!(h.screen_row(11), "");
        // Hidden hints can't be clicked.
        h.mouse(MouseEventKind::Down(MouseButton::Left), 40, 11);
        assert!(h.screen().contains("Some text"));
        // Passing messages still show.
        h.ctrl('x');
        assert!(h.screen_row(10).contains("Select some text to cut."));
        // So does saving a version, with its own hints.
        h.ctrl('s');
        assert!(h.screen().contains("Save version"));
        assert_ne!(h.screen_row(11), "");
        h.press(KeyCode::Esc);
        h.editor.show_chrome = true;
        h.draw();
        assert!(h.screen_row(11).contains("Ctrl+K menu"));
    }

    #[test]
    fn markdown_is_formatted_as_you_type() {
        use ratatui::style::{Color, Modifier};
        let mut h = Harness::new("# Title\n");
        h.typed("Some **bold**, `code` and **unfinished");
        let buffer = h.terminal.backend().buffer();
        let cell = |x: u16, y: u16| &buffer[(LEFT + x, y)];
        // The title: '#' faded, the words bold.
        assert!(cell(0, 1).modifier.contains(Modifier::DIM));
        assert!(cell(2, 1).modifier.contains(Modifier::BOLD));
        // "**" faded, "bold" bold.
        assert!(cell(5, 2).modifier.contains(Modifier::DIM));
        assert!(cell(7, 2).modifier.contains(Modifier::BOLD));
        // Code green, its backticks faded.
        assert!(cell(15, 2).modifier.contains(Modifier::DIM));
        assert_eq!(cell(16, 2).fg, Color::Green);
        // Not closed yet: plain.
        assert_eq!(cell(26, 2).modifier, Modifier::empty());
        assert_eq!(cell(28, 2).modifier, Modifier::empty());
        // Selecting keeps the formatting, reversed.
        h.ctrl('a');
        let buffer = h.terminal.backend().buffer();
        let bold = &buffer[(LEFT + 7, 2)];
        assert!(bold.modifier.contains(Modifier::BOLD | Modifier::REVERSED));
    }

    #[test]
    fn links_open_with_ctrl_o_or_ctrl_click() {
        let mut h = Harness::new("# Title\nSee [the docs](https://example.com) and file:///x");
        // Not on a link: Ctrl+O says how.
        h.ctrl('o');
        assert!(h.screen_row(10).contains("Put the cursor on a link"));
        // On a link, Ctrl+O opens it.
        h.mouse(MouseEventKind::Down(MouseButton::Left), LEFT + 6, 2);
        h.ctrl('o');
        assert!(h.screen_row(10).contains("Opened https://example.com"));
        // Ctrl+click on the link opens it without moving the cursor.
        h.key(KeyCode::Home);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: LEFT + 30,
            row: 2,
            modifiers: KeyModifiers::CONTROL,
        };
        h.editor.handle(Event::Mouse(mouse));
        h.draw();
        assert!(h.screen_row(10).contains("Opened https://example.com"));
        assert_eq!(h.cursor(), (LEFT, 2));
        // A Ctrl+click elsewhere is an ordinary click.
        h.editor.handle(Event::Mouse(MouseEvent { column: LEFT + 2, ..mouse }));
        h.draw();
        assert_eq!(h.cursor(), (LEFT + 2, 2));
    }

    #[test]
    fn list_items_wrap_under_their_text() {
        let point = "- A point long enough that it wraps onto a second row, which lines up under its text";
        let mut h = Harness::new(&format!("# Title\n{point}"));
        assert_eq!(h.screen_row(2), format!("{}- A point long enough that it wraps onto a second row, which lines", " ".repeat(LEFT as usize)));
        assert_eq!(h.screen_row(3), format!("{}  up under its text", " ".repeat(LEFT as usize)));
        // The cursor, at the end, is past the indent.
        assert_eq!(h.cursor(), (LEFT + 19, 3));
        // Clicking in the indent puts it at the row's start.
        h.mouse(MouseEventKind::Down(MouseButton::Left), LEFT, 3);
        assert_eq!(h.cursor(), (LEFT + 2, 3));
        h.typed("x");
        assert!(h.editor.text().ends_with("lines xup under its text"));
    }

    #[test]
    fn zen_text_takes_the_whole_screen() {
        let body: String = (1..=30).map(|n| format!("\nline {n}")).collect();
        let mut h = Harness::new(&format!("# Title{body}"));
        h.editor.zen = true;
        h.editor.show_chrome = false;
        h.with(KeyCode::Home, KeyModifiers::CONTROL);
        // The text runs to the bottom row.
        assert_eq!(h.screen_row(11), format!("{}line 10", " ".repeat(6)));
        // Holding Ctrl draws the hints over the bottom row.
        h.editor.show_chrome = true;
        h.draw();
        assert_eq!(h.screen_row(10), format!("{}line 9", " ".repeat(6)));
        assert!(h.screen_row(11).contains("Ctrl+K menu"));
        // A passing message covers only its own row.
        h.editor.show_chrome = false;
        h.ctrl('x');
        assert!(h.screen_row(10).contains("Select some text to cut."));
        assert_eq!(h.screen_row(11), format!("{}line 10", " ".repeat(6)));
    }

    #[test]
    fn the_bar_shows_the_menu_words_and_when_the_post_was_started() {
        let mut h = Harness::new("# Title\n\n- Some *text*\n\n---\n");
        let bar = h.screen_row(11);
        // As wide as the text, which starts at LEFT.
        assert!(bar.starts_with(&format!("{}Ctrl+K menu", " ".repeat(LEFT as usize))), "{bar}");
        assert!(bar.contains("3 words · "), "{bar}");
        assert_eq!(bar.chars().count(), (LEFT + COLUMN_WIDTH) as usize);
        // Clicking the hint opens the menu.
        h.mouse(MouseEventKind::Down(MouseButton::Left), LEFT + 1, 11);
        assert!(h.screen().contains("type to filter"));
        // Narrower: just the words, then nothing.
        let mut h = Harness::new("# Title\nOne");
        h.terminal = Terminal::new(TestBackend::new(30, 12)).unwrap();
        h.draw();
        assert!(h.screen_row(11).ends_with("2 words"));
        h.terminal = Terminal::new(TestBackend::new(16, 12)).unwrap();
        h.draw();
        assert!(!h.screen_row(11).contains("word"));
    }

    #[test]
    fn words_are_counted_without_markdown() {
        let count = |text: &str| count_words(&Rope::from_str(text));
        assert_eq!(count("# Title\n\n- one *two*\n\n---\n> three, four"), 5);
        assert_eq!(count(""), 0);
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(1248), "1,248");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn the_menu_filters_and_runs_commands() {
        let mut h = Harness::new("# Title\nHello world");
        h.ctrl('k');
        let screen = h.screen();
        assert!(screen.contains("type to filter"));
        assert!(screen.contains("Save version") && screen.contains("Ctrl+S"));
        assert!(h.screen_row(11).contains("Esc close"));
        // Nothing is highlighted, so Enter does nothing.
        h.key(KeyCode::Enter);
        assert!(h.editor.menu.is_some());
        // Typing narrows the list and highlights the first match.
        h.typed("select");
        assert!(!h.screen().contains("Save version"));
        assert!(h.screen_row(11).contains("Enter run"));
        h.key(KeyCode::Enter);
        assert!(h.editor.menu.is_none());
        assert_eq!(h.editor.buffer.selected_text().as_deref(), Some("# Title\nHello world"));
        // Esc and Ctrl+K close it, without running anything.
        h.ctrl('k');
        h.key(KeyCode::Esc);
        assert!(h.editor.menu.is_none());
        assert!(h.editor.buffer.selection().is_some());
        h.ctrl('k');
        h.ctrl('k');
        assert!(h.editor.menu.is_none());
        // Commands can be clicked; a click outside closes the menu.
        h.ctrl('k');
        let (y, row) = (0..12).map(|y| (y, h.screen_row(y))).find(|(_, row)| row.contains("History")).unwrap();
        let x = row.find("History").unwrap() as u16;
        h.mouse(MouseEventKind::Down(MouseButton::Left), x, y);
        assert!(h.editor.menu.is_none());
        assert!(h.screen_row(10).contains("No versions yet."));
        h.ctrl('k');
        h.mouse(MouseEventKind::Down(MouseButton::Left), 0, 0);
        assert!(h.editor.menu.is_none());
        // Ones that don't fit scroll into view.
        h.ctrl('k');
        h.key(KeyCode::Up);
        assert!(h.screen().contains("Quit"));
        assert!(matches!(h.editor.handle(Event::Key(KeyEvent::from(KeyCode::Enter))), Action::Quit));
    }

    #[test]
    fn copy_as_markdown_copies_the_whole_post() {
        let mut h = Harness::new("# Title\nSome **bold**");
        // The kitty keyboard protocol reports Ctrl+Shift+C as Ctrl+"C".
        h.ctrl('C');
        assert!(h.screen_row(10).contains("Copied the whole post as markdown."));
        assert_eq!(h.editor.clipboard.paste(), "# Title\nSome **bold**");
        // Other terminals, as Ctrl+Shift+"c".
        h.with(KeyCode::Char('c'), KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        assert!(h.screen_row(10).contains("Copied the whole post"));
    }

    #[test]
    fn export_writes_front_matter_and_asks_before_replacing() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().to_str().unwrap();
        let file = dir.path().join("my-post.md");
        let today = jiff::Zoned::now().date();
        let mut h = Harness::new("# My Post\n\nHello **there**");
        h.terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        h.ctrl('S');
        // The post's file name in the current folder, and front matter
        // from the post.
        let screen = h.all();
        assert!(screen.contains("Export as markdown"), "{screen}");
        assert!(screen.contains("[x] Front matter"));
        assert!(screen.contains("title: \"My Post\"") && screen.contains("slug: my-post"));
        assert!(screen.contains("description: \"Hello there\""));
        assert!(screen.contains("Tab complete"));
        assert!(h.form().path().ends_with("/my-post.md"));

        // Tab completes the folder.
        h.ctrl('u');
        h.typed(&folder[..folder.len() - 1]);
        h.key(KeyCode::Tab);
        assert_eq!(h.form().path(), format!("{folder}/"));
        // Add a field at the end of the front matter, and export.
        h.key(KeyCode::Down);
        h.key(KeyCode::Down);
        for _ in 0..4 {
            h.key(KeyCode::Down);
        }
        h.key(KeyCode::End);
        h.typed("\ntags: [a]");
        let Action::RecordExport(settings) = h.ctrl('s') else { panic!("not exported") };
        let expected = format!(
            "---\ntitle: \"My Post\"\ndate: {today}\nupdated: {today}\ndescription: \"Hello there\"\n\
             slug: my-post\ntags: [a]\n---\n\nHello **there**\n"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), expected);
        assert_eq!(settings.path, file);
        assert!(settings.with_front_matter && settings.front_matter.ends_with("tags: [a]"));
        assert!(h.all().contains("Exported to"));

        // Again: the same file and front matter. The file is there, so it
        // asks first; cancelling goes back to the dialog.
        h.typed(" again");
        h.ctrl('S');
        assert_eq!(h.form().path(), file.to_str().unwrap());
        assert!(h.all().contains("tags: [a]"));
        h.key(KeyCode::Enter);
        assert!(h.all().contains("Replace file?"));
        h.key(KeyCode::Char('n'));
        assert!(!h.all().contains("Replace file?"));
        assert!(h.editor.exporting.is_some());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), expected);
        // Without front matter, it's the post as it is.
        h.key(KeyCode::Down);
        h.key(KeyCode::Char(' '));
        assert!(h.all().contains("[ ] Front matter"));
        assert!(!h.all().contains("tags: [a]"));
        h.ctrl('s');
        h.key(KeyCode::Char('y'));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "# My Post\n\nHello **there** again\n");
        assert!(h.editor.exporting.is_none());
        // It's remembered as off.
        h.ctrl('S');
        assert!(h.all().contains("[ ] Front matter"));

        // A folder that isn't there is refused, and the dialog stays open.
        h.ctrl('u');
        h.typed("/no/such/folder/x.md");
        h.key(KeyCode::Enter);
        assert!(h.all().contains("There's no folder"));
        assert!(h.editor.exporting.is_some());
        h.key(KeyCode::Esc);
        assert!(h.editor.exporting.is_none());
    }
}
