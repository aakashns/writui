//! Lists as you type: Enter carries a list item's bullet, number or task box
//! on to the next line (or, on an empty item, ends the list), and Tab or
//! Shift+Tab at the start of an item nests it under the one above or brings
//! it back out. Quotes carry on too. Good enough for the usual cases, not a
//! full markdown parser: it looks at the line's own prefix only.

use std::ops::Range;

use super::wrap::TAB_WIDTH;

/// The markdown at the start of a line that makes it a list item or quote.
#[derive(Debug, PartialEq)]
pub struct Prefix {
    /// `>` quote markers (with their spaces), before anything else.
    quote: String,
    /// Spaces (or tabs) before the marker, in chars.
    indent: usize,
    /// How wide the indent is on screen.
    indent_width: usize,
    marker: Marker,
    /// A task box after the marker: `[ ] `, `[x] `.
    task: bool,
    /// The whole prefix, in chars.
    pub len: usize,
}

#[derive(Debug, PartialEq)]
enum Marker {
    /// Just a quote.
    None,
    /// `-`, `*` or `+`.
    Bullet(char),
    /// A number and `.` or `)`.
    Number(u64, char),
}

impl Prefix {
    /// What starts the next line of the same list (or quote).
    pub fn next(&self, line: &str) -> String {
        let indent: String =
            line.chars().skip(self.quote.chars().count()).take(self.indent).collect();
        let marker = match self.marker {
            Marker::None => String::new(),
            Marker::Bullet(ch) => format!("{ch} "),
            Marker::Number(n, delim) => format!("{}{delim} ", n + 1),
        };
        let task = if self.task { "[ ] " } else { "" };
        format!("{}{indent}{marker}{task}", self.quote)
    }

    /// Nothing written after the prefix yet.
    pub fn empty(&self, line: &str) -> bool {
        line.chars().skip(self.len).all(char::is_whitespace)
    }

    /// A list item (rather than just a quote).
    pub fn is_item(&self) -> bool {
        self.marker != Marker::None
    }

    /// Where the item's text starts, on screen: its children are indented
    /// this far.
    fn content_width(&self) -> usize {
        let marker = match self.marker {
            Marker::None => 0,
            Marker::Bullet(_) => 2,
            Marker::Number(n, _) => n.to_string().len() + 2,
        };
        self.indent_width + marker
    }
}

/// The list or quote prefix of `line` (without its newline), if it has one.
pub fn prefix(line: &str) -> Option<Prefix> {
    let mut rest = line;
    while let Some(r) = rest.strip_prefix('>') {
        rest = r.strip_prefix(' ').unwrap_or(r);
    }
    let quote = line[..line.len() - rest.len()].to_string();
    let after_indent = rest.trim_start_matches([' ', '\t']);
    let indent_text = &rest[..rest.len() - after_indent.len()];
    let indent_width = indent_text.chars().map(|ch| if ch == '\t' { TAB_WIDTH } else { 1 }).sum();
    let (marker, after) = match marker(after_indent) {
        Some(found) => found,
        None if !quote.is_empty() => (Marker::None, after_indent),
        None => return None,
    };
    let (task, after) = match ["[ ] ", "[x] ", "[X] "].iter().find_map(|b| after.strip_prefix(b)) {
        Some(r) if marker != Marker::None => (true, r),
        _ => (false, after),
    };
    // A quote's indent is part of its text, unless a list item follows.
    let (indent, after) = match marker {
        Marker::None => (0, rest),
        _ => (indent_text.chars().count(), after),
    };
    let len = line.chars().count() - after.chars().count();
    Some(Prefix { quote, indent, indent_width, marker, task, len })
}

/// A list marker at the start of `text`, and what follows it.
fn marker(text: &str) -> Option<(Marker, &str)> {
    let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let (marker, rest) = match digits {
        0 => {
            let ch = text.chars().next().filter(|ch| matches!(ch, '-' | '*' | '+'))?;
            (Marker::Bullet(ch), &text[1..])
        }
        1..=9 => {
            let delim = text[digits..].chars().next().filter(|ch| matches!(ch, '.' | ')'))?;
            (Marker::Number(text[..digits].parse().ok()?, delim), &text[digits + 1..])
        }
        _ => return None,
    };
    Some((marker, rest.strip_prefix(' ')?))
}

