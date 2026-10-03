//! First run: create the vault and choose its password.

use std::path::PathBuf;

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::Action;
use super::hints::{HintBar, hint};
use super::widgets::{COLUMN_WIDTH, PasswordField, centered, dev_password_note, truncate};

const MIN_PASSWORD_LEN: usize = 8;
const LABEL_WIDTH: usize = 18;

#[derive(Clone, Copy)]
enum Cmd {
    Submit,
    SwitchField,
    Quit,
}

pub struct Setup {
    vault_path: PathBuf,
    fields: [PasswordField; 2],
    focus: usize,
    error: Option<&'static str>,
    busy: bool,
    field_areas: [Rect; 2],
    hints: HintBar<Cmd>,
}

impl Setup {
    pub fn new(vault_path: PathBuf) -> Self {
        Setup {
            vault_path,
            fields: Default::default(),
            focus: 0,
            error: None,
            busy: false,
            field_areas: Default::default(),
            hints: HintBar::default(),
        }
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [body, hint_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

        let status = if self.busy {
            Line::from("Creating your vault…".dim())
        } else if let Some(error) = self.error {
            Line::from(error.red())
        } else {
            Line::default()
        };
        let path = format!("Vault: {}", self.vault_path.display());
        let lines = vec![
            Line::from("writui".bold()),
            Line::default(),
            Line::from("Create your vault"),
            Line::default(),
            Line::from("Everything you write in writui is encrypted with this password."),
            Line::from("There is no way to recover it: if you forget the password, your"),
            Line::from("writing is gone for good."),
            Line::default(),
            self.field_line(0, "Password"),
            self.field_line(1, "Confirm password"),
            Line::default(),
            status,
            Line::default(),
            Line::from(truncate(&path, COLUMN_WIDTH as usize).dim()),
            Line::default(),
            Line::from(dev_password_note().unwrap_or_default().dim()),
        ];
        const FIRST_FIELD_LINE: u16 = 8;

        let area = centered(body, COLUMN_WIDTH, lines.len() as u16);
        for i in 0..2 {
            self.field_areas[i] =
                Rect::new(area.x, area.y + FIRST_FIELD_LINE + i as u16, area.width, 1);
        }
        if !self.busy {
            let field = &self.field_areas[self.focus];
            let x = field.x + (LABEL_WIDTH + self.fields[self.focus].len()) as u16;
            frame.set_cursor_position(Position::new(x.min(field.right() - 1), field.y));
        }
        frame.render_widget(Paragraph::new(lines), area);

        self.hints.render(
            frame,
            hint_area,
            &[
                hint("Enter", "create vault", Cmd::Submit),
                hint("Tab", "next field", Cmd::SwitchField),
                hint("Ctrl+Q", "quit", Cmd::Quit),
            ],
        );
    }

    fn field_line(&self, index: usize, label: &str) -> Line<'static> {
        let label = format!("{label:<LABEL_WIDTH$}");
        let label = if index == self.focus { Span::raw(label).bold() } else { Span::raw(label).dim() };
        Line::from(vec![label, Span::raw(self.fields[index].masked())])
    }

    pub fn handle(&mut self, event: Event) -> Action {
        if self.busy {
            return Action::None;
        }
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Enter if self.focus == 0 && self.fields[1].is_empty() => {
                    self.focus = 1;
                    Action::None
                }
                KeyCode::Enter => self.run(Cmd::Submit),
                KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
                    self.run(Cmd::SwitchField)
                }
                _ => {
                    if self.fields[self.focus].handle_key(key) {
                        self.error = None;
                    }
                    Action::None
                }
            },
            Event::Paste(text) => {
                self.fields[self.focus].paste(&text);
                Action::None
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                let pos = Position::new(mouse.column, mouse.row);
                if let Some(i) = self.field_areas.iter().position(|a| a.contains(pos)) {
                    self.focus = i;
                }
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
            Cmd::SwitchField => {
                self.focus = 1 - self.focus;
                Action::None
            }
            Cmd::Submit => {
                let [password, confirm] = &mut self.fields;
                if password.len() < MIN_PASSWORD_LEN {
                    self.error = Some("Use at least 8 characters.");
                    self.focus = 0;
                    Action::None
                } else if password.value() != confirm.value() {
                    self.error = Some("The passwords don't match. Try confirming again.");
                    confirm.clear();
                    self.focus = 1;
                    Action::None
                } else {
                    Action::CreateVault(password.value().clone())
                }
            }
        }
    }
}
