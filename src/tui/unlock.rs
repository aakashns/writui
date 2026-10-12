//! Every later run: enter the password to unlock the vault.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use super::Action;
use super::hints::{HintBar, hint};
use super::widgets::{
    COLUMN_WIDTH, PASSWORD_BOX, PasswordField, centered, dev_password_note, overview, title,
};

#[derive(Clone, Copy)]
enum Cmd {
    Unlock,
    Clear,
    Quit,
}

#[derive(Default)]
pub struct Unlock {
    field: PasswordField,
    /// writui was locked (Ctrl+O), rather than just started.
    locked: bool,
    error: Option<&'static str>,
    busy: bool,
    hints: HintBar<Cmd>,
}

impl Unlock {
    pub fn locked() -> Self {
        Unlock { locked: true, ..Unlock::default() }
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    pub fn wrong_password(&mut self) {
        self.busy = false;
        self.field.clear();
        self.error = Some("Wrong password.");
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [body, hint_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

        let status = if self.busy {
            Line::from("Unlocking…".dim())
        } else if let Some(error) = self.error {
            Line::from(error.red())
        } else if self.locked {
            Line::from("Locked. Unlock to pick up where you were.".dim())
        } else {
            Line::default()
        };
        // The password box is drawn over the second to fourth lines.
        let mut rest = vec![
            Line::default(),
            Line::default(),
            Line::default(),
            Line::default(),
            Line::default(),
            status,
            Line::default(),
        ];
        rest.extend(overview());
        if let Some(note) = dev_password_note() {
            rest.extend([Line::default(), Line::from(note.dim())]);
        }
        let mut lines = title(body, rest.len());
        let field_line = lines.len() as u16 + 1;
        lines.extend(rest);
        let area = centered(body, COLUMN_WIDTH, lines.len() as u16);
        frame.render_widget(Paragraph::new(lines), area);
        let (width, height) = PASSWORD_BOX;
        let field_area = Rect::new(area.x, area.y + field_line, width, height).intersection(area);
        let cursor = self.field.render_boxed(frame, field_area, "Password", true);
        if !self.busy {
            frame.set_cursor_position(cursor);
        }

        // Esc clears the password, or quits if there's nothing to clear.
        let mut hints = vec![hint("Enter", "unlock", Cmd::Unlock)];
        if self.field.is_empty() {
            hints.push(hint("Esc", "quit", Cmd::Quit));
        } else {
            hints.extend([hint("Esc", "clear", Cmd::Clear), hint("Ctrl+Q", "quit", Cmd::Quit)]);
        }
        self.hints.render(frame, hint_area, &hints);
    }

    pub fn handle(&mut self, event: Event) -> Action {
        if self.busy {
            return Action::None;
        }
        match event {
            Event::Key(key) if key.code == KeyCode::Enter => self.run(Cmd::Unlock),
            Event::Key(key) if key.code == KeyCode::Esc => {
                self.run(if self.field.is_empty() { Cmd::Quit } else { Cmd::Clear })
            }
            Event::Key(key) => {
                if self.field.handle_key(key) {
                    self.error = None;
                }
                Action::None
            }
            Event::Paste(text) => {
                self.field.paste(&text);
                Action::None
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                match self.hints.hit(mouse.column, mouse.row) {
                    Some(cmd) => self.run(cmd),
                    None => Action::None,
                }
            }
            _ => Action::None,
        }
    }

    fn run(&mut self, cmd: Cmd) -> Action {
        match cmd {
            Cmd::Quit => Action::Quit,
            Cmd::Clear => {
                self.field.clear();
                self.error = None;
                Action::None
            }
            Cmd::Unlock if self.field.is_empty() => Action::None,
            Cmd::Unlock => Action::Unlock(self.field.value().clone()),
        }
    }
}
