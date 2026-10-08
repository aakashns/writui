//! Soft wrap: how the text is split into rows on screen.
//!
//! Lines break after spaces, so words stay whole. A word too long for the
//! column is cut wherever it runs out of room. One space may hang a cell
//! past the end of a row, so a row never starts with the space that ended
//! the word before it.
//!
//! A list item or a quote wraps with a hanging indent: its later rows line
//! up under its text, not under the bullet or the `>`. That's on screen
//! only; the text itself has no extra spaces.
//!
//! Positions are char indices into the whole text, and the cursor only ever
//! sits on grapheme boundaries (an emoji or an accented letter is one step).

use std::ops::Range;

use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// How many cells a tab takes on screen.
pub const TAB_WIDTH: usize = 4;

/// One row on screen: the chars `start..end` of the text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    /// Which line (paragraph) of the text this row belongs to.
    pub line: usize,
    pub start: usize,
    pub end: usize,
    /// The last row of its line. The cursor can sit at `end` only on the
    /// last row; elsewhere `end` is where the next row starts.
    pub last: bool,
    /// Blank cells before the row's text: a hanging indent.
    pub indent: usize,
}

pub fn grapheme_width(g: &str) -> usize {
    if g == "\t" { TAB_WIDTH } else { g.width() }
}

/// Split the whole text into rows of at most `width` cells.
pub fn layout(rope: &Rope, width: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut line_start = 0;
    for (line, slice) in rope.lines().enumerate() {
        let text = slice.to_string();
        let text = text.strip_suffix('\n').unwrap_or(&text);
        let indent = hanging_indent(text, width);
        let ranges = wrap_line(text, width, indent);
        let count = ranges.len();
        rows.extend(ranges.into_iter().enumerate().map(|(i, range)| Row {
            line,
            start: line_start + range.start,
            end: line_start + range.end,
            last: i + 1 == count,
            indent: if i == 0 { 0 } else { indent },
        }));
        line_start += slice.len_chars();
    }
    rows
}

/// How far a line's later rows are indented: up to the text of a list item
/// (`- `, `1. `, `- [ ] `) or a quote (`> `), after any indentation. Never
/// more than half the width.
pub fn hanging_indent(line: &str, width: usize) -> usize {
    let mut rest = line.trim_start_matches([' ', '\t']);
    let mut marked = false;
    while let Some(r) = rest.strip_prefix('>').or_else(|| list_marker(rest)) {
        rest = r.trim_start_matches(' ');
        marked = true;
    }
    if let Some(r) = ["[ ] ", "[x] ", "[X] "].iter().find_map(|b| rest.strip_prefix(b)) {
        rest = r.trim_start_matches(' ');
    }
    if !marked || rest.is_empty() {
        return 0;
    }
    let prefix = &line[..line.len() - rest.len()];
    let indent: usize = prefix.graphemes(true).map(grapheme_width).sum();
    if indent * 2 > width { 0 } else { indent }
}

/// The rest of `text` after a list item's bullet or number and its space.
fn list_marker(text: &str) -> Option<&str> {
    let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let rest = match digits {
        0 => text.strip_prefix(['-', '*', '+'])?,
        1..=9 => text[digits..].strip_prefix(['.', ')'])?,
        _ => return None,
    };
    rest.strip_prefix(' ')
}

/// Split one line (without its newline) into rows, the later ones `indent`
/// cells narrower. Returns char ranges within the line; an empty line is
/// one empty row.
pub fn wrap_line(line: &str, width: usize, indent: usize) -> Vec<Range<usize>> {
    let mut rows = Vec::new();
    let mut width = width;
    let mut start = 0;
    let mut col = 0;
    let mut pos = 0;
    // Where the row could break: just after the last space (char index, and
    // the column there).
    let mut brk: Option<(usize, usize)> = None;
    for g in line.graphemes(true) {
        let w = grapheme_width(g);
        let space = usize::from(g == " ");
        while col > 0 && col + w > width + space {
            let at = match brk.take() {
                Some((at, at_col)) => {
                    col -= at_col;
                    at
                }
                None => {
                    col = 0;
                    pos
                }
            };
            rows.push(start..at);
            start = at;
            if rows.len() == 1 {
                width -= indent.min(width - 1);
            }
        }
        col += w;
        pos += g.chars().count();
        if g == " " || g == "\t" {
            brk = Some((pos, col));
        }
    }
    rows.push(start..pos);
    rows
}

/// The row the cursor at `pos` is on. At the boundary between two rows of a
/// line, that's the later row.
pub fn row_of(rows: &[Row], pos: usize) -> usize {
    rows.partition_point(|r| r.start <= pos).saturating_sub(1)
}

/// How many cells from the start of `row` (its indent included) the
/// position `pos` is.
pub fn x_of(rope: &Rope, row: Row, pos: usize) -> usize {
    let text = rope.slice(row.start..pos.clamp(row.start, row.end)).to_string();
    row.indent + text.graphemes(true).map(grapheme_width).sum::<usize>()
}

