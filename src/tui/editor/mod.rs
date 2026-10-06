//! The post editing screen: the text in a centred column, soft-wrapped, with
//! the cursor, scrolling and the mouse.

mod buffer;
mod wrap;

use ratatui::Frame;
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ropey::Rope;

use super::Action;
use super::hints::{HintBar, hint};
use super::widgets::{COLUMN_WIDTH, truncate};
use crate::vault::{Post, TITLE_PREFIX};
use buffer::{Buffer, MIN};
use wrap::{Row, TAB_WIDTH, layout, pos_at_x, row_of, x_of};

/// What the Tab key types.
const TAB: &str = "  ";

/// Rows kept between the cursor and the top or bottom edge when scrolling to
/// follow it. The text can also scroll this far past its last row.
const SCROLL_MARGIN: usize = 3;

#[derive(Clone, Copy)]
enum Cmd {
    Back,
    Quit,
}

pub struct Editor {
    post_id: i64,
    /// The text as last stored in the vault.
    stored: String,
    buffer: Buffer,
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
    error: Option<String>,
    hints: HintBar<Cmd>,
}

impl Editor {
    /// Opens with the cursor where it was left (or at the end of the post).
    pub fn new(post: Post) -> Self {
        Editor {
            post_id: post.id,
            buffer: Buffer::new(&post.body, post.cursor),
            stored: post.body,
            rows: Vec::new(),
            stale: true,
            width: COLUMN_WIDTH as usize,
            text_area: Rect::default(),
            top: 0,
            follow: true,
            opening: true,
            goal: None,
            error: None,
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

    pub fn mark_stored(&mut self, text: String) {
        self.stored = text;
    }

    /// Shown above the hint bar (e.g. the draft couldn't be stored).
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    pub fn render(&mut self, frame: &mut Frame) {
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
        let lines: Vec<Line> =
            self.rows.iter().skip(self.top).take(height).map(|row| row_line(rope, row)).collect();
        frame.render_widget(Paragraph::new(lines), self.text_area);
        if (self.top..self.top + height).contains(&cursor_row) {
            let y = body.y + (cursor_row - self.top) as u16;
            frame.set_cursor_position(Position::new(left + cursor_x as u16, y));
        }

        if let Some(error) = &self.error {
            let area = Rect { x: left, width: width as u16, ..status };
            frame.render_widget(Paragraph::new(truncate(error, width).red()), area);
        }
        self.hints.render(
            frame,
            hint_area,
            &[hint("Esc", "back to posts", Cmd::Back), hint("Ctrl+Q", "quit", Cmd::Quit)],
        );
    }

    pub fn handle(&mut self, event: Event) -> Action {
        match event {
            Event::Key(key) => return self.key(key),
            Event::Mouse(mouse) => return self.mouse(mouse),
            Event::Paste(text) => self.edit(|b| b.insert(&text)),
            _ => {}
        }
        Action::None
    }

    fn key(&mut self, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc => return self.run(Cmd::Back),
            KeyCode::Char(ch) if !ctrl && !alt => {
                self.edit(|b| b.insert(ch.encode_utf8(&mut [0; 4])))
            }
            KeyCode::Enter => self.edit(|b| b.insert("\n")),
            KeyCode::Tab => self.edit(|b| b.insert(TAB)),
            KeyCode::Backspace => self.edit(Buffer::backspace),
            KeyCode::Delete => self.edit(Buffer::delete),
            KeyCode::Left => self.step(Buffer::left),
            KeyCode::Right => self.step(Buffer::right),
            KeyCode::Up => self.move_rows(-1),
            KeyCode::Down => self.move_rows(1),
            KeyCode::PageUp => self.page(-1),
            KeyCode::PageDown => self.page(1),
            KeyCode::Home if ctrl => self.step(|b| b.set_cursor(MIN)),
            KeyCode::End if ctrl => self.step(|b| b.set_cursor(b.len())),
            KeyCode::Home => self.row_start(),
            KeyCode::End => self.row_end(),
            _ => {}
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
            _ => {}
        }
        Action::None
    }

    fn run(&mut self, cmd: Cmd) -> Action {
        match cmd {
            Cmd::Back => Action::BackToList(Some(self.post_id)),
            Cmd::Quit => Action::Quit,
        }
    }

    /// Change the text, then re-wrap and scroll to the cursor.
    fn edit(&mut self, f: impl FnOnce(&mut Buffer)) {
        f(&mut self.buffer);
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
            None if by < 0 => MIN,
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
    /// start or end; below the text, the very end.
    fn click(&mut self, column: u16, row: u16) {
        let area = self.text_area;
        if row < area.y || row >= area.bottom() {
            return;
        }
        self.refresh();
        let index = self.top + (row - area.y) as usize;
        let pos = match self.rows.get(index) {
            Some(&r) => pos_at_x(self.buffer.rope(), r, column.saturating_sub(area.x) as usize),
            None => self.buffer.len(),
        };
        self.buffer.set_cursor(pos);
        self.goal = None;
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

/// One row of text. The title is bold, with a dim placeholder while empty.
fn row_line(rope: &Rope, row: &Row) -> Line<'static> {
    let text = rope.slice(row.start..row.end).to_string().replace('\t', &" ".repeat(TAB_WIDTH));
    if row.line > 0 {
        Line::from(text)
    } else if text == TITLE_PREFIX && row.last {
        Line::from(vec![text.bold(), "Title".dim()])
    } else {
        Line::from(text.bold())
    }
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
            let editor = Editor::new(Post { id: 1, body: body.into(), cursor });
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
}
