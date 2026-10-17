//! Every later run: enter the password to unlock the vault, or use
//! Touch ID where it's on.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

use super::Action;
use super::hints::{HintBar, hint};
use super::widgets::{
    COLUMN_WIDTH, PASSWORD_BOX, PasswordField, centered, dev_password_note, overview, title,
    truncate,
};

/// Whether this vault can be unlocked with Touch ID here.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum TouchId {
    /// Not on this computer (or not right now, e.g. over SSH).
    #[default]
    Unavailable,
    /// This Mac has it, but it's not turned on for this vault.
    Off,
    On,
}

#[derive(Clone, Copy)]
enum Cmd {
    Unlock,
    TouchId,
    Clear,
    Quit,
}

#[derive(Default)]
pub struct Unlock {
    field: PasswordField,
    /// writui was locked (Ctrl+O), rather than just started.
    locked: bool,
    touch_id: TouchId,
    error: Option<String>,
    notice: Option<&'static str>,
    busy: bool,
    /// Busy waiting for a fingerprint, rather than checking the password.
    waiting: bool,
    /// Where the Unlock button was drawn, for clicks.
    button: Rect,
    hints: HintBar<Cmd>,
}

impl Unlock {
    pub fn new(touch_id: TouchId) -> Self {
        Unlock { touch_id, ..Unlock::default() }
    }

    pub fn locked(touch_id: TouchId) -> Self {
        Unlock { locked: true, touch_id, ..Unlock::default() }
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    /// The Touch ID prompt is up.
    pub fn wait_for_touch_id(&mut self) {
        self.busy = true;
        self.waiting = true;
        self.error = None;
        self.notice = None;
    }

    pub fn touch_id_cancelled(&mut self) {
        self.busy = false;
        self.waiting = false;
        self.notice = Some("Touch ID cancelled. Enter tries again, or type your password.");
    }

    pub fn touch_id_failed(&mut self, why: &str) {
        self.busy = false;
        self.waiting = false;
        self.error = Some(format!("Touch ID didn't work: {why}"));
    }

    pub fn wrong_password(&mut self) {
        self.busy = false;
        self.field.clear();
        self.notice = None;
        self.error = Some("Wrong password.".into());
    }

    /// Enter (or the button) asks for a fingerprint, rather than trying
    /// the password.
    fn uses_touch_id(&self) -> bool {
        self.touch_id == TouchId::On && self.field.is_empty()
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [body, hint_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

        let status = if self.busy && self.waiting {
            Line::from("Waiting for Touch ID…".dim())
        } else if self.busy {
            Line::from("Unlocking…".dim())
        } else if let Some(error) = &self.error {
            Line::from(truncate(error, COLUMN_WIDTH as usize).red())
        } else if let Some(notice) = self.notice {
            Line::from(notice.dim())
        } else if self.locked && self.uses_touch_id() {
            Line::from("Locked. Press Enter to unlock with Touch ID.".dim())
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
        if self.touch_id == TouchId::Off {
            let tip = "· On this Mac, Settings can turn on unlocking with Touch ID.";
            rest.push(Line::from(tip.dim()));
        }
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

        // The Unlock button, right of the password box and as tall as it,
        // left out if the window is too narrow for all of it. Dimmed while
        // there's no password for it to try, unless it's for Touch ID.
        let button = Rect::new(field_area.right() + 2, field_area.y, 12, height);
        self.button = if area.contains(Position::new(button.right() - 1, button.bottom() - 1)) {
            button
        } else {
            Rect::default()
        };
        if !self.button.is_empty() {
            let style = if self.busy || (self.field.is_empty() && !self.uses_touch_id()) {
                Style::new().dim()
            } else {
                Style::new().bold()
            };
            let label = if self.uses_touch_id() { "Touch ID" } else { "Unlock" };
            let block = Block::bordered().border_style(style);
            let label = Paragraph::new(Line::from(label).style(style)).centered();
            frame.render_widget(label, block.inner(button));
            frame.render_widget(block, button);
        }

        // Esc clears the password, or quits if there's nothing to clear.
        let mut hints = vec![match self.uses_touch_id() {
            true => hint("Enter", "Touch ID", Cmd::TouchId),
            false => hint("Enter", "unlock", Cmd::Unlock),
        }];
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
                    self.notice = None;
                }
                Action::None
            }
            Event::Paste(text) => {
                self.field.paste(&text);
                Action::None
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                if self.button.contains(Position::new(mouse.column, mouse.row)) {
                    return self.run(Cmd::Unlock);
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
            Cmd::Clear => {
                self.field.clear();
                self.error = None;
                Action::None
            }
            Cmd::Unlock | Cmd::TouchId if self.uses_touch_id() => Action::UnlockTouchId,
            Cmd::TouchId => Action::None,
            Cmd::Unlock if self.field.is_empty() => Action::None,
            Cmd::Unlock => Action::Unlock(self.field.value().clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

    use super::*;

    fn screen(unlock: &mut Unlock) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        terminal.draw(|frame| unlock.render(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..40)
            .map(|y| (0..100).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn press(unlock: &mut Unlock, code: KeyCode) -> Action {
        unlock.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
    }

    #[test]
    fn enter_uses_touch_id_until_a_password_is_typed() {
        let mut unlock = Unlock::locked(TouchId::On);
        let text = screen(&mut unlock);
        assert!(text.contains("Touch ID") && text.contains("Enter Touch ID"), "{text}");
        assert!(matches!(press(&mut unlock, KeyCode::Enter), Action::UnlockTouchId));
        unlock.touch_id_cancelled();
        assert!(screen(&mut unlock).contains("Touch ID cancelled"));
        press(&mut unlock, KeyCode::Char('p'));
        assert!(screen(&mut unlock).contains("Enter unlock"));
        let Action::Unlock(password) = press(&mut unlock, KeyCode::Enter) else {
            panic!("not unlocking with the password")
        };
        assert_eq!(password.as_str(), "p");
    }

    #[test]
    fn touch_id_is_suggested_only_where_it_is_off() {
        let tip = "Settings can turn on unlocking with Touch ID";
        assert!(screen(&mut Unlock::new(TouchId::Off)).contains(tip));
        let mut unlock = Unlock::new(TouchId::Unavailable);
        let text = screen(&mut unlock);
        assert!(!text.contains(tip) && !text.contains("Touch ID"), "{text}");
        assert!(matches!(press(&mut unlock, KeyCode::Enter), Action::None));
    }
}
