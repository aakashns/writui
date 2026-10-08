//! The hint bar at the bottom of each screen. Every hint is also a button:
//! clicking it does the same thing as pressing its key.

use std::borrow::Cow;

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub struct Hint<C> {
    pub key: &'static str,
    pub label: Cow<'static, str>,
    pub cmd: C,
}

pub fn hint<C>(key: &'static str, label: impl Into<Cow<'static, str>>, cmd: C) -> Hint<C> {
    Hint { key, label: label.into(), cmd }
}

const GAP: u16 = 3;

/// Remembers where each hint was drawn, for mouse clicks.
pub struct HintBar<C> {
    hits: Vec<(Rect, C)>,
}

impl<C> Default for HintBar<C> {
    fn default() -> Self {
        HintBar { hits: Vec::new() }
    }
}

impl<C: Copy> HintBar<C> {
    /// Draw as many hints as fit, centred in `area`.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, hints: &[Hint<C>]) {
        self.draw(frame, area, hints, true);
    }

    /// Draw as many hints as fit from the left of `area`. Returns the
    /// width they took.
    pub fn render_left(&mut self, frame: &mut Frame, area: Rect, hints: &[Hint<C>]) -> u16 {
        self.draw(frame, area, hints, false)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, hints: &[Hint<C>], centre: bool) -> u16 {
        self.hits.clear();
        let widths: Vec<u16> = hints
            .iter()
            .map(|h| (Span::raw(h.key).width() + 1 + Span::raw(h.label.as_ref()).width()) as u16)
            .collect();
        let mut count = 0;
        let mut total = 0;
        for (i, width) in widths.iter().enumerate() {
            let next = total + width + if i > 0 { GAP } else { 0 };
            if next > area.width {
                break;
            }
            total = next;
            count += 1;
        }

        let left_pad = if centre { (area.width - total) / 2 } else { 0 };
        let mut spans = vec![Span::raw(" ".repeat(left_pad as usize))];
        let mut x = area.x + left_pad;
        for (i, h) in hints.iter().take(count).enumerate() {
            if i > 0 {
                spans.push(Span::raw(" ".repeat(GAP as usize)));
                x += GAP;
            }
            spans.push(Span::raw(h.key).bold());
            spans.push(Span::raw(" "));
            spans.push(Span::raw(h.label.clone()).dim());
            self.hits.push((Rect::new(x, area.y, widths[i], 1), h.cmd));
            x += widths[i];
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        total
    }

    /// Draw nothing: no hint can be clicked.
    pub fn hide(&mut self) {
        self.hits.clear();
    }

    /// The command under a mouse click, if any.
    pub fn hit(&self, column: u16, row: u16) -> Option<C> {
        let pos = Position::new(column, row);
        self.hits
            .iter()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, cmd)| *cmd)
    }
}
