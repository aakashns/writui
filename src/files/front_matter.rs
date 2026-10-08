//! Front matter: the YAML header between `---` lines that site generators
//! (Zola, Hugo, Jekyll, …) read a post's title, date and so on from.

use std::collections::HashMap;

use jiff::civil::Date;

use super::file_name;
use crate::vault::ExportSettings;

/// How long a suggested description is, at most, in characters.
const DESCRIPTION_CHARS: usize = 160;

/// The front matter writui suggests for a post: its title, today as the
/// date it was published and updated, the start of its text as the
/// description, and its file name as the slug.
pub fn suggest(title: &str, body: &str, today: Date) -> String {
    let slug = file_name(title);
    let slug = slug.trim_end_matches(".md");
    format!(
        "title: {}\ndate: {today}\nupdated: {today}\ndescription: {}\nslug: {slug}",
        quote(title),
        quote(&description(body)),
    )
}

/// The front matter to start an export with. `suggested` is the front
/// matter suggested now; `last` is how the post was exported (or imported)
/// last time (`same_post`), or another post was, most recently.
///
/// The fields of the last export are kept, in their order, with the ones
/// writui fills in brought up to date: `updated` is today, and the title,
/// description and slug follow the post again unless they were changed by
/// hand. For the same post, `date` stays; for another, it's today, and
/// the title, description and slug are this post's. Fields writui fills in
/// that weren't suggested last time (e.g. the file was imported without
/// them) are added after the others it fills in; ones taken out by hand
/// stay out.
pub fn front_matter(suggested: &str, last: Option<&ExportSettings>, same_post: bool) -> String {
    let Some(last) = last.filter(|last| !last.front_matter.trim().is_empty()) else {
        return suggested.to_string();
    };
    let keyed = |text: &str| -> HashMap<String, String> {
        fields(text).into_iter().filter_map(|(key, text)| Some((key?, text))).collect()
    };
    let fresh = keyed(suggested);
    let before = keyed(&last.suggested);
    let mut out: Vec<String> = Vec::new();
    // Where the fields writui fills in end, so far.
    let mut after_suggested = 0;
    for (key, text) in fields(&last.front_matter) {
        if key.as_ref().is_some_and(|key| fresh.contains_key(key)) {
            after_suggested = out.len() + 1;
        }
        let keep = match key.as_deref() {
            Some("updated") => false,
            Some("date") => same_post,
            Some(key @ ("title" | "description" | "slug")) => {
                same_post && before.get(key).map(|text| value(text)) != Some(value(&text))
            }
            _ => true,
        };
        match key.and_then(|key| fresh.get(&key)) {
            Some(fresh) if !keep => out.push(fresh.clone()),
            _ => out.push(text),
        }
    }
    let had = keyed(&last.front_matter);
    let new = fields(suggested)
        .into_iter()
        .filter(|(key, _)| key.as_ref().is_some_and(|key| !had.contains_key(key) && !before.contains_key(key)))
        .map(|(_, text)| text);
    out.splice(after_suggested..after_suggested, new);
    out.join("\n")
}

/// The fields of `suggested` that `front_matter` has, as if those were
/// all writui suggested: for an imported file, so the fields it has can
/// follow the post, and the ones it lacks are suggested on export.
pub fn suggested_of(suggested: &str, front_matter: &str) -> String {
    let has: Vec<Option<String>> = fields(front_matter).into_iter().map(|(key, _)| key).collect();
    let kept: Vec<String> = fields(suggested).into_iter().filter(|(key, _)| has.contains(key)).map(|(_, text)| text).collect();
    kept.join("\n")
}