/// The position in `row` nearest to `x` cells from its start (e.g. for a
/// click, or moving up and down).
pub fn pos_at_x(rope: &Rope, row: Row, x: usize) -> usize {
    let text = rope.slice(row.start..row.end).to_string();
    let mut graphemes = text.graphemes(true).peekable();
    let x = x.saturating_sub(row.indent);
    let mut pos = row.start;
    let mut col = 0;
    while let Some(g) = graphemes.next() {
        let w = grapheme_width(g);
        // Past the last grapheme of a non-last row is the next row's start.
        let row_end = graphemes.peek().is_none() && !row.last;
        if col + w > x || row_end {
            break;
        }
        col += w;
        pos += g.chars().count();
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(line: &str, width: usize) -> Vec<String> {
        let chars: Vec<char> = line.chars().collect();
        wrap_line(line, width, 0).into_iter().map(|r| chars[r].iter().collect()).collect()
    }

    #[test]
    fn short_lines_stay_whole() {
        assert_eq!(rows("", 10), [""]);
        assert_eq!(rows("hello", 10), ["hello"]);
        assert_eq!(rows("0123456789", 10), ["0123456789"]);
    }

    #[test]
    fn breaks_after_spaces() {
        assert_eq!(rows("the quick brown fox", 10), ["the quick ", "brown fox"]);
        assert_eq!(rows("aaaa bbbb cccc dddd", 8), ["aaaa ", "bbbb ", "cccc ", "dddd"]);
    }

    #[test]
    fn one_space_hangs_past_the_edge() {
        // The space after a full row stays on it instead of starting the next.
        assert_eq!(rows("0123456789 next", 10), ["0123456789 ", "next"]);
        // A second space doesn't fit, so it starts the next row.
        assert_eq!(rows("0123456789  next", 10), ["0123456789 ", " next"]);
    }

    #[test]
    fn long_words_are_cut() {
        assert_eq!(rows("abcdefghijklmnop", 6), ["abcdef", "ghijkl", "mnop"]);
        assert_eq!(rows("hi abcdefghijklmnop", 6), ["hi ", "abcdef", "ghijkl", "mnop"]);
    }

    #[test]
    fn wide_characters_count_double() {
        assert_eq!(rows("日本語のテキスト", 6), ["日本語", "のテキ", "スト"]);
    }

    #[test]
    fn graphemes_are_never_split() {
        // "e" + combining acute accent is one grapheme of width 1.
        let text = "cafe\u{301} cafe\u{301}";
        assert_eq!(rows(text, 5), ["cafe\u{301} ", "cafe\u{301}"]);
    }

    #[test]
    fn layout_covers_every_line() {
        let rope = Rope::from_str("# Title\n\nthe quick brown fox");
        let rows = layout(&rope, 10);
        let row = |line, start, end, last| Row { line, start, end, last, indent: 0 };
        assert_eq!(
            rows,
            [row(0, 0, 7, true), row(1, 8, 8, true), row(2, 9, 19, false), row(2, 19, 28, true)]
        );
        assert_eq!(row_of(&rows, 7), 0);
        assert_eq!(row_of(&rows, 8), 1);
        assert_eq!(row_of(&rows, 18), 2);
        assert_eq!(row_of(&rows, 19), 3);
        assert_eq!(row_of(&rows, 28), 3);
    }

    #[test]
    fn positions_and_columns() {
        let rope = Rope::from_str("the quick brown fox");
        let rows = layout(&rope, 10);
        assert_eq!(x_of(&rope, rows[1], 13), 3);
        assert_eq!(pos_at_x(&rope, rows[1], 3), 13);
        // Past the end of the last row: its end.
        assert_eq!(pos_at_x(&rope, rows[1], 50), 19);
        // Past the end of an earlier row: just before its last character,
        // so the cursor stays on that row.
        assert_eq!(pos_at_x(&rope, rows[0], 50), 9);
    }

    #[test]
    fn list_items_and_quotes_hang() {
        assert_eq!(hanging_indent("- point", 60), 2);
        assert_eq!(hanging_indent("  * nested", 60), 4);
        assert_eq!(hanging_indent("12. twelfth", 60), 4);
        assert_eq!(hanging_indent("1) first", 60), 3);
        assert_eq!(hanging_indent("- [ ] task", 60), 6);
        assert_eq!(hanging_indent("> quote", 60), 2);
        assert_eq!(hanging_indent("> - quoted point", 60), 4);
        // Not list items.
        assert_eq!(hanging_indent("plain", 60), 0);
        assert_eq!(hanging_indent("-not", 60), 0);
        assert_eq!(hanging_indent("**bold**", 60), 0);
        assert_eq!(hanging_indent("2024. A year", 60), 6);
        assert_eq!(hanging_indent("1234567890. no", 60), 0);
        // Just the bullet so far, or a too-wide prefix.
        assert_eq!(hanging_indent("- ", 60), 0);
        assert_eq!(hanging_indent("                - x", 20), 0);
    }

    #[test]
    fn later_rows_are_narrower_by_the_indent() {
        let line = "- the quick brown fox jumps";
        let chars: Vec<char> = line.chars().collect();
        let rows: Vec<String> = wrap_line(line, 12, 2).into_iter().map(|r| chars[r].iter().collect()).collect();
        assert_eq!(rows, ["- the quick ", "brown fox ", "jumps"]);

        let rope = Rope::from_str(line);
        let rows = layout(&rope, 12);
        assert_eq!((rows[0].indent, rows[1].indent, rows[2].indent), (0, 2, 2));
        // The cursor and clicks count the indent.
        assert_eq!(x_of(&rope, rows[1], 12), 2);
        assert_eq!(pos_at_x(&rope, rows[1], 0), 12);
        assert_eq!(pos_at_x(&rope, rows[1], 4), 14);
    }
}
