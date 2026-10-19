//! The Settings screen: a few things to set, kept in the vault (but for
//! Touch ID, which belongs to the Mac). Each setting shows its value (or
//! all its options, the current one highlighted), and below it, faded,
//! what that value does; lines separate them, and a bar on the left marks
//! the selected one. Enter (or a click) changes it in a dialog, or switches
//! it to its next option; ←/→ or a click on an option picks one.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListState, Paragraph};

use super::Action;
use super::dialog::{Button, Dialog};
use super::hints::{HintBar, hint};
use super::widgets::{COLUMN_WIDTH, column, truncate};
use crate::vault::{DEFAULT_LINE_WIDTH, LINE_WIDTHS, Settings as Values, Styling};

/// The longest author's name.
const MAX_AUTHOR: usize = 80;

/// Screen rows per setting: its name and value, what it does, and the line
/// below it.
const ROWS_EACH: u16 = 3;

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Author,
    LineWidth,
    Styling,
    NerdFont,
    TouchId,
}

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
    /// Whether Touch ID is on, where this Mac has it.
    touch_id: Option<bool>,
    fields: Vec<Field>,
    state: ListState,
    /// Changing the highlighted setting.
    dialog: Option<Dialog<Cmd>>,
    notice: Option<String>,
    error: Option<String>,
    rows_area: Rect,
    /// Where each option was drawn: the field's index, and the option's.
    option_hits: Vec<(Rect, usize, usize)>,
    hints: HintBar<Cmd>,
}

/// The bar beside the selected setting.
const BAR: &str = "┃";

/// Markdown styling's options, in order.
const STYLINGS: [Styling; 3] = [Styling::Full, Styling::Minimal, Styling::Off];

impl Settings {
    pub fn new(values: Values, touch_id: Option<bool>) -> Self {
        let mut fields = vec![Field::Author, Field::LineWidth, Field::Styling, Field::NerdFont];
        if touch_id.is_some() {
            fields.push(Field::TouchId);
        }
        Settings {
            values,
            touch_id,
            fields,
            state: ListState::default().with_selected(Some(0)),
            dialog: None,
            notice: None,
            error: None,
            rows_area: Rect::default(),
            option_hits: Vec::new(),
            hints: HintBar::default(),
        }
    }

    /// Storing the settings failed.
    pub fn set_error(&mut self, error: String) {
        self.notice = None;
        self.error = Some(error);
    }

    /// Touch ID was switched on or off.
    pub fn touch_id_set(&mut self, on: bool) {
        self.touch_id = Some(on);
        self.error = None;
        self.notice = Some(match on {
            true => "Touch ID is on: writui asks for a fingerprint when it starts.".into(),
            false => "Touch ID is off: unlocking takes the password.".into(),
        });
    }

    fn field(&self) -> Field {
        self.fields[self.state.selected().unwrap_or(0).min(self.fields.len() - 1)]
    }