/// The front matter's fields: each top-level `key: value` line with the
/// indented (or `- `) lines after it, keyed by its name. Other lines (e.g.
/// comments) have no key.
fn fields(text: &str) -> Vec<(Option<String>, String)> {
    let mut fields: Vec<(Option<String>, String)> = Vec::new();
    for line in text.trim_end().lines() {
        let continues = line.starts_with([' ', '\t']) || line.starts_with("- ") || line == "-";
        if let Some((_, text)) = fields.last_mut().filter(|(key, _)| continues && key.is_some()) {
            text.push('\n');
            text.push_str(line.trim_end());
            continue;
        }
        let key = line
            .split_once(':')
            .map(|(key, _)| key)
            .filter(|key| !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-'));
        fields.push((key.map(str::to_string), line.trim_end().to_string()));
    }
    fields
}

/// The value of a one-line `key: value` field, unquoted.
fn value(field: &str) -> String {
    unquote(field.split_once(':').map_or("", |(_, value)| value))
}

/// The value of the field `key` in YAML front matter, if it has one.
pub fn get(front_matter: &str, key: &str) -> Option<String> {
    fields(front_matter)
        .into_iter()
        .find(|(k, _)| k.as_deref() == Some(key))
        .map(|(_, text)| value(&text))
}

/// `text` as a double-quoted YAML string.
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars().filter(|c| !c.is_control()) {
        if matches!(c, '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// A one-line YAML value as text: `"double"` (with its escapes) and
/// `'single'` quotes undone, and a ` # comment` after a plain one dropped.
fn unquote(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(inner) = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(c) => out.push(c),
                None => {}
            }
        }
        return out;
    }
    if let Some(inner) = raw.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    raw.split(" #").next().unwrap_or_default().trim().to_string()
}

/// A file's front matter and the rest of it. YAML front matter (between
/// `---` lines) comes back as it is; TOML (between `+++` lines, as Zola
/// also takes) is turned into YAML. `None` if the file has none.
pub fn split(text: &str) -> Option<(String, &str)> {
    let fence = ["---", "+++"].into_iter().find(|fence| text.lines().next().map(str::trim_end) == Some(fence))?;
    let start = text.find('\n')? + 1;
    let mut at = start;
    for line in text[start..].split_inclusive('\n') {
        if line.trim_end() == fence {
            let inside = text[start..at].trim_end_matches(['\n', '\r']);
            let front_matter = if fence == "+++" { toml_to_yaml(inside) } else { inside.to_string() };
            return Some((front_matter, &text[at + line.len()..]));
        }
        at += line.len();
    }
    None
}

/// TOML front matter as YAML: `key = value` becomes `key: value`, and
/// `[tables]` become indented maps. Strings, numbers, dates and arrays are
/// written the same way in both. What doesn't convert (multi-line strings,
/// arrays of tables) is kept as a comment, to fix by hand.
fn toml_to_yaml(toml: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    // The table the lines are in, e.g. ["extra", "links"].
    let mut table: Vec<String> = Vec::new();
    for line in toml.lines() {
        let trimmed = line.trim();
        let indent = "  ".repeat(table.len());
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push(if trimmed.is_empty() { String::new() } else { format!("{indent}{trimmed}") });
            continue;
        }
        if let Some(name) = trimmed.strip_prefix('[').and_then(|r| r.strip_suffix(']')).filter(|n| !n.starts_with('[')) {
            let path: Vec<String> = name.split('.').map(|part| part.trim().trim_matches('"').to_string()).collect();
            let shared = table.iter().zip(&path).take_while(|(a, b)| a == b).count();
            for (depth, part) in path.iter().enumerate().skip(shared) {
                out.push(format!("{}{part}:", "  ".repeat(depth)));
            }
            table = path;
            continue;
        }
        let converted = trimmed.split_once('=').and_then(|(key, value)| {
            let (key, value) = (key.trim(), value.trim());
            let multi_line = value.starts_with("\"\"\"") || value.starts_with("'''");
            let open_array = value.starts_with('[') && !value.ends_with(']');
            if multi_line || open_array || key.is_empty() {
                return None;
            }
            Some(format!("{indent}{}: {}", key.trim_matches('"'), toml_value(value)))
        });
        out.push(converted.unwrap_or_else(|| format!("{indent}# {trimmed}")));
    }
    out.join("\n")
}

/// A TOML value as YAML. Literal 'strings' become YAML single-quoted ones;
/// inline tables `{ a = 1 }` become `{ a: 1 }`; the rest is the same.
fn toml_value(value: &str) -> String {
    if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        return format!("'{}'", inner.replace('\'', "''"));
    }
    if value.starts_with('{') {
        return value.replace(" = ", ": ");
    }
    value.to_string()
}

