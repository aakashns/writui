//! The post editing screen: the text in a centred column, soft-wrapped, with
//! the cursor, scrolling, the mouse, undo, and saves.

mod autosave;
mod buffer;
mod clipboard;
mod history;
mod undo;
mod wrap;

use std::ops::Range;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ropey::Rope;

use super::Action;
use super::dialog::{Button, Dialog};
use super::hints::{HintBar, hint};
use super::widgets::{COLUMN_WIDTH, truncate};
use crate::vault::{Post, Save, TITLE_PREFIX, title_from_first_line};
use autosave::Autosave;
use buffer::Buffer;
use clipboard::Clipboard;
use history::{History, Outcome};
use undo::{Kind, Undo};
use wrap::{Row, TAB_WIDTH, layout, pos_at_x, row_of, x_of};

/// What the Tab key types.
const TAB: &str = "  ";

/// Clicks this close together (in time, and in place) make a double or
/// triple click.
const MULTI_CLICK: Duration = Duration::from_millis(500);

/// The longest name a save can have.
const MAX_SAVE_NAME: usize = 80;

/// Rows kept between the cursor and the top or bottom edge when scrolling to
/// follow it. The text can also scroll this far past its last row.
const SCROLL_MARGIN: usize = 3;

#[derive(Clone, Copy)]
enum Cmd {
    Back,
    Save,
    ConfirmSave,
    CancelDialog,
    History,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    Deselect,
    Quit,
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
    /// The text as last stored in the vault.
    stored: String,
    /// The post's most recent save, with its text.
    last_save: Option<Save>,
    buffer: Buffer,
    undo: Undo,
    autosave: Autosave,
    /// The rows on screen for the current text and width.
    rows: Vec<Row>,
    /// `rows` needs rebuilding (the text or the width changed).
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
    /// A passing message (e.g. "Saved"), until the next key or click.
    notice: Option<String>,
    /// Naming a save.
    dialog: Option<Dialog<Cmd>>,
    /// The post's history, shown instead of the post while open.
    history: Option<History>,
    hints: HintBar<Cmd>,
}

