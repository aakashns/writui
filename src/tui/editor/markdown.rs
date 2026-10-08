//! Live markdown formatting: how each part of the text is styled on screen.
//!
//! The markdown source always shows in full. Formatted text gets the
//! terminal's text styles (bold, italic, underline, strikethrough) and no
//! colours, except code, which is green. The markdown symbols around it
//! (`#`, `**`, backticks, `>`, list bullets, a link's `](url)`) are faded.
//!
//! The text is parsed by a CommonMark parser (with GitHub's strikethrough,
//! task lists and tables), so something is styled only when it would really
//! render that way: a half-typed `**bold` stays plain until it's closed.
//!
//! Links are found here too, so they can be opened: markdown links, `<…>`
//! links, and web addresses written out in the text.

use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag};
use ratatui::style::Style;
use ropey::Rope;

/// A link in the text: the chars it covers, and where it goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub range: Range<usize>,
    pub url: String,
}

/// The styles and links of a text.
#[derive(Clone, Debug, Default)]
pub struct Markup {
    /// Where each style starts (a char index), in order. Each runs to where
    /// the next starts; the first starts at 0.
    runs: Vec<(usize, Style)>,
    links: Vec<Link>,
}

impl Markup {
    pub fn new(rope: &Rope) -> Self {
        let text = rope.to_string();
        let mut styler = Styler { runs: Vec::new(), links: Vec::new(), stack: Vec::new(), pos: 0, text: None };
        let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES;
        for (event, range) in Parser::new_ext(&text, options).into_offset_iter() {
            styler.event(&text, event, range);
        }
        styler.gap(&text, text.len());
        styler.flush_text(&text);

        // From byte offsets to char indices.
        let mut runs: Vec<(usize, Style)> = Vec::with_capacity(styler.runs.len());
        for (start, style) in styler.runs {
            let start = rope.byte_to_char(start);
            match runs.last_mut() {
                Some(last) if last.1 == style => {}
                Some(last) if last.0 == start => last.1 = style,
                _ => runs.push((start, style)),
            }
        }
        styler.links.sort_by_key(|(range, _)| range.start);
        let links = styler
            .links
            .into_iter()
            .map(|(range, url)| Link { range: rope.byte_to_char(range.start)..rope.byte_to_char(range.end), url })
            .collect();
        Markup { runs, links }
    }

    /// The styled pieces of the chars `range`, in order, covering all of it.
    pub fn styles(&self, range: Range<usize>) -> Vec<(Range<usize>, Style)> {
        let mut out = Vec::new();
        let first = self.runs.partition_point(|r| r.0 <= range.start).saturating_sub(1);
        for (i, &(start, style)) in self.runs.iter().enumerate().skip(first) {
            let end = self.runs.get(i + 1).map_or(usize::MAX, |r| r.0);
            let piece = start.max(range.start)..end.min(range.end);
            if start >= range.end {
                break;
            }
            if piece.start < piece.end {
                out.push((piece, style));
            }
        }
        if out.is_empty() && range.start < range.end {
            out.push((range, Style::new()));
        }
        out
    }

    /// The link at the char `pos`, if there is one.
    pub fn link_at(&self, pos: usize) -> Option<&Link> {
        self.links.iter().find(|l| l.range.contains(&pos))
    }
}

/// Walks the parser's events, styling every byte of the text in order.
struct Styler {
    /// Byte offset where each style starts.
    runs: Vec<(usize, Style)>,
    /// Byte ranges.
    links: Vec<(Range<usize>, String)>,
    /// The styles of the elements we're inside, and whether each is a link.
    stack: Vec<(Style, bool)>,
    /// Everything before this byte is styled.
    pos: usize,
    /// Plain text seen since the last other event, to look for web
    /// addresses in (the parser can split text into several pieces).
    text: Option<Range<usize>>,
}

