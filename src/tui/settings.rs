//! The Settings screen: a few things to set, kept in the vault. Each row
//! shows a setting's value; Enter (or a click) changes it in a dialog.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List as ListWidget, ListItem, ListState, Paragraph};

use super::Action;
use super::dialog::{Button, Dialog};
use super::hints::{HintBar, hint};
use super::widgets::{COLUMN_WIDTH, column, truncate};
use crate::vault::{DEFAULT_LINE_WIDTH, LINE_WIDTHS, Settings as Values};

/// The longest author's name.
const MAX_AUTHOR: usize = 80;

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Author,
    LineWidth,
}

const FIELDS: [Field; 2] = [Field::Author, Field::LineWidth];

#[derive(Clone, Copy)]
enum Cmd {
    Change,
    Save,
    Cancel,
    Back,
    Lock,
}

pub struct Settings {
    values: Values,
    state: ListState,
    /// Changing the highlighted setting.
    dialog: Option<Dialog<Cmd>>,
    notice: Option<String>,
    error: Option<String>,
    rows_area: Rect,
    hints: HintBar<Cmd>,
}

impl Settings {
    pub fn new(values: Values) -> Self {
        Settings {
            values,
            state: ListState::default().with_selected(Some(0)),
            dialog: None,
            notice: None,
            error: None,
            rows_area: Rect::default(),
            hints: HintBar::default(),
        }
    }

    /// Storing the settings failed.
    pub fn set_error(&mut self, error: String) {
        self.notice = None;
        self.error = Some(error);
    }