    fn name(field: Field) -> &'static str {
        match field {
            Field::Author => "Author",
            Field::LineWidth => "Line width",
            Field::Styling => "Markdown styling",
            Field::NerdFont => "Nerd Font",
            Field::TouchId => "Touch ID",
        }
    }

    /// The options of a field that's switched rather than changed in a
    /// dialog, and which one it's on.
    fn options(&self, field: Field) -> Option<(Vec<&'static str>, usize)> {
        let on_off = |on: bool| (vec!["on", "off"], if on { 0 } else { 1 });
        match field {
            Field::Author | Field::LineWidth => None,
            Field::Styling => Some((
                STYLINGS.iter().map(|s| s.name()).collect(),
                STYLINGS.iter().position(|&s| s == self.values.styling).unwrap_or(0),
            )),
            Field::NerdFont => Some(on_off(self.values.nerd_font)),
            Field::TouchId => Some(on_off(self.touch_id == Some(true))),
        }
    }

    /// The value of a field changed in a dialog.
    fn value(&self, field: Field) -> Span<'static> {
        match field {
            Field::Author if self.values.author.is_empty() => Span::raw("none").dim().italic(),
            Field::Author => Span::raw(self.values.author.clone()),
            _ => Span::raw(format!("{} characters", self.values.line_width)),
        }
    }

    /// Switch the selected field to option `i`.
    fn choose(&mut self, i: usize) -> Action {
        let field = self.field();
        if self.options(field).is_none_or(|(_, current)| current == i) {
            return Action::None;
        }
        let mut values = self.values.clone();
        match field {
            Field::Styling => values.styling = STYLINGS[i.min(STYLINGS.len() - 1)],
            Field::NerdFont => values.nerd_font = i == 0,
            Field::TouchId => return Action::SetTouchId(i == 0),
            Field::Author | Field::LineWidth => {}
        }
        self.save(values)
    }

    /// ←/→: the option before or after the current one.
    fn step_option(&mut self, delta: isize) -> Action {
        let Some((options, current)) = self.options(self.field()) else { return Action::None };
        let i = (current as isize + delta).clamp(0, options.len() as isize - 1);
        self.choose(i as usize)
    }

    /// What a field's value does, now.
    fn describe(&self, field: Field) -> String {
        match field {
            Field::Author if self.values.author.is_empty() => {
                "Exports leave the author out of their front matter.".into()
            }
            Field::Author => "Exports suggest you as the author in their front matter.".into(),
            Field::LineWidth => {
                format!("Text is at most {} characters wide as you write.", self.values.line_width)
            }
            Field::Styling => match self.values.styling {
                Styling::Full => "Bold, italics and code are formatted; markdown symbols faded.",
                Styling::Minimal => "No bold: just italics, green code and faded markdown symbols.",
                Styling::Off => "Markdown shows as plain text, with no formatting.",
            }
            .into(),
            Field::NerdFont if self.values.nerd_font => {
                "Uses the font's icons, like the search field's magnifier.".into()
            }
            Field::NerdFont => "No icons. Switch it on if your font is a Nerd Font.".into(),
            Field::TouchId if self.touch_id == Some(true) => {
                "Unlocks with your fingerprint when writui starts.".into()
            }
            Field::TouchId => "Unlocking takes the password.".into(),
        }
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
        // The bar goes left of the column, where there's room; otherwise
        // the settings move over for it.
        let (bar_x, col) = match col.x.checked_sub(2) {
            Some(x) => (x, col),
            None => (col.x, Rect { x: col.x + 2, width: col.width.saturating_sub(2), ..col }),
        };
        let width = col.width as usize;
        // Scroll, if the screen is too short for them all, to keep the
        // selected one in view.
        let fit = ((col.height + 1) / ROWS_EACH).max(1) as usize;
        let selected = self.state.selected().unwrap_or(0);
        let first =
            (*self.state.offset_mut()).min(selected).max((selected + 1).saturating_sub(fit));
        *self.state.offset_mut() = first;
        self.option_hits.clear();
        let mut y = col.y;
        for (i, &field) in self.fields.iter().enumerate().skip(first) {
            if y >= col.bottom() {
                break;
            }
            if i > first {
                let rule = "─".repeat(width).dim();
                frame.render_widget(Paragraph::new(rule), Rect { y, height: 1, ..col });
                y += 1;
            }
            let name = Self::name(field);
            // On the right: the options, the current one highlighted (each
            // with a space either side, so the highlight has some room), or
            // the value.
            let value: Vec<Span> = match self.options(field) {
                Some((options, current)) => {
                    let mut spans = Vec::new();
                    let total: usize =
                        options.iter().map(|o| o.len() + 2).sum::<usize>() + options.len() - 1;
                    let mut x = col.right().saturating_sub(total as u16);
                    for (n, option) in options.iter().enumerate() {
                        if n > 0 {
                            spans.push("|".dim());
                            x += 1;
                        }
                        let text = format!(" {option} ");
                        let w = text.len() as u16;
                        spans.push(if n == current { text.reversed() } else { text.dim() });
                        self.option_hits.push((Rect::new(x, y, w, 1), i, n));
                        x += w;
                    }
                    spans
                }
                None => {
                    let value = self.value(field);
                    let text = truncate(&value.content, width.saturating_sub(name.len() + 2));
                    vec![Span::styled(text, value.style)]
                }
            };
            let used: usize = name.len() + value.iter().map(Span::width).sum::<usize>();
            let mut first_line =
                vec![Span::raw(name).bold(), Span::raw(" ".repeat(width.saturating_sub(used)))];
            first_line.extend(value);
            let lines = vec![
                Line::from(first_line),
                Line::from(truncate(&self.describe(field), width).dim()),
            ];
            let area = Rect { y, height: 2, ..col }.intersection(col);
            frame.render_widget(Paragraph::new(lines), area);
            if i == selected {
                let bar = Paragraph::new(vec![Line::from(BAR), Line::from(BAR)]);
                frame.render_widget(bar, Rect { x: bar_x, width: 1, ..area });
            }
            y += 2;
        }
        self.rows_area = Rect { height: y - col.y, ..col };

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
        let mut hints = match self.options(self.field()) {
            Some(_) => {
                vec![hint("←/→", "choose", Cmd::Change), hint("Enter", "switch", Cmd::Change)]
            }
            None => vec![hint("Enter", "change", Cmd::Change)],
        };
        hints.extend([hint("Esc", "back", Cmd::Back), hint("Ctrl+O", "lock", Cmd::Lock)]);
        self.hints.render(frame, hint_area, &hints);
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
                KeyCode::Left => self.step_option(-1),
                KeyCode::Right => self.step_option(1),
                _ => Action::None,
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.move_by(-1),
                MouseEventKind::ScrollDown => self.move_by(1),
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(cmd) = self.hints.hit(mouse.column, mouse.row) {
                        return self.run(cmd);
                    }
                    let at = Position::new(mouse.column, mouse.row);
                    let option = self.option_hits.iter().find(|(rect, ..)| rect.contains(at));
                    if let Some(&(_, field, option)) = option {
                        self.state.select(Some(field));
                        return self.choose(option);
                    }
                    if !self.rows_area.contains(Position::new(mouse.column, mouse.row)) {
                        return Action::None;
                    }
                    // Not on a line between them.
                    let row = mouse.row - self.rows_area.y;
                    if row % ROWS_EACH == ROWS_EACH - 1 {
                        return Action::None;
                    }
                    let index = self.state.offset() + (row / ROWS_EACH) as usize;
                    if index >= self.fields.len() {
                        return Action::None;
                    }
                    self.state.select(Some(index));
                    self.run(Cmd::Change)
                }
                _ => Action::None,
            },
            _ => Action::None,
        }
    }

    fn move_by(&mut self, delta: isize) -> Action {
        let current = self.state.selected().unwrap_or(0) as isize;
        self.state
            .select(Some((current + delta).clamp(0, self.fields.len() as isize - 1) as usize));
        Action::None
    }

    fn run(&mut self, cmd: Cmd) -> Action {
        match cmd {
            Cmd::Back => Action::BackToList(None),
            Cmd::Lock => Action::Lock,
            Cmd::Change => match self.options(self.field()) {
                // A few options: no need to ask, just go to the next.
                Some((options, current)) => self.choose((current + 1) % options.len()),
                None => {
                    self.dialog = Some(self.dialog_for(self.field()));
                    Action::None
                }
            },
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
                            self.notice =
                                Some(format!("The line width is a number from {min} to {max}."));
                            return Action::None;
                        }
                    },
                    Field::Styling | Field::NerdFont | Field::TouchId => {}
                }
                self.save(values)
            }
        }
    }

    /// Keep `values`, if they've changed; the app stores them.
    fn save(&mut self, values: Values) -> Action {
        if values == self.values {
            return Action::None;
        }
        self.values = values.clone();
        self.error = None;
        self.notice = Some("Saved.".into());
        Action::SaveSettings(values)
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
                        Line::from(
                            format!("From {min} to {max}; the default is {DEFAULT_LINE_WIDTH}.")
                                .dim(),
                        ),
                    ],
                    buttons(),
                    0,
                    1,
                )
                .with_input("68", 3)
                .with_text(&self.values.line_width.to_string())
            }
            Field::Styling | Field::NerdFont | Field::TouchId => {
                unreachable!("switched, not asked")
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
        (0..24)
            .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
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
        let mut settings = Settings::new(Values::default(), None);
        let text = screen(&mut settings);
        assert!(
            text.contains("Author") && text.contains("none") && text.contains("68 characters"),
            "{text}"
        );
        // The author.
        press(&mut settings, KeyCode::Enter);
        type_text(&mut settings, "Ada");
        let Action::SaveSettings(values) = press(&mut settings, KeyCode::Enter) else {
            panic!("not saved")
        };
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
        let Action::SaveSettings(values) = press(&mut settings, KeyCode::Enter) else {
            panic!("not saved")
        };
        assert_eq!((values.author.as_str(), values.line_width), ("Ada", 80));
        // Markdown styling: Enter switches to the next of three, no dialog,
        // and what it does shows below.
        press(&mut settings, KeyCode::Down);
        let text = screen(&mut settings);
        assert!(text.contains("Markdown styling") && text.contains("full"), "{text}");
        assert!(text.contains("Bold, italics and code are formatted"), "{text}");
        let mut seen = Vec::new();
        for _ in 0..3 {
            let Action::SaveSettings(values) = press(&mut settings, KeyCode::Enter) else {
                panic!("not saved")
            };
            assert!(settings.dialog.is_none());
            seen.push(values.styling);
        }
        assert_eq!(seen, [Styling::Minimal, Styling::Off, Styling::Full]);
        // Nerd Font: Enter switches it, no dialog.
        press(&mut settings, KeyCode::Down);
        let text = screen(&mut settings);
        assert!(
            text.contains("Nerd Font") && text.contains("off") && text.contains("Enter switch"),
            "{text}"
        );
        let Action::SaveSettings(values) = press(&mut settings, KeyCode::Enter) else {
            panic!("not saved")
        };
        assert!(values.nerd_font && settings.dialog.is_none());
        assert!(screen(&mut settings).contains("on"));
        // Esc goes back.
        assert!(matches!(press(&mut settings, KeyCode::Esc), Action::BackToList(None)));
    }

    #[test]
    fn each_setting_says_what_it_does_with_lines_between_and_clicks_change_them() {
        let mut settings = Settings::new(Values::default(), None);
        let text = screen(&mut settings);
        let rows: Vec<&str> =
            text.lines().map(|r| r.trim().trim_start_matches(BAR).trim()).collect();
        let author = rows.iter().position(|r| r.starts_with("Author")).unwrap();
        assert!(rows[author + 1].starts_with("Exports leave the author out"), "{text}");
        assert!(rows[author + 2].starts_with("────"), "{text}");
        assert!(rows[author + 3].starts_with("Line width"), "{text}");
        // A click on the styling setting's description switches it; one on
        // a line between settings does nothing.
        let y = (author + 7) as u16;
        let click = |settings: &mut Settings, row| {
            settings.handle(Event::Mouse(ratatui::crossterm::event::MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 20,
                row,
                modifiers: KeyModifiers::NONE,
            }))
        };
        assert!(matches!(click(&mut settings, y - 2), Action::None));
        let Action::SaveSettings(values) = click(&mut settings, y) else { panic!("not saved") };
        assert_eq!(values.styling, Styling::Minimal);
        assert!(screen(&mut settings).contains("No bold: just italics"));
    }

    #[test]
    fn options_show_side_by_side_and_can_be_picked() {
        let mut settings = Settings::new(Values::default(), None);
        press(&mut settings, KeyCode::Down);
        press(&mut settings, KeyCode::Down);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| settings.render(frame)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row = |y: u16| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>();
        // The cell where `text` starts on row `y`.
        let at = |y: u16, text: &str| {
            let row = row(y);
            row[..row.find(text).unwrap()].chars().count() as u16
        };
        let y = (0..24).find(|&y| row(y).contains("Markdown styling")).unwrap();
        assert!(row(y).trim_end().ends_with(" full | minimal | off"), "{}", row(y));
        // The current option is highlighted, the others faded; a bar marks
        // the selected setting, and nothing else is reversed.
        let x = at(y, " full ") + 1;
        assert!(buffer[(x, y)].modifier.contains(ratatui::style::Modifier::REVERSED));
        assert!(buffer[(x + 7, y)].modifier.contains(ratatui::style::Modifier::DIM));
        let name = at(y, "Markdown");
        assert!(!buffer[(name, y)].modifier.contains(ratatui::style::Modifier::REVERSED));
        assert_eq!(buffer[(name - 2, y)].symbol(), BAR);
        assert_eq!(buffer[(name - 2, y + 1)].symbol(), BAR);
        assert_ne!(buffer[(name - 2, y - 3)].symbol(), BAR);
        // ←/→ pick the option before or after; past the ends, nothing.
        let Action::SaveSettings(values) = press(&mut settings, KeyCode::Right) else {
            panic!("not saved")
        };
        assert_eq!(values.styling, Styling::Minimal);
        press(&mut settings, KeyCode::Right);
        assert!(matches!(press(&mut settings, KeyCode::Right), Action::None));
        // A click on an option picks it, on any setting.
        let x = at(y + 3, " on ") + 1;
        let click = settings.handle(Event::Mouse(ratatui::crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y + 3,
            modifiers: KeyModifiers::NONE,
        }));
        let Action::SaveSettings(values) = click else { panic!("not saved") };
        assert!(values.nerd_font);
        assert!(settings.field() == Field::NerdFont);
    }

    #[test]
    fn touch_id_is_switched_where_the_mac_has_it() {
        assert!(!screen(&mut Settings::new(Values::default(), None)).contains("Touch ID"));
        let mut settings = Settings::new(Values::default(), Some(false));
        for _ in 0..4 {
            press(&mut settings, KeyCode::Down);
        }
        assert!(screen(&mut settings).contains("Touch ID"));
        assert!(matches!(press(&mut settings, KeyCode::Enter), Action::SetTouchId(true)));
        settings.touch_id_set(true);
        let text = screen(&mut settings);
        assert!(text.contains("Touch ID is on"), "{text}");
        assert!(text.contains("Unlocks with your fingerprint"), "{text}");
        assert!(matches!(press(&mut settings, KeyCode::Enter), Action::SetTouchId(false)));
    }
}
