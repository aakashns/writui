//! Every later run: enter the password to unlock the vault.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::Action;
use super::hints::{HintBar, hint};
use super::widgets::{COLUMN_WIDTH, PasswordField, centered};

const LABEL: &str = "Password  ";

#[derive(Clone, Copy)]
enum Cmd {
    Unlock,
    Quit,
}

#[derive(Default)]
pub struct Unlock {
    field: PasswordField,
    error: Option<&'static str>,
    busy: bool,
    hints: HintBar<Cmd>,
}

impl Unlock {
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
        } else {
            Line::default()
        };
        let lines = vec![
            Line::from("writui".bold()),
            Line::default(),
            Line::from(vec![Span::raw(LABEL).bold(), Span::raw(self.field.masked())]),
            Line::default(),
            status,
        ];
        let area = centered(body, COLUMN_WIDTH, lines.len() as u16);
        if !self.busy {
            let x = area.x + (LABEL.len() + self.field.len()) as u16;
            frame.set_cursor_position(Position::new(x.min(area.right() - 1), area.y + 2));
        }
        frame.render_widget(Paragraph::new(lines), area);

        self.hints.render(
            frame,
            hint_area,
            &[hint("Enter", "unlock", Cmd::Unlock), hint("Ctrl+Q", "quit", Cmd::Quit)],
        );
    }

    pub fn handle(&mut self, event: Event) -> Action {
        if self.busy {
            return Action::None;
        }
        match event {
            Event::Key(key) if key.code == KeyCode::Enter => self.run(Cmd::Unlock),
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
            Cmd::Unlock if self.field.is_empty() => Action::None,
            Cmd::Unlock => Action::Unlock(self.field.value().clone()),
        }
    }
}
