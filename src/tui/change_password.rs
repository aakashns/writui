//! The "Change password" dialog, opened from Settings: three boxed
//! password fields (the current password, the new one, and the new one
//! again). Tab moves between them, Enter goes on to the next empty one or
//! changes the password, Esc cancels, and the fields and buttons can be
//! clicked.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use zeroize::Zeroizing;

use super::hints::{Hint, hint};
use super::widgets::{MIN_PASSWORD_LEN, PasswordField, centered};

const WIDTH: u16 = 56;
const LABELS: [&str; 3] = ["Current password", "New password", "Confirm new password"];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cmd {
    Submit,
    NextField,
    Cancel,
}

/// What the dialog asks for.
pub enum Outcome {
    /// Change the password from `current` to `new`.
    Change {
        current: Zeroizing<String>,
        new: Zeroizing<String>,
    },
    Cancel,
}

#[derive(Default)]
pub struct ChangePassword {
    fields: [PasswordField; 3],
    focus: usize,
    error: Option<&'static str>,
    /// Changing the password: input is ignored until it's done.
    busy: bool,
    field_areas: [Rect; 3],
    /// Where the buttons were drawn, for clicks.
    buttons: Vec<(Rect, Cmd)>,
}

impl ChangePassword {
    pub fn set_busy(&mut self) {
        self.busy = true;
        self.error = None;
    }

    /// The current password typed wasn't the vault's.
    pub fn wrong_password(&mut self) {
        self.busy = false;
        self.error = Some("That's not your current password.");
        self.fields[0].clear();
        self.focus = 0;
    }

    /// Hints for the hint bar while the dialog is open.
    pub fn hints(&self) -> Vec<Hint<Cmd>> {
        if self.busy {
            return Vec::new();
        }
        vec![
            hint("Enter", "change password", Cmd::Submit),
            hint("Tab", "next field", Cmd::NextField),
            hint("Esc", "cancel", Cmd::Cancel),
        ]
    }

    pub fn render(&mut self, frame: &mut Frame) {
        // A line, a blank, three boxes, the status, a blank, the buttons.
        let (_, box_height) = super::widgets::PASSWORD_BOX;
        let height = 2 + 3 * box_height + 3 + 2;
        let area = centered(frame.area(), WIDTH, height);
        let block = Block::bordered().title(" Change password ");
        let inner = block.inner(area);
        let inner = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let note = "Forget it and the writing is gone: no recovery.";
        frame.render_widget(Paragraph::new(note.dim()), Rect { height: 1, ..inner });
        let mut y = inner.y + 2;
        for (i, label) in LABELS.into_iter().enumerate() {
            let field_area = Rect::new(inner.x, y, inner.width, box_height).intersection(inner);
            self.field_areas[i] = field_area;
            let focused = i == self.focus;
            let cursor = self.fields[i].render_boxed(frame, field_area, label, focused);
            if focused && !self.busy {
                frame.set_cursor_position(cursor);
            }
            y += box_height;
        }
        let status = if self.busy {
            Line::from("Changing the password…".dim())
        } else if let Some(error) = self.error {
            Line::from(error.red())
        } else {
            Line::default()
        };
        let status_area = Rect { y, height: 1, ..inner }.intersection(inner);
        frame.render_widget(Paragraph::new(status), status_area);

        // The buttons: Enter presses the first.
        self.buttons.clear();
        let mut spans = Vec::new();
        let mut x = inner.x;
        let buttons_y = y + 2;
        for (i, (label, cmd)) in
            [("Change password", Cmd::Submit), ("Cancel", Cmd::Cancel)].into_iter().enumerate()
        {
            if i > 0 {
                spans.push(Span::raw("  "));
                x += 2;
            }
            let text = format!("[ {label} ]");
            let width = text.chars().count() as u16;
            let style = if i == 0 { Style::new().reversed().bold() } else { Style::new() };
            spans.push(Span::styled(text, style));
            self.buttons.push((Rect::new(x, buttons_y, width, 1), cmd));
            x += width;
        }
        let buttons_area = Rect { y: buttons_y, height: 1, ..inner }.intersection(inner);
        frame.render_widget(Paragraph::new(Line::from(spans)), buttons_area);
    }