impl Editor {
    /// Opens with the cursor where it was left (or at the end of the post).
    pub fn new(post: Post, last_save: Option<Save>) -> Self {
        Editor {
            last_save,
            post_id: post.id,
            buffer: Buffer::new(&post.body, post.cursor),
            stored: post.body,
            undo: Undo::default(),
            autosave: Autosave::default(),
            rows: Vec::new(),
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
            history: None,
            hints: HintBar::default(),
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

    /// The text differs from the most recent save (or there are no saves).
    fn unsaved(&self) -> bool {
        self.last_save.as_ref().is_none_or(|save| *self.buffer.rope() != save.body.as_str())
    }

    /// A save of the post was just made.
    pub fn saved(&mut self, save: Save) {
        self.notice = Some(match save.name.as_str() {
            "" => "Saved.".to_string(),
            name => format!("Saved “{name}”."),
        });
        self.last_save = Some(save);
    }

    /// Something went wrong outside of storing the draft, e.g. saving.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Show the post's history (newest save first).
    pub fn show_history(&mut self, saves: Vec<Save>) {
        let title = title_from_first_line(&self.buffer.rope().line(0).to_string());
        self.history = Some(History::new(title, saves, self.unsaved()));
    }

    /// Open a save (with its text) in the history, to read.
    pub fn show_save(&mut self, save: Save) {
        if let Some(history) = &mut self.history {
            history.read(save);
        }
    }

    /// When the draft should next be stored, if it has unstored edits.
    pub fn autosave_due(&self) -> Option<Instant> {
        self.autosave.due()
    }

    /// The draft in the vault is now `text` (or already was, if `None`).
    pub fn mark_stored(&mut self, text: Option<String>) {
        if let Some(text) = text {
            self.stored = text;
        }
        self.autosave.stored();
        self.error = None;
    }

    /// Storing the draft failed; say so above the hint bar, and try again
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
        let [_, body, status, hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

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
        let lines: Vec<Line> = self
            .rows
            .iter()
            .skip(self.top)
            .take(height)
            .map(|row| row_line(rope, row, selection.as_ref()))
            .collect();
        frame.render_widget(Paragraph::new(lines), self.text_area);
        if (self.top..self.top + height).contains(&cursor_row) {
            let y = body.y + (cursor_row - self.top) as u16;
            frame.set_cursor_position(Position::new(left + cursor_x as u16, y));
        }

        // The status row: a message on the left, whether the post has
        // changed since its last save on the right.
        let state = match &self.last_save {
            _ if self.unsaved() => {
                let what = if self.last_save.is_some() { "Changed since last save" } else { "Never saved" };
                Line::from(vec!["● ".yellow(), what.dim()])
            }
            Some(save) if save.name.is_empty() => Line::from("Saved".dim()),
            Some(save) => Line::from(format!("Saved · {}", truncate(&save.name, width / 3)).dim()),
            None => Line::default(),
        };
        let state_width = state.width();
        let area = Rect { x: left, width: width as u16, ..status };
        frame.render_widget(Paragraph::new(state).right_aligned(), area);
        let room = width.saturating_sub(state_width + 2);
        let message = match (&self.error, &self.notice) {
            (Some(error), _) => Some(truncate(error, room).red()),
            (None, Some(notice)) => Some(truncate(notice, room).dim()),
            (None, None) => None,
        };
        if let Some(message) = message {
            frame.render_widget(Paragraph::new(message), area);
        }

        if let Some(dialog) = &mut self.dialog {
            dialog.render(frame);
            let hints = dialog.hints();
            self.hints.render(frame, hint_area, &hints);
            return;
        }
        if self.buffer.selection().is_some() {
            self.hints.render(
                frame,
                hint_area,
                &[
                    hint("Esc", "deselect", Cmd::Deselect),
                    hint("Ctrl+C", "copy", Cmd::Copy),
                    hint("Ctrl+X", "cut", Cmd::Cut),
                    hint("Ctrl+V", "paste", Cmd::Paste),
                    hint("Ctrl+Z", "undo", Cmd::Undo),
                    hint("Ctrl+Q", "quit", Cmd::Quit),
                ],
            );
            return;
        }
        self.hints.render(
            frame,
            hint_area,
            &[
                hint("Esc", "back to posts", Cmd::Back),
                hint("Ctrl+S", "save", Cmd::Save),
                hint("Ctrl+R", "history", Cmd::History),
                hint("Ctrl+Z", "undo", Cmd::Undo),
                hint("Ctrl+Y", "redo", Cmd::Redo),
                hint("Ctrl+V", "paste", Cmd::Paste),
                hint("Ctrl+Q", "quit", Cmd::Quit),
            ],
        );
    }

    pub fn handle(&mut self, event: Event) -> Action {
        if let Some(history) = &mut self.history {
            return match history.handle(event) {
                Outcome::None => Action::None,
                Outcome::Close => {
                    self.history = None;
                    Action::None
                }
                Outcome::Load(id) => Action::LoadSave(id),
                Outcome::Restore(save) => {
                    self.history = None;
                    self.restore(save);
                    Action::None
                }
                Outcome::Quit => Action::Quit,
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
            KeyCode::Char('z') if ctrl => return self.run(Cmd::Undo),
            KeyCode::Char('y') if ctrl => return self.run(Cmd::Redo),
            KeyCode::Char('s') if ctrl => return self.run(Cmd::Save),
            KeyCode::Char('r') if ctrl => return self.run(Cmd::History),
            KeyCode::Char('c') if ctrl => return self.run(Cmd::Copy),
            KeyCode::Char('x') if ctrl => return self.run(Cmd::Cut),
            KeyCode::Char('v') if ctrl => return self.run(Cmd::Paste),
            KeyCode::Char('a') if ctrl => self.step(Buffer::select_all),
            KeyCode::Char(ch) if !ctrl && !alt => {
                let kind = Kind::Type { space: ch.is_whitespace() };
                self.edit(kind, |b| b.insert(ch.encode_utf8(&mut [0; 4])))
            }
            KeyCode::Enter => self.edit(Kind::Other, |b| b.insert("\n")),
            KeyCode::Tab => self.edit(Kind::Other, |b| b.insert(TAB)),
            KeyCode::Backspace => self.edit(Kind::Backspace, Buffer::backspace),
            KeyCode::Delete => self.edit(Kind::Delete, Buffer::delete),
            KeyCode::Left if ctrl || alt => self.step(Buffer::word_left),
            KeyCode::Right if ctrl || alt => self.step(Buffer::word_right),
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
            Cmd::Save if !self.unsaved() => {
                self.notice = Some("Nothing has changed since the last save.".into());
            }
            Cmd::Save => {
                let dialog = Dialog::new(
                    "Save",
                    vec![Line::from("Name this save, like a commit message.".dim())],
                    vec![
                        Button { label: "Save", key: "", cmd: Cmd::ConfirmSave, danger: false },
                        Button { label: "Cancel", key: "", cmd: Cmd::CancelDialog, danger: false },
                    ],
                    0,
                    1,
                );
                self.dialog = Some(dialog.with_input("e.g. First draft", MAX_SAVE_NAME));
            }
            Cmd::ConfirmSave => {
                if let Some(dialog) = self.dialog.take() {
                    return Action::SavePost { name: dialog.input().trim().to_string() };
                }
            }
            Cmd::CancelDialog => self.dialog = None,
            Cmd::History if self.last_save.is_none() => {
                self.notice = Some("No saves yet. Ctrl+S saves the post.".into());
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
            Cmd::Quit => return Action::Quit,
        }
        Action::None
    }

    /// Replace the text with a save's, as one step that can be undone.
    fn restore(&mut self, save: Save) {
        let before = self.buffer.clone();
        self.buffer = Buffer::new(&save.body, Some(before.cursor()));
        if self.buffer.rope() != before.rope() {
            self.undo.edited(Kind::Other, before, &self.buffer);
            self.changed_text();
        }
        self.notice = Some(match save.name.as_str() {
            "" => "Restored the save. Ctrl+Z undoes it.".to_string(),
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
        let area = self.text_area;
        if row < area.y || row >= area.bottom() {
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
        let index = self.top + row.saturating_sub(self.text_area.y) as usize;
        match self.rows.get(index) {
            Some(&r) => pos_at_x(self.buffer.rope(), r, column.saturating_sub(self.text_area.x) as usize),
            None => self.buffer.len(),
        }
    }

    fn refresh(&mut self) {
        if self.stale {
            self.rows = layout(self.buffer.rope(), self.width);
            self.stale = false;
        }
    }

    /// The cursor's row and column on screen. Just after a space hanging off
    /// the end of a full row, it shows at the start of the next row, where
    /// the next word will go.
    fn cursor_cell(&self) -> (usize, usize) {
        let pos = self.buffer.cursor();
        let row = row_of(&self.rows, pos);
        let x = x_of(self.buffer.rope(), self.rows[row], pos);
        if x > self.width { (row + 1, 0) } else { (row, x) }
    }

    /// The furthest the text can scroll: its last row a margin above the
    /// bottom edge.
    fn max_top(&self) -> usize {
        let height = self.text_area.height as usize;
        self.rows.len().saturating_sub(height.saturating_sub(margin(height)))
    }
}

fn margin(height: usize) -> usize {
    SCROLL_MARGIN.min(height.saturating_sub(1) / 2)
}

/// One row of text, with the selected part reversed. The title is bold, with
/// a dim placeholder while empty.
fn row_line(rope: &Rope, row: &Row, selection: Option<&Range<usize>>) -> Line<'static> {
    let base = if row.line == 0 { Style::new().bold() } else { Style::new() };
    let text = |start, end| rope.slice(start..end).to_string().replace('\t', &" ".repeat(TAB_WIDTH));
    let selected = selection
        .map(|s| s.start.max(row.start)..s.end.min(row.end))
        .filter(|s| s.start < s.end);
    let mut spans = Vec::new();
    match selected {
        None => spans.push(Span::styled(text(row.start, row.end), base)),
        Some(s) => {
            spans.push(Span::styled(text(row.start, s.start), base));
            spans.push(Span::styled(text(s.start, s.end), base.reversed()));
            spans.push(Span::styled(text(s.end, row.end), base));
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
            let editor = Editor::new(Post { id: 1, body: body.into(), cursor }, None);
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

        fn screen_row(&self, y: u16) -> String {
            let buffer = self.terminal.backend().buffer();
            (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>().trim_end().to_string()
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
        // Scrolled up, keeping a margin above the cursor.
        assert_eq!(h.screen_row(4), format!("{}line 20", " ".repeat(6)));
        assert_eq!(h.cursor(), (LEFT, 4));
        // The wheel scrolls without moving the cursor.
        h.mouse(MouseEventKind::ScrollUp, 40, 5);
        assert_eq!(h.cursor(), (LEFT, 5));
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
        // The hint bar is on the last row; its hints can be clicked.
        let hints = h.screen_row(11);
        let redo = hints.find("Ctrl+Y").unwrap() as u16;
        h.mouse(MouseEventKind::Down(MouseButton::Left), redo, 11);
        assert_eq!(h.editor.text(), "# Title\nHello world");
        let undo = hints.find("Ctrl+Z").unwrap() as u16;
        h.mouse(MouseEventKind::Down(MouseButton::Left), undo + 2, 11);
        assert_eq!(h.editor.text(), "# Title\nHello ");
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
    fn select_all_cut_and_paste() {
        let mut h = Harness::new("# Title\nHello world");
        h.ctrl('a');
        assert!(h.screen_row(11).contains("Ctrl+C"));
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

    fn save(id: i64, name: &str, body: &str) -> Save {
        Save { id, name: name.into(), created_at: jiff::Timestamp::now(), body: body.into() }
    }

    #[test]
    fn ctrl_s_names_a_save() {
        let mut h = Harness::new("# Title\nSome text");
        assert!(h.screen_row(10).ends_with("● Never saved"));
        h.ctrl('s');
        assert!(h.screen().contains("Name this save"));
        h.typed("First draft");
        let action = h.press(KeyCode::Enter);
        assert!(matches!(action, Action::SavePost { ref name } if name == "First draft"));
        // The app records it and tells the editor.
        h.editor.saved(save(1, "First draft", "# Title\nSome text"));
        h.draw();
        assert!(h.screen_row(10).ends_with("Saved · First draft"));
        assert!(h.screen_row(10).contains("Saved “First draft”."));
        // Nothing new to save.
        h.ctrl('s');
        assert!(h.screen_row(10).contains("Nothing has changed since the last save."));
        h.typed("!");
        assert!(h.screen_row(10).ends_with("● Changed since last save"));
        // Esc cancels naming a save.
        h.ctrl('s');
        assert!(matches!(h.press(KeyCode::Esc), Action::None));
        assert!(!h.screen().contains("Name this save"));
    }

    #[test]
    fn history_restores_a_save_and_undo_takes_it_back() {
        let first = save(1, "First draft", "# Title\nOld text");
        let editor = Editor::new(Post { id: 1, body: "# Title\nNew text".into(), cursor: None }, Some(first.clone()));
        let mut h = Harness { editor, terminal: Terminal::new(TestBackend::new(80, 12)).unwrap() };
        h.draw();
        assert!(matches!(h.ctrl('r'), Action::ShowHistory));
        h.editor.show_history(vec![first.clone()]);
        h.draw();
        assert!(h.screen().contains("Saves of “Title”"));
        assert!(h.screen().contains("First draft"));
        assert!(matches!(h.press(KeyCode::Enter), Action::LoadSave(1)));
        h.editor.show_save(first);
        h.draw();
        assert!(h.screen().contains("Old text"));
        h.press(KeyCode::Enter);
        assert!(h.screen().contains("Replace the draft with “First draft”?"));
        assert!(h.screen().contains("The draft has changes since the last save."));
        h.press(KeyCode::Char('y'));
        assert_eq!(h.editor.text(), "# Title\nOld text");
        assert!(h.screen_row(10).contains("Restored “First draft”."));
        h.ctrl('z');
        assert_eq!(h.editor.text(), "# Title\nNew text");
    }

    #[test]
    fn history_needs_a_save() {
        let mut h = Harness::new("# Title");
        assert!(matches!(h.ctrl('r'), Action::None));
        assert!(h.screen_row(10).contains("No saves yet."));
    }
}
