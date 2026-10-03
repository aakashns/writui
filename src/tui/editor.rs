//! The post editing screen. Placeholder until the editor lands.

use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use super::Action;
use super::hints::{HintBar, hint};
use super::widgets::{COLUMN_WIDTH, centered};
use crate::vault::{Post, title_from_first_line};

#[derive(Clone, Copy)]
enum Cmd {
    Back,
    Quit,
}

pub struct Editor {
    post: Post,
    hints: HintBar<Cmd>,
}

impl Editor {
    pub fn new(post: Post) -> Self {
        Editor { post, hints: HintBar::default() }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [body, hint_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
        let title = title_from_first_line(self.post.body.lines().next().unwrap_or_default());
        let title = if title.is_empty() { "Untitled".to_string() } else { title };
        let lines = vec![
            Line::from(title.bold()),
            Line::default(),
            Line::from("The editor arrives in the next PR.".dim()),
        ];
        frame.render_widget(Paragraph::new(lines).centered(), centered(body, COLUMN_WIDTH, 3));
        self.hints.render(
            frame,
            hint_area,
            &[hint("Esc", "back to posts", Cmd::Back), hint("Ctrl+Q", "quit", Cmd::Quit)],
        );
    }

    pub fn handle(&mut self, event: Event) -> Action {
        let cmd = match event {
            Event::Key(key) if key.code == KeyCode::Esc => Some(Cmd::Back),
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                self.hints.hit(mouse.column, mouse.row)
            }
            _ => None,
        };
        match cmd {
            Some(Cmd::Back) => Action::BackToList(Some(self.post.id)),
            Some(Cmd::Quit) => Action::Quit,
            None => Action::None,
        }
    }
}