/// Nest the list item `lines[at]` one level deeper (under the item above
/// it) or bring it one level out: the chars of the line to replace, and
/// what to put there (the new leading whitespace, and for a numbered item
/// its new number). `None` if it isn't a list item, or can't go that way.
pub fn reindent(lines: &[String], at: usize, deeper: bool) -> Option<(Range<usize>, String)> {
    let item = prefix(&lines[at]).filter(Prefix::is_item)?;
    // The items above, nearest first, in the same quote.
    let above = || {
        lines[..at]
            .iter()
            .rev()
            .take_while(|line| !line.trim().is_empty())
            .filter_map(|line| prefix(line))
            .filter(|p| p.is_item() && p.quote == item.quote)
    };
    let width = if deeper {
        // Under the item above at the same level, lined up with its text.
        match above().find(|p| p.indent_width <= item.indent_width) {
            Some(p) if p.indent_width == item.indent_width => p.content_width(),
            _ => item.content_width(),
        }
    } else {
        if item.indent_width == 0 {
            return None;
        }
        // Back to where its parent is.
        above().find(|p| p.indent_width < item.indent_width).map_or(0, |p| p.indent_width)
    };
    let start = item.quote.chars().count();
    let mut end = start + item.indent;
    let mut text = " ".repeat(width);
    // A nested numbered list has to start at 1 to be a list at all; one
    // brought out carries on from the item above it at its new level.
    if let Marker::Number(n, _) = item.marker {
        end += n.to_string().len();
        let n = match above().find(|p| p.indent_width <= width) {
            _ if deeper => 1,
            Some(Prefix { marker: Marker::Number(above, _), indent_width, .. })
                if indent_width == width =>
            {
                above + 1
            }
            _ => n,
        };
        text.push_str(&n.to_string());
    }
    Some((start..end, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next(line: &str) -> Option<String> {
        prefix(line).map(|p| p.next(line))
    }

    #[test]
    fn items_carry_on() {
        assert_eq!(next("- milk").as_deref(), Some("- "));
        assert_eq!(next("  * eggs").as_deref(), Some("  * "));
        assert_eq!(next("+ x").as_deref(), Some("+ "));
        assert_eq!(next("1. one").as_deref(), Some("2. "));
        assert_eq!(next("9) nine").as_deref(), Some("10) "));
        assert_eq!(next("- [x] done").as_deref(), Some("- [ ] "));
        assert_eq!(next("   3. [ ] task").as_deref(), Some("   4. [ ] "));
        assert_eq!(next("> quoted").as_deref(), Some("> "));
        assert_eq!(next("> - in a quote").as_deref(), Some("> - "));
    }

    #[test]
    fn other_lines_are_not_lists() {
        for line in ["plain", "-no space", "---", "**bold**", "1.5 litres", "# Title", ""] {
            assert_eq!(prefix(line), None, "{line}");
        }
    }

    #[test]
    fn empty_items() {
        let empty = |line: &str| prefix(line).unwrap().empty(line);
        assert!(empty("- ") && empty("  2. ") && empty("- [ ] ") && empty("> "));
        assert!(!empty("- a") && !empty("> a"));
        assert_eq!(prefix("- [ ] task").unwrap().len, 6);
    }

    #[test]
    fn nesting_lines_up_under_the_item_above() {
        let doc: Vec<String> = "- a\n- b\n  - c\n  - d\n1. one\n2. two\nplain\n> - x\n> - y"
            .lines()
            .map(String::from)
            .collect();
        let indent = |at, deeper| reindent(&doc, at, deeper).map(|(range, s)| (range, s.len()));
        assert_eq!(indent(1, true), Some((0..0, 2)));
        assert_eq!(indent(3, true), Some((0..2, 4)));
        assert_eq!(indent(3, false), Some((0..2, 0)));
        assert_eq!(indent(1, false), None);
        // Numbered items nest under the number's text, starting again at 1,
        // and carry on the numbers above when brought back out.
        assert_eq!(reindent(&doc, 5, true), Some((0..1, "   1".into())));
        let nested: Vec<String> =
            ["1. one", "   1. a", "   2. b", "2. two"].map(String::from).to_vec();
        assert_eq!(reindent(&nested, 2, false), Some((0..4, "2".into())));
        assert_eq!(reindent(&nested, 3, true), Some((0..1, "   1".into())));
        assert_eq!(indent(6, true), None);
        // The first item can still go in, by its own marker.
        assert_eq!(indent(0, true), Some((0..0, 2)));
        // In a quote, after the `> `.
        assert_eq!(indent(8, true), Some((2..2, 2)));
    }
}