impl Styler {
    fn event(&mut self, text: &str, event: Event, range: Range<usize>) {
        if !matches!(event, Event::Text(_)) {
            self.flush_text(text);
        }
        match event {
            Event::Start(tag) => {
                self.gap(text, range.start);
                let (style, link) = match tag {
                    Tag::Heading { .. } | Tag::Strong | Tag::TableHead => (Style::new().bold(), None),
                    Tag::Emphasis => (Style::new().italic(), None),
                    Tag::Strikethrough => (Style::new().crossed_out(), None),
                    Tag::CodeBlock(_) => (code(), None),
                    Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                        (Style::new().underlined(), Some(dest_url.to_string()))
                    }
                    _ => (Style::new(), None),
                };
                if let Some(url) = &link {
                    self.links.push((range, url.clone()));
                }
                self.stack.push((style, link.is_some()));
            }
            Event::End(_) => {
                self.gap(text, range.end);
                self.stack.pop();
            }
            Event::Text(_) => {
                self.gap(text, range.start);
                self.styled(range.end, self.style());
                let start = self.text.as_ref().map_or(range.start, |t| t.start);
                self.text = Some(start..range.end);
            }
            Event::Code(_) => {
                // The backticks are faded, the code is green.
                self.gap(text, range.start);
                let source = &text[range.clone()];
                let open = source.len() - source.trim_start_matches('`').len();
                let close = source.len() - source.trim_end_matches('`').len();
                let inner = (range.start + open)..(range.end - close).max(range.start + open);
                self.styled(inner.start, faded());
                self.styled(inner.end, self.style().patch(code()));
                self.styled(range.end, faded());
            }
            // Raw HTML, rules, task boxes and line breaks are all markdown
            // symbols, really.
            _ => {
                self.gap(text, range.start);
                self.gap(text, range.end);
            }
        }
    }

    /// The style of text inside everything we're in.
    fn style(&self) -> Style {
        self.stack.iter().fold(Style::new(), |style, (s, _)| style.patch(*s))
    }

    /// Style the text from `pos` up to `end`.
    fn styled(&mut self, end: usize, style: Style) {
        if end > self.pos {
            self.runs.push((self.pos, style));
            self.pos = end;
        }
    }

    /// Text the parser skipped over, up to `end`, is markdown symbols:
    /// faded. Spaces and line breaks around them stay plain.
    fn gap(&mut self, text: &str, end: usize) {
        if end > self.pos {
            let skipped = &text[self.pos..end];
            let symbols = self.pos + (skipped.len() - skipped.trim_start().len());
            let after = end - (skipped.len() - skipped.trim_end().len());
            self.styled(symbols, Style::new());
            self.styled(after, faded());
            self.styled(end, Style::new());
        }
    }

    /// Look for web addresses in the plain text just seen (not inside a
    /// link already, and not in code: code comes as its own event).
    fn flush_text(&mut self, text: &str) {
        let Some(range) = self.text.take() else { return };
        if self.stack.iter().any(|(_, link)| *link) {
            return;
        }
        for found in web_addresses(&text[range.clone()]) {
            let url = text[range.start + found.start..range.start + found.end].to_string();
            self.links.push((range.start + found.start..range.start + found.end, url));
            self.underline(range.start + found.start..range.start + found.end);
        }
    }

    /// Underline a byte range that's already been styled.
    fn underline(&mut self, range: Range<usize>) {
        // Split the runs at both ends, then underline those in between.
        for at in [range.start, range.end] {
            let i = self.runs.partition_point(|r| r.0 <= at);
            if i > 0 && self.runs[i - 1].0 != at && at < self.pos {
                let style = self.runs[i - 1].1;
                self.runs.insert(i, (at, style));
            }
        }
        let first = self.runs.partition_point(|r| r.0 < range.start);
        for run in self.runs[first..].iter_mut().take_while(|r| r.0 < range.end) {
            run.1 = run.1.underlined();
        }
    }
}

fn faded() -> Style {
    Style::new().dim()
}

fn code() -> Style {
    Style::new().green()
}

