//! First run: create the vault and choose its password.

use std::path::PathBuf;

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use super::Action;
use super::hints::{HintBar, hint};
use super::widgets::{
    COLUMN_WIDTH, PASSWORD_BOX, PasswordField, centered, dev_password_note, overview, title,
    truncate,
};

const MIN_PASSWORD_LEN: usize = 8;
const LABELS: [&str; 2] = ["Password", "Confirm password"];

#[derive(Clone, Copy)]
enum Cmd {
    Submit,
    SwitchField,
    Clear,
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
        let mut rest = vec![
            Line::default(),
            Line::from("Create your vault"),
            Line::default(),
            Line::from("Everything you write in writui is encrypted with this password."),
            Line::from("There is no way to recover it: if you forget the password, your"),
            Line::from("writing is gone for good."),
            Line::default(),
            // The two password boxes go here, side by side.
            Line::default(),
            Line::default(),
            Line::default(),
            Line::default(),
            status,
            Line::from(truncate(&path, COLUMN_WIDTH as usize).dim()),
            Line::default(),
        ];
        rest.extend(overview());
        if let Some(note) = dev_password_note() {
            rest.extend([Line::default(), Line::from(note.dim())]);
        }
        let mut lines = title(body, rest.len());
        let first_field_line = lines.len() as u16 + 7;
        lines.extend(rest);

        let area = centered(body, COLUMN_WIDTH, lines.len() as u16);
        frame.render_widget(Paragraph::new(lines), area);
        // Narrow windows get narrower boxes, still side by side.
        let (width, height) = PASSWORD_BOX;
        let width = width.min(area.width.saturating_sub(2) / 2);
        for i in 0..2 {
            let x = area.x + i as u16 * (area.width - width);
            let field_area = Rect::new(x, area.y + first_field_line, width, height);
            self.field_areas[i] = field_area.intersection(area);
            let focused = i == self.focus;
            let cursor =
                self.fields[i].render_boxed(frame, self.field_areas[i], LABELS[i], focused);
            if focused && !self.busy {
                frame.set_cursor_position(cursor);
            }
        }

        // Esc clears both passwords, or quits if there's nothing to clear.
        let mut hints = vec![
            hint("Enter", "create vault", Cmd::Submit),
            hint("Tab", "next field", Cmd::SwitchField),
        ];
        if self.fields.iter().all(PasswordField::is_empty) {
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
            Event::Key(key) => match key.code {
                KeyCode::Enter if self.focus == 0 && self.fields[1].is_empty() => {
                    self.focus = 1;
                    Action::None
                }
                KeyCode::Enter => self.run(Cmd::Submit),
                KeyCode::Esc if self.fields.iter().all(PasswordField::is_empty) => {
                    self.run(Cmd::Quit)
                }
                KeyCode::Esc => self.run(Cmd::Clear),
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
            Cmd::Clear => {
                self.fields.iter_mut().for_each(PasswordField::clear);
                self.focus = 0;
                self.error = None;
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