    fn field(&self) -> Field {
        FIELDS[self.state.selected().unwrap_or(0).min(FIELDS.len() - 1)]
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [_, header, _, rows, notice, _, hint_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        frame.render_widget(Paragraph::new("Settings".bold()), column(header, COLUMN_WIDTH));

        let col = column(rows, COLUMN_WIDTH);
        let width = col.width as usize;
        let items: Vec<ListItem> = FIELDS
            .iter()
            .map(|field| {
                let (name, value) = match field {
                    Field::Author if self.values.author.is_empty() => ("Author", Span::raw("none").dim().italic()),
                    Field::Author => ("Author", Span::raw(self.values.author.clone())),
                    Field::LineWidth => ("Line width", Span::raw(format!("{} characters", self.values.line_width))),
                };
                let value = Span::styled(truncate(&value.content, width.saturating_sub(name.len() + 2)), value.style);
                let pad = width.saturating_sub(name.len() + value.width());
                ListItem::new(Line::from(vec![Span::raw(name).bold(), Span::raw(" ".repeat(pad)), value]))
            })
            .collect();
        self.rows_area = Rect { height: (FIELDS.len() as u16).min(col.height), ..col };
        frame.render_stateful_widget(
            ListWidget::new(items).highlight_style(Style::new().reversed()),
            self.rows_area,
            &mut self.state,
        );

        let message = match (&self.error, &self.notice) {
            (Some(error), _) => Some(truncate(error, COLUMN_WIDTH as usize).red()),
            (None, Some(notice)) => Some(truncate(notice, COLUMN_WIDTH as usize).dim()),
            (None, None) => None,
        };
        if let Some(message) = message {
            frame.render_widget(Paragraph::new(message), column(notice, COLUMN_WIDTH));
        }

        if let Some(dialog) = &mut self.dialog {
            dialog.render(frame);
            let hints = dialog.hints();
            self.hints.render(frame, hint_area, &hints);
            return;
        }
        self.hints.render(frame, hint_area, &[hint("Enter", "change", Cmd::Change), hint("Esc", "back to posts", Cmd::Back), hint("Ctrl+O", "lock", Cmd::Lock)]);
    }

    pub fn handle(&mut self, event: Event) -> Action {
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
        if let Event::Key(_) | Event::Mouse(_) = event {
            self.notice = None;
        }
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Enter => self.run(Cmd::Change),
                KeyCode::Esc => self.run(Cmd::Back),
                KeyCode::Up => self.move_by(-1),
                KeyCode::Down => self.move_by(1),
                _ => Action::None,
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.move_by(-1),
                MouseEventKind::ScrollDown => self.move_by(1),
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(cmd) = self.hints.hit(mouse.column, mouse.row) {
                        return self.run(cmd);
                    }
                    if !self.rows_area.contains(Position::new(mouse.column, mouse.row)) {
                        return Action::None;
                    }
                    self.state.select(Some((mouse.row - self.rows_area.y) as usize));
                    self.run(Cmd::Change)
                }
                _ => Action::None,
            },
            _ => Action::None,
        }
    }

    fn move_by(&mut self, delta: isize) -> Action {
        let current = self.state.selected().unwrap_or(0) as isize;
        self.state.select(Some((current + delta).clamp(0, FIELDS.len() as isize - 1) as usize));
        Action::None
    }

    fn run(&mut self, cmd: Cmd) -> Action {
        match cmd {
            Cmd::Back => Action::BackToList(None),
            Cmd::Lock => Action::Lock,
            Cmd::Change => {
                self.dialog = Some(self.dialog_for(self.field()));
                Action::None
            }
            Cmd::Cancel => {
                self.dialog = None;
                Action::None
            }
            Cmd::Save => {
                let Some(dialog) = self.dialog.take() else { return Action::None };
                let text = dialog.input().trim();
                let mut values = self.values.clone();
                match self.field() {
                    Field::Author => values.author = text.to_string(),
                    Field::LineWidth => match text.parse::<u16>() {
                        Ok(width) if LINE_WIDTHS.contains(&width) => values.line_width = width,
                        _ => {
                            let (min, max) = (LINE_WIDTHS.start(), LINE_WIDTHS.end());
                            self.notice = Some(format!("The line width is a number from {min} to {max}."));
                            return Action::None;
                        }
                    },
                }
                if values == self.values {
                    return Action::None;
                }
                self.values = values.clone();
                self.error = None;
                self.notice = Some("Saved.".into());
                Action::SaveSettings(values)
            }
        }
    }

    fn dialog_for(&self, field: Field) -> Dialog<Cmd> {
        let buttons = || {
            vec![
                Button { label: "Save", key: "", cmd: Cmd::Save, danger: false },
                Button { label: "Cancel", key: "", cmd: Cmd::Cancel, danger: false },
            ]
        };
        match field {
            Field::Author => Dialog::new(
                "Author",
                vec![
                    Line::from("Your name, for the front matter of exports."),
                    Line::from("Leave it empty to leave the author out.".dim()),
                ],
                buttons(),
                0,
                1,
            )
            .with_input("e.g. Ada Lovelace", MAX_AUTHOR)
            .with_text(&self.values.author),
            Field::LineWidth => {
                let (min, max) = (LINE_WIDTHS.start(), LINE_WIDTHS.end());
                Dialog::new(
                    "Line width",
                    vec![
                        Line::from("How wide the text is in the editor, in characters."),
                        Line::from(format!("From {min} to {max}; the default is {DEFAULT_LINE_WIDTH}.").dim()),
                    ],
                    buttons(),
                    0,
                    1,
                )
                .with_input("68", 3)
                .with_text(&self.values.line_width.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

    use super::*;

    fn screen(settings: &mut Settings) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| settings.render(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..24).map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    fn press(settings: &mut Settings, code: KeyCode) -> Action {
        settings.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
    }

    fn type_text(settings: &mut Settings, text: &str) {
        for ch in text.chars() {
            press(settings, KeyCode::Char(ch));
        }
    }

    #[test]
    fn settings_are_changed_in_a_dialog() {
        let mut settings = Settings::new(Values::default());
        let text = screen(&mut settings);
        assert!(text.contains("Author") && text.contains("none") && text.contains("68 characters"), "{text}");
        // The author.
        press(&mut settings, KeyCode::Enter);
        type_text(&mut settings, "Ada");
        let Action::SaveSettings(values) = press(&mut settings, KeyCode::Enter) else { panic!("not saved") };
        assert_eq!(values.author, "Ada");
        assert!(screen(&mut settings).contains("Ada"));
        // The line width: starts with the current one; out of range is refused.
        press(&mut settings, KeyCode::Down);
        press(&mut settings, KeyCode::Enter);
        assert!(screen(&mut settings).contains("68"));
        press(&mut settings, KeyCode::Backspace);
        press(&mut settings, KeyCode::Backspace);
        type_text(&mut settings, "500");
        assert!(matches!(press(&mut settings, KeyCode::Enter), Action::None));
        assert!(screen(&mut settings).contains("from 40 to 120"));
        press(&mut settings, KeyCode::Enter);
        press(&mut settings, KeyCode::Backspace);
        press(&mut settings, KeyCode::Backspace);
        type_text(&mut settings, "80");
        let Action::SaveSettings(values) = press(&mut settings, KeyCode::Enter) else { panic!("not saved") };
        assert_eq!((values.author.as_str(), values.line_width), ("Ada", 80));
        // Esc goes back.
        assert!(matches!(press(&mut settings, KeyCode::Esc), Action::BackToList(None)));
    }
}