    /// Handle an event; `None` while the dialog stays open as it is.
    pub fn handle(&mut self, event: Event) -> Option<Outcome> {
        if self.busy {
            return None;
        }
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Enter => {
                    // On to the next field, while there's one left to fill.
                    match (self.focus + 1..3).find(|&i| self.fields[i].is_empty()) {
                        Some(next) if !self.fields[self.focus].is_empty() => {
                            self.focus = next;
                            None
                        }
                        _ => self.run(Cmd::Submit),
                    }
                }
                KeyCode::Esc => self.run(Cmd::Cancel),
                KeyCode::Tab | KeyCode::Down => self.run(Cmd::NextField),
                KeyCode::BackTab | KeyCode::Up => {
                    self.focus = (self.focus + 2) % 3;
                    None
                }
                _ => {
                    if self.fields[self.focus].handle_key(key) {
                        self.error = None;
                    }
                    None
                }
            },
            Event::Paste(text) => {
                self.fields[self.focus].paste(&text);
                None
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                let pos = Position::new(mouse.column, mouse.row);
                if let Some(i) = self.field_areas.iter().position(|a| a.contains(pos)) {
                    self.focus = i;
                    return None;
                }
                let &(_, cmd) = self.buttons.iter().find(|(rect, _)| rect.contains(pos))?;
                self.run(cmd)
            }
            _ => None,
        }
    }

    /// Carry out a command from the dialog or the hint bar.
    pub fn run(&mut self, cmd: Cmd) -> Option<Outcome> {
        if self.busy {
            return None;
        }
        match cmd {
            Cmd::Cancel => Some(Outcome::Cancel),
            Cmd::NextField => {
                self.focus = (self.focus + 1) % 3;
                None
            }
            Cmd::Submit => {
                let [current, new, confirm] = &mut self.fields;
                let (error, focus) = if current.is_empty() {
                    ("Type your current password.", 0)
                } else if new.len() < MIN_PASSWORD_LEN {
                    ("Use at least 8 characters for the new password.", 1)
                } else if new.value() != confirm.value() {
                    confirm.clear();
                    ("The new passwords don't match. Try confirming again.", 2)
                } else {
                    return Some(Outcome::Change {
                        current: current.value().clone(),
                        new: new.value().clone(),
                    });
                };
                self.error = Some(error);
                self.focus = focus;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers, MouseEvent};

    use super::*;

    fn screen(dialog: &mut ChangePassword) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| dialog.render(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..24)
            .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn press(dialog: &mut ChangePassword, code: KeyCode) -> Option<Outcome> {
        dialog.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
    }

    fn type_text(dialog: &mut ChangePassword, text: &str) {
        for ch in text.chars() {
            press(dialog, KeyCode::Char(ch));
        }
    }

    #[test]
    fn three_boxes_with_enter_going_on_to_the_next() {
        let mut dialog = ChangePassword::default();
        let text = screen(&mut dialog);
        for label in LABELS {
            assert!(text.contains(label), "{text}");
        }
        assert!(text.contains("[ Change password ]") && text.contains("[ Cancel ]"), "{text}");
        type_text(&mut dialog, "old password");
        assert!(press(&mut dialog, KeyCode::Enter).is_none());
        type_text(&mut dialog, "new password");
        assert!(press(&mut dialog, KeyCode::Enter).is_none());
        assert_eq!(dialog.focus, 2);
        type_text(&mut dialog, "new password");
        let Some(Outcome::Change { current, new }) = press(&mut dialog, KeyCode::Enter) else {
            panic!("not changed")
        };
        assert_eq!((current.as_str(), new.as_str()), ("old password", "new password"));
        assert!(screen(&mut dialog).contains("••••••••••••"));
        dialog.set_busy();
        assert!(screen(&mut dialog).contains("Changing the password…"));
        assert!(press(&mut dialog, KeyCode::Esc).is_none());
    }

    #[test]
    fn mistakes_are_said_in_the_dialog() {
        let mut dialog = ChangePassword::default();
        press(&mut dialog, KeyCode::Tab);
        type_text(&mut dialog, "new password");
        assert!(press(&mut dialog, KeyCode::Enter).is_none());
        // Enter with the confirmation empty goes on to it, and then says
        // what's missing.
        assert_eq!(dialog.focus, 2);
        press(&mut dialog, KeyCode::Enter);
        assert!(screen(&mut dialog).contains("Type your current password."));
        assert_eq!(dialog.focus, 0);
        type_text(&mut dialog, "old password");
        press(&mut dialog, KeyCode::Tab);
        press(&mut dialog, KeyCode::Tab);
        type_text(&mut dialog, "new passwork");
        assert!(press(&mut dialog, KeyCode::Enter).is_none());
        assert!(screen(&mut dialog).contains("The new passwords don't match."));
        assert!(dialog.fields[2].is_empty() && dialog.focus == 2);
        // Too short.
        press(&mut dialog, KeyCode::BackTab);
        for _ in 0..12 {
            press(&mut dialog, KeyCode::Backspace);
        }
        type_text(&mut dialog, "short");
        assert!(dialog.run(Cmd::Submit).is_none());
        assert!(screen(&mut dialog).contains("Use at least 8 characters"));
        // A wrong current password, once tried.
        dialog.set_busy();
        dialog.wrong_password();
        assert!(screen(&mut dialog).contains("That's not your current password."));
        assert!(dialog.fields[0].is_empty() && dialog.focus == 0);
        assert!(matches!(press(&mut dialog, KeyCode::Esc), Some(Outcome::Cancel)));
    }

    #[test]
    fn fields_and_buttons_can_be_clicked() {
        let mut dialog = ChangePassword::default();
        screen(&mut dialog);
        let click = |dialog: &mut ChangePassword, rect: Rect| {
            dialog.handle(Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: rect.x + 1,
                row: rect.y + 1.min(rect.height - 1),
                modifiers: KeyModifiers::NONE,
            }))
        };
        let third = dialog.field_areas[2];
        assert!(click(&mut dialog, third).is_none());
        assert_eq!(dialog.focus, 2);
        let (submit, cancel) = (dialog.buttons[0].0, dialog.buttons[1].0);
        assert!(click(&mut dialog, submit).is_none());
        assert!(screen(&mut dialog).contains("Type your current password."));
        assert!(matches!(click(&mut dialog, cancel), Some(Outcome::Cancel)));
    }
}
