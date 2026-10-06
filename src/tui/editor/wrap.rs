//! Soft wrap: how the text is split into rows on screen.
//!
//! Lines break after spaces, so words stay whole. A word too long for the
//! column is cut wherever it runs out of room. One space may hang a cell
//! past the end of a row, so a row never starts with the space that ended
//! the word before it.
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
        let ranges = wrap_line(text.strip_suffix('\n').unwrap_or(&text), width);
        let count = ranges.len();
        rows.extend(ranges.into_iter().enumerate().map(|(i, range)| Row {
            line,
            start: line_start + range.start,
            end: line_start + range.end,
            last: i + 1 == count,
        }));
        line_start += slice.len_chars();
    }
    rows
}

/// Split one line (without its newline) into rows. Returns char ranges
/// within the line; an empty line is one empty row.
pub fn wrap_line(line: &str, width: usize) -> Vec<Range<usize>> {
    let mut rows = Vec::new();
    let mut start = 0;
    let mut col = 0;
    let mut pos = 0;
    // Where the row could break: just after the last space (char index, and
    // the column there).
    let mut brk: Option<(usize, usize)> = None;
    for g in line.graphemes(true) {
        let w = grapheme_width(g);
        let limit = if g == " " { width + 1 } else { width };
        while col > 0 && col + w > limit {
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

/// How many cells from the start of `row` the position `pos` is.
pub fn x_of(rope: &Rope, row: Row, pos: usize) -> usize {
    let text = rope.slice(row.start..pos.clamp(row.start, row.end)).to_string();
    text.graphemes(true).map(grapheme_width).sum()
}

/// The position in `row` nearest to `x` cells from its start (e.g. for a
/// click, or moving up and down).
pub fn pos_at_x(rope: &Rope, row: Row, x: usize) -> usize {
    let text = rope.slice(row.start..row.end).to_string();
    let mut graphemes = text.graphemes(true).peekable();
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
        wrap_line(line, width).into_iter().map(|r| chars[r].iter().collect()).collect()
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
        let row = |line, start, end, last| Row { line, start, end, last };
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
}