/// Web addresses (`http://…`, `https://…`) in `text`, as byte ranges. An
/// address ends at whitespace; punctuation at its end (a full stop, a
/// closing bracket with no opening one) is left out.
fn web_addresses(text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(i) = ["https://", "http://"].iter().filter_map(|p| text[from..].find(p)).min() {
        let start = from + i;
        let rest = &text[start..];
        let mut end = start + rest.find(char::is_whitespace).unwrap_or(rest.len());
        loop {
            let address = &text[start..end];
            let Some(last) = address.chars().last() else { break };
            let unbalanced = last == ')' && address.matches('(').count() < address.matches(')').count();
            if ".,;:!?'\"*_~<>".contains(last) || unbalanced {
                end -= last.len_utf8();
            } else {
                break;
            }
        }
        // Just the scheme isn't an address.
        if text[start..end].trim_start_matches("https://").trim_start_matches("http://").contains('.') {
            found.push(start..end);
        }
        from = end.max(start + 1);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text with each styled piece marked: `[b:…]` bold, `[i:…]`
    /// italic, `[f:…]` faded, `[u:…]` underlined, `[s:…]` struck through,
    /// `[g:…]` green (combined, e.g. `[bf:…]`). Plain text is left as it is.
    fn marked(text: &str) -> String {
        let rope = Rope::from_str(text);
        let markup = Markup::new(&rope);
        let mut out = String::new();
        for (range, style) in markup.styles(0..rope.len_chars()) {
            let piece = rope.slice(range).to_string();
            let m = style.add_modifier;
            use ratatui::style::Modifier;
            let tag: String = [
                (Modifier::BOLD, 'b'),
                (Modifier::ITALIC, 'i'),
                (Modifier::DIM, 'f'),
                (Modifier::UNDERLINED, 'u'),
                (Modifier::CROSSED_OUT, 's'),
            ]
            .iter()
            .filter(|(modifier, _)| m.contains(*modifier))
            .map(|(_, c)| *c)
            .chain((style.fg == Some(ratatui::style::Color::Green)).then_some('g'))
            .collect();
            if tag.is_empty() {
                out.push_str(&piece);
            } else {
                out.push_str(&format!("[{tag}:{piece}]"));
            }
        }
        out
    }

    #[test]
    fn headings_are_bold_with_faded_hashes() {
        assert_eq!(marked("# Title"), "[f:#] [b:Title]");
        assert_eq!(marked("# Title\n\n## Part two\ntext"), "[f:#] [b:Title]\n\n[f:##] [b:Part two]\ntext");
    }

    #[test]
    fn bold_italic_and_struck_through() {
        assert_eq!(marked("a **b** c"), "a [f:**][b:b][f:**] c");
        assert_eq!(marked("a *b* _c_"), "a [f:*][i:b][f:*] [f:_][i:c][f:_]");
        assert_eq!(marked("***both***"), "[f:***][bi:both][f:***]");
        assert_eq!(marked("~~gone~~"), "[f:~~][s:gone][f:~~]");
    }

    #[test]
    fn half_typed_formatting_stays_plain() {
        assert_eq!(marked("a **b"), "a **b");
        assert_eq!(marked("`code"), "`code");
    }

    #[test]
    fn code_shows_plain_with_faded_backticks() {
        assert_eq!(marked("a `b*c*` d"), "a [f:`][g:b*c*][f:`] d");
        assert_eq!(marked("```\nx = *1*\n```"), "[f:```]\n[g:x = *1*\n][f:```]");
        assert_eq!(marked("**`b`**"), "[f:**`][bg:b][f:`**]");
    }

    #[test]
    fn quotes_and_lists_fade_their_markers() {
        assert_eq!(marked("> said\n> this"), "[f:>] said\n[f:>] this");
        assert_eq!(marked("- one\n- two"), "[f:-] one\n[f:-] two");
        assert_eq!(marked("1. one\n2. two"), "[f:1.] one\n[f:2.] two");
        assert_eq!(marked("- [ ] task"), "[f:-] [f:[ \\]] task".replace('\\', ""));
    }

    #[test]
    fn rules_and_escapes_are_faded() {
        assert_eq!(marked("a\n\n---\n\nb"), "a\n\n[f:---]\n\nb");
        assert_eq!(marked("\\*not\\*"), "[f:\\]*not[f:\\]*");
    }

    #[test]
    fn links_underline_their_text_and_fade_the_address() {
        assert_eq!(marked("see [this](https://x.com) now"), "see [f:[][u:this][f:](https://x.com)] now");
        let markup = Markup::new(&Rope::from_str("see [this](https://x.com) now"));
        assert_eq!(markup.link_at(4).map(|l| l.url.as_str()), Some("https://x.com"));
        assert_eq!(markup.link_at(24).map(|l| l.url.as_str()), Some("https://x.com"));
        assert_eq!(markup.link_at(25), None);
        assert_eq!(markup.link_at(3), None);
    }

    #[test]
    fn web_addresses_in_text_are_links() {
        let text = "Read https://example.com/a_b(c). Or http://x.org, or <https://y.net>.";
        assert_eq!(
            marked(text),
            "Read [u:https://example.com/a_b(c)]. Or [u:http://x.org], or [f:<][u:https://y.net][f:>]."
        );
        let markup = Markup::new(&Rope::from_str(text));
        let urls: Vec<&str> = markup.links.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(urls, ["https://example.com/a_b(c)", "http://x.org", "https://y.net"]);
        // Not in code.
        assert_eq!(marked("`https://x.com`"), "[f:`][g:https://x.com][f:`]");
    }

    #[test]
    fn formatting_inside_formatting() {
        assert_eq!(marked("## A *b*"), "[f:##] [b:A ][f:*][bi:b][f:*]");
        assert_eq!(marked("**[x](u)**"), "[f:**[][bu:x][f:](u)**]");
    }

    #[test]
    fn styles_cover_a_range_exactly() {
        let markup = Markup::new(&Rope::from_str("ab **cd** ef"));
        let pieces: Vec<Range<usize>> = markup.styles(1..6).into_iter().map(|p| p.0).collect();
        assert_eq!(pieces, [1..3, 3..5, 5..6]);
        assert_eq!(Markup::default().styles(0..3), [(0..3, Style::new())]);
    }

    #[test]
    fn long_posts_are_quick() {
        let paragraph = "Some **bold** and *italic* text, a [link](https://x.com), `code`, and more words. ";
        let text = format!("# Title\n\n{}", (0..2000).map(|i| format!("## Part {i}\n\n{paragraph}\n\n- a\n- b\n\n")).collect::<String>());
        let rope = Rope::from_str(&text);
        let start = std::time::Instant::now();
        let markup = Markup::new(&rope);
        // ~260 KB. Generous, for slow CI machines and unoptimised builds.
        assert!(start.elapsed() < std::time::Duration::from_millis(500), "{:?}", start.elapsed());
        assert_eq!(markup.links.len(), 2000);
    }
}