/// The start of the post's text (after the title), without its markdown,
/// cut at a word to at most `DESCRIPTION_CHARS` characters.
pub fn description(body: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut in_code = false;
    for line in body.lines().skip(1) {
        let line = line.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        let rule = line.len() >= 3 && line.chars().all(|c| matches!(c, '-' | '*' | '_' | ' '));
        if in_code || rule || line.starts_with('|') {
            continue;
        }
        let line = line.trim_start_matches(['#', '>', ' ']);
        words.extend(plain(strip_list_marker(line)).split_whitespace().map(str::to_string));
    }
    let mut out = String::new();
    for word in words {
        let len = out.chars().count() + usize::from(!out.is_empty()) + word.chars().count();
        if len > DESCRIPTION_CHARS {
            let cut = if out.is_empty() { word.chars().take(DESCRIPTION_CHARS - 1).collect() } else { out };
            return format!("{}…", cut.trim_end_matches([',', ';', ':', '.', '-']));
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&word);
    }
    out
}

fn strip_list_marker(line: &str) -> &str {
    for marker in ["- [ ] ", "- [x] ", "- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return rest;
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    match line[digits..].strip_prefix(". ").or_else(|| line[digits..].strip_prefix(") ")) {
        Some(rest) if digits > 0 => rest,
        _ => line,
    }
}

/// A line of markdown as plain text: links and images become their text,
/// and emphasis and code markers go.
fn plain(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(start) = rest.find('[') {
        let (before, after) = rest.split_at(start);
        let link = after[1..]
            .split_once("](")
            .and_then(|(text, tail)| tail.split_once(')').map(|(_, tail)| (text, tail)));
        match link {
            Some((text, tail)) if !text.contains(']') => {
                out.push_str(before.strip_suffix('!').unwrap_or(before));
                out.push_str(text);
                rest = tail;
            }
            _ => {
                out.push_str(before);
                out.push('[');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").replace("__", "").replace("~~", "").replace(['*', '`'], "")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn suggestions_come_from_the_post() {
        let today = Date::new(2026, 10, 8).unwrap();
        let body = "# Say \"hi\"\n\nSome **bold** and [a link](https://x.y), `code`.\n\n```\nskipped\n```\n- A list item";
        assert_eq!(
            suggest("Say \"hi\"", body, today),
            "title: \"Say \\\"hi\\\"\"\ndate: 2026-10-08\nupdated: 2026-10-08\n\
             description: \"Some bold and a link, code. A list item\"\nslug: say-hi"
        );
    }

    #[test]
    fn descriptions_are_cut_at_a_word() {
        let body = format!("# T\n{}", "word ".repeat(100));
        let text = description(&body);
        assert!(text.ends_with("word…"), "{text}");
        assert!(text.chars().count() <= DESCRIPTION_CHARS + 1);
        assert_eq!(description("# T\n> Quoted, *nicely*.\n\n1. First\n---\n| a | b |"), "Quoted, nicely. First");
        assert_eq!(description("# Only a title"), "");
    }

    fn settings(front_matter: &str, suggested: &str) -> ExportSettings {
        ExportSettings {
            path: PathBuf::from("/x.md"),
            front_matter: front_matter.into(),
            suggested: suggested.into(),
            with_front_matter: true,
        }
    }

    #[test]
    fn front_matter_remembers_what_was_changed_by_hand() {
        let then = "title: \"Old\"\ndate: 2026-01-01\nupdated: 2026-01-01\ndescription: \"Old text\"\nslug: old";
        let now = "title: \"New\"\ndate: 2026-10-08\nupdated: 2026-10-08\ndescription: \"New text\"\nslug: new";
        // Nothing remembered: the suggestion.
        assert_eq!(front_matter(now, None, true), now);
        assert_eq!(front_matter(now, Some(&settings(" ", then)), true), now);
        // Unchanged fields follow the post; `date` stays; added ones stay.
        let typed = format!("{then}\ntaxonomies:\n  tags: [a, b]\n# a comment");
        assert_eq!(
            front_matter(now, Some(&settings(&typed, then)), true),
            "title: \"New\"\ndate: 2026-01-01\nupdated: 2026-10-08\ndescription: \"New text\"\nslug: new\n\
             taxonomies:\n  tags: [a, b]\n# a comment"
        );
        // Quoted differently is still unchanged.
        let typed = "title: Old\nslug: 'old'";
        assert_eq!(front_matter(now, Some(&settings(typed, then)), true), "title: \"New\"\nslug: new");
        // Changed by hand, reordered or removed: kept so.
        let typed = "slug: mine\ntitle: \"Mine\"\ndate: 2026-01-01\nupdated: 2026-01-01";
        assert_eq!(
            front_matter(now, Some(&settings(typed, then)), true),
            "slug: mine\ntitle: \"Mine\"\ndate: 2026-01-01\nupdated: 2026-10-08"
        );
        // From another post: its own fields are this post's, the rest stays.
        let typed = "title: \"Mine\"\ndate: 2026-01-01\nauthor: Me\ntags:\n- a\n- b";
        assert_eq!(
            front_matter(now, Some(&settings(typed, then)), false),
            "title: \"New\"\ndate: 2026-10-08\nauthor: Me\ntags:\n- a\n- b"
        );
    }

    #[test]
    fn fields_not_suggested_before_are_added() {
        let now = "title: \"New\"\ndate: 2026-10-08\nupdated: 2026-10-08\ndescription: \"New text\"\nslug: new";
        // Imported with only a title and a date, and something else.
        let file = "title: \"New\"\ndate: 2025-05-05\ntags: [a]";
        let then = suggested_of(now, file);
        assert_eq!(then, "title: \"New\"\ndate: 2026-10-08");
        assert_eq!(
            front_matter(now, Some(&settings(file, &then)), true),
            "title: \"New\"\ndate: 2025-05-05\nupdated: 2026-10-08\ndescription: \"New text\"\nslug: new\ntags: [a]"
        );
        // None of them: at the start.
        let file = "tags: [a]";
        assert_eq!(front_matter(now, Some(&settings(file, &suggested_of(now, file))), true), format!("{now}\ntags: [a]"));
    }

    #[test]
    fn values_are_read_back() {
        let text = "title: \"Say \\\"hi\\\"\"\nslug: 'it''s'\nplain: words # note\nlist:\n  - a";
        assert_eq!(get(text, "title").as_deref(), Some("Say \"hi\""));
        assert_eq!(get(text, "slug").as_deref(), Some("it's"));
        assert_eq!(get(text, "plain").as_deref(), Some("words"));
        assert_eq!(get(text, "missing"), None);
    }

    #[test]
    fn front_matter_is_split_off() {
        assert_eq!(split("---\ntitle: Hi\n---\n\nText"), Some(("title: Hi".to_string(), "\nText")));
        assert_eq!(split("---\r\ntitle: Hi\r\n---\r\nText"), Some(("title: Hi".to_string(), "Text")));
        assert_eq!(split("---\n---\nText"), Some((String::new(), "Text")));
        // Not closed, or not at the very start: not front matter.
        assert_eq!(split("---\ntitle: Hi\nText"), None);
        assert_eq!(split("# Title\n---\na: b\n---"), None);
    }

    #[test]
    fn toml_becomes_yaml() {
        let toml = "title = \"Hello, world\"\ndate = 2026-10-08\nlit = 'it's'\ndraft = false\n\
                    tags = [\"a\", \"b\"]\n# note\n\n[taxonomies]\ncategories = [\"x\"]\n\
                    [extra.social]\nimage = { src = \"a.png\" }\n[extra.more]\nbody = \"\"\"\nmulti\n\"\"\"";
        let file = format!("+++\n{toml}\n+++\nText");
        let (yaml, rest) = split(&file).unwrap();
        assert_eq!(rest, "Text");
        assert_eq!(
            yaml,
            "title: \"Hello, world\"\ndate: 2026-10-08\nlit: 'it''s'\ndraft: false\ntags: [\"a\", \"b\"]\n# note\n\n\
             taxonomies:\n  categories: [\"x\"]\nextra:\n  social:\n    image: { src: \"a.png\" }\n  more:\n\
             \x20   # body = \"\"\"\n    # multi\n    # \"\"\""
        );
    }
}
