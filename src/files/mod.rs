//! Markdown files outside the vault: exporting a post to one, importing one
//! as a post, and the paths typed to name them.

pub mod front_matter;

use std::path::{Path, PathBuf};

use crate::vault::{ExportSettings, TITLE_PREFIX};

/// The largest file that can be imported.
const MAX_IMPORT_BYTES: u64 = 10 * 1024 * 1024;

/// A file name for a post: its title in lowercase, words joined by dashes,
/// e.g. "Morning pages, again" → "morning-pages-again.md".
pub fn file_name(title: &str) -> String {
    let mut name = String::new();
    for ch in title.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            name.push(ch);
        } else if !name.is_empty() && !name.ends_with('-') {
            name.push('-');
        }
    }
    let name: String = name.chars().take(80).collect();
    match name.trim_end_matches('-') {
        "" => "untitled.md".into(),
        name => format!("{name}.md"),
    }
}

/// Where an export goes unless changed: the post's own file (where it was
/// last exported to, or imported from); failing that, the post's file name
/// in the folder of the latest export, or else in the folder writui was
/// started from.
pub fn default_path(title: &str, this_post: Option<&ExportSettings>, latest: Option<&ExportSettings>) -> String {
    if let Some(last) = this_post {
        return display(&last.path);
    }
    display(&last_folder(latest).join(file_name(title)))
}

/// The folder of the latest export or import, or else the one writui was
/// started from.
pub fn last_folder(latest: Option<&ExportSettings>) -> PathBuf {
    match latest.and_then(|latest| latest.path.parent()) {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir().unwrap_or_default(),
    }
}

/// A path as shown to the person, with their home folder as `~`.
pub fn display(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        return Path::new("~").join(rest).display().to_string();
    }
    path.display().to_string()
}

/// A folder as shown to the person, ending in `/` so typing goes into it.
pub fn display_folder(dir: &Path) -> String {
    let text = display(dir);
    if text.ends_with('/') { text } else { format!("{text}/") }
}

/// The path typed: `~` is the home folder, and a relative path is from the
/// folder writui was started from.
fn absolute(input: &str) -> Result<PathBuf, String> {
    let path = match (input.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(input),
    };
    if path.is_relative() {
        let current = std::env::current_dir().map_err(|e| format!("Couldn't find the current folder: {e}"))?;
        return Ok(current.join(path));
    }
    Ok(path)
}

/// The file a typed export path means (see `absolute`). A folder means the
/// post's file name in that folder. The folder has to exist.
pub fn resolve(input: &str, title: &str) -> Result<PathBuf, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Type where to export the post.".into());
    }
    let mut path = absolute(input)?;
    if path.is_dir() || input.ends_with('/') {
        path = path.join(file_name(title));
    }
    match path.parent() {
        Some(dir) if dir.is_dir() => Ok(path),
        Some(dir) => Err(format!("There's no folder {}.", display(dir))),
        None => Err("That isn't a file.".into()),
    }
}

/// What a typed path could be completed to, as Tab does in a shell: the
/// files and folders starting with its last part, sorted, folders ending
/// in `/`. Hidden ones only once a `.` is typed. If none match exactly,
/// matching ignores case.
pub fn completions(input: &str) -> Vec<String> {
    let input = if input == "~" { "~/" } else { input };
    let (dir_part, prefix) = match input.rfind('/') {
        Some(i) => input.split_at(i + 1),
        None => ("", input),
    };
    let Ok(dir) = absolute(if dir_part.is_empty() { "." } else { dir_part }) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if name.starts_with('.') && !prefix.starts_with('.') {
                return None;
            }
            // Following links, so a link to a folder counts as one.
            Some(if entry.path().is_dir() { format!("{name}/") } else { name })
        })
        .collect();
    names.sort();
    let exact: Vec<&String> = names.iter().filter(|name| name.starts_with(prefix)).collect();
    let matches = if exact.is_empty() {
        let prefix = prefix.to_lowercase();
        names.iter().filter(|name| name.to_lowercase().starts_with(&prefix)).collect()
    } else {
        exact
    };
    matches.into_iter().map(|name| format!("{dir_part}{name}")).collect()
}

/// The longest start all of `items` share.
pub fn common_prefix(items: &[String]) -> String {
    let Some(first) = items.first() else { return String::new() };
    let mut prefix: Vec<char> = first.chars().collect();
    for item in &items[1..] {
        let shared = prefix.iter().zip(item.chars()).take_while(|(a, b)| **a == *b).count();
        prefix.truncate(shared);
    }
    prefix.into_iter().collect()
}

/// The file's contents: the post's markdown, ending with a newline as text
/// files do. With front matter, that goes first, between `---` lines, and
/// takes the place of the `# Title` line unless `keep_title`.
pub fn contents(body: &str, front_matter: Option<&str>, keep_title: bool) -> String {
    let mut text = match front_matter.map(str::trim).filter(|text| !text.is_empty()) {
        Some(front_matter) => {
            let skip = usize::from(!keep_title);
            let rest = body.lines().skip(skip).skip_while(|line| line.trim().is_empty()).collect::<Vec<_>>().join("\n");
            if rest.is_empty() {
                format!("---\n{front_matter}\n---\n")
            } else {
                format!("---\n{front_matter}\n---\n\n{rest}")
            }
        }
        None => body.to_string(),
    };
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// Write the file.
pub fn write(path: &Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| format!("Couldn't export to {}: {e}", display(path)))
}

/// A markdown file, read to become a post.
#[derive(Debug, PartialEq)]
pub struct Imported {
    /// Where it was read from, in full.
    pub path: PathBuf,
    pub title: String,
    /// The post's text: the `# Title` line, then the file's text.
    pub body: String,
    /// The file's front matter, as YAML (empty if it had none).
    pub front_matter: String,
    pub had_front_matter: bool,
    /// Its front matter was TOML, turned into YAML.
    pub converted: bool,
    /// It had front matter, and the `# Title` line too.
    pub had_title_line: bool,
}

/// The file a typed import path means (see `absolute`).
pub fn resolve_import(input: &str) -> Result<PathBuf, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Type the path of a markdown file.".into());
    }
    let path = absolute(input)?;
    if path.is_dir() {
        return Err("That's a folder. Tab lists what's in it.".into());
    }
    if !path.is_file() {
        return Err("There's no such file.".into());
    }
    Ok(path)
}

/// Read a markdown file to import. The title comes from its front matter,
/// or else its first line if that's a `# Title`, or else its file name.
pub fn read(path: &Path) -> Result<Imported, String> {
    let shown = display(path);
    let size = std::fs::metadata(path).map_err(|e| format!("Couldn't read {shown}: {e}"))?.len();
    if size > MAX_IMPORT_BYTES {
        return Err(format!("{shown} is too big to be a post."));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {shown}: {e}"))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{shown} isn't a text file."))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text).replace("\r\n", "\n");
    if text.contains('\0') {
        return Err(format!("{shown} isn't a text file."));
    }
    let toml = text.starts_with("+++");
    let (front_matter, rest) = match front_matter::split(&text) {
        Some((front_matter, rest)) => (Some(front_matter), rest),
        None => (None, text.as_str()),
    };
    let rest = rest.trim_start_matches(['\n', ' ', '\t']).trim_end();
    // A `# Title` first line is the title, unless the front matter has
    // another: then it's a heading in the text.
    let heading = rest.lines().next().and_then(|line| line.strip_prefix(TITLE_PREFIX)).map(str::trim);
    let from_front_matter = front_matter.as_deref().and_then(|f| front_matter::get(f, "title")).filter(|t| !t.is_empty());
    let title = from_front_matter.clone().or(heading.map(str::to_string)).unwrap_or_else(|| title_from_file_name(path));
    let title_line = heading.is_some_and(|heading| from_front_matter.is_none() || heading == title);
    let rest = match heading {
        Some(_) if title_line => {
            rest.split_once('\n').map_or("", |(_, rest)| rest).trim_start_matches(['\n', ' ', '\t'])
        }
        _ => rest,
    };
    let title: String = title.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let body = if rest.is_empty() { format!("{TITLE_PREFIX}{title}") } else { format!("{TITLE_PREFIX}{title}\n\n{rest}") };
    Ok(Imported {
        path: path.to_path_buf(),
        title,
        body,
        had_front_matter: front_matter.is_some(),
        converted: toml && front_matter.as_deref().is_some_and(|f| !f.is_empty()),
        had_title_line: title_line && front_matter.is_some(),
        front_matter: front_matter.unwrap_or_default(),
    })
}

/// "morning-pages.md" → "Morning pages".
fn title_from_file_name(path: &Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let words = stem.replace(['-', '_'], " ");
    let words = words.trim();
    let mut chars = words.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Whether two paths are the same file (following links).
pub fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_come_from_titles() {
        assert_eq!(file_name("Morning pages"), "morning-pages.md");
        assert_eq!(file_name("  What's next? (Part 2)  "), "what-s-next-part-2.md");
        assert_eq!(file_name("Café au lait"), "café-au-lait.md");
        assert_eq!(file_name(""), "untitled.md");
        assert_eq!(file_name("!!!"), "untitled.md");
    }

    #[test]
    fn folders_get_the_file_name() {
        let dir = std::env::temp_dir();
        assert_eq!(resolve(dir.to_str().unwrap(), "Hello world"), Ok(dir.join("hello-world.md")));
        assert_eq!(resolve(dir.join("x.md").to_str().unwrap(), "Hello"), Ok(dir.join("x.md")));
    }

    #[test]
    fn home_is_expanded() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(resolve("~/a.md", "t"), Ok(home.join("a.md")));
        assert_eq!(display(&home.join("a.md")), "~/a.md");
        assert_eq!(display_folder(&home.join("blog")), "~/blog/");
    }

    #[test]
    fn missing_folders_and_empty_paths_are_refused() {
        assert!(resolve("", "t").is_err());
        assert!(resolve("/no/such/folder/x.md", "t").is_err());
    }

    #[test]
    fn files_end_with_a_newline() {
        assert_eq!(contents("# Title\nText", None, false), "# Title\nText\n");
        assert_eq!(contents("# Title\nText\n", None, false), "# Title\nText\n");
    }

    #[test]
    fn front_matter_takes_the_place_of_the_title() {
        let fm = "title: \"Title\"";
        assert_eq!(contents("# Title\n\n\nText\n\nMore", Some(fm), false), "---\ntitle: \"Title\"\n---\n\nText\n\nMore\n");
        assert_eq!(contents("# Title", Some(fm), false), "---\ntitle: \"Title\"\n---\n");
        // Or kept, after it.
        assert_eq!(contents("# Title\n\nText", Some(fm), true), "---\ntitle: \"Title\"\n---\n\n# Title\n\nText\n");
        // Empty front matter is none at all.
        assert_eq!(contents("# Title\nText", Some(" \n"), false), "# Title\nText\n");
    }

    fn settings(path: PathBuf) -> ExportSettings {
        ExportSettings {
            path,
            front_matter: String::new(),
            suggested: String::new(),
            with_front_matter: true,
            keep_title: false,
        }
    }

    #[test]
    fn last_exports_decide_the_path() {
        let home = dirs::home_dir().unwrap();
        let last = settings(home.join("blog/old-name.md"));
        assert_eq!(default_path("New name", Some(&last), None), "~/blog/old-name.md");
        assert_eq!(default_path("New name", None, Some(&last)), "~/blog/new-name.md");
        assert!(default_path("New name", None, None).ends_with("/new-name.md"));
    }

    #[test]
    fn paths_complete_like_in_a_shell() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().to_str().unwrap();
        std::fs::create_dir(dir.path().join("posts")).unwrap();
        std::fs::create_dir(dir.path().join("Pictures")).unwrap();
        std::fs::write(dir.path().join("post.md"), "").unwrap();
        std::fs::write(dir.path().join(".hidden"), "").unwrap();
        assert_eq!(completions(&format!("{base}/po")), [format!("{base}/post.md"), format!("{base}/posts/")]);
        assert_eq!(common_prefix(&completions(&format!("{base}/po"))), format!("{base}/post"));
        assert_eq!(completions(&format!("{base}/Pi")), [format!("{base}/Pictures/")]);
        // Ignoring case only when nothing matches exactly.
        assert_eq!(completions(&format!("{base}/pi")), [format!("{base}/Pictures/")]);
        assert_eq!(completions(&format!("{base}/")).len(), 3);
        assert_eq!(completions(&format!("{base}/.h")), [format!("{base}/.hidden")]);
        assert!(completions(&format!("{base}/nothing")).is_empty());
        assert!(completions("/no/such/folder/x").is_empty());
        assert!(completions("~").iter().all(|path| path.starts_with("~/")));
    }

    fn import(name: &str, text: &str) -> Imported {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, text).unwrap();
        read(&path).unwrap()
    }

    #[test]
    fn imports_take_the_title_from_front_matter_then_the_first_line_then_the_name() {
        let file = import("a.md", "---\ntitle: \"Hi\"\ndate: 2026-01-01\n---\n\n# Hi\n\nText\n");
        assert_eq!((file.title.as_str(), file.body.as_str()), ("Hi", "# Hi\n\nText"));
        assert_eq!(file.front_matter, "title: \"Hi\"\ndate: 2026-01-01");
        assert!(file.had_front_matter && file.had_title_line && !file.converted);
        // A different heading stays in the text.
        let file = import("a.md", "---\ntitle: Hi\n---\n# Part one\nText");
        assert_eq!(file.body, "# Hi\n\n# Part one\nText");
        assert!(!file.had_title_line);
        let file = import("a.md", "\u{feff}# Plain\r\n\r\nText\r\n");
        assert_eq!((file.body.as_str(), file.had_front_matter), ("# Plain\n\nText", false));
        let file = import("morning-pages.md", "Just text");
        assert_eq!(file.body, "# Morning pages\n\nJust text");
        let file = import("+++.md", "+++\ntitle = \"Toml\"\n+++\nText");
        assert_eq!((file.body.as_str(), file.front_matter.as_str(), file.converted), ("# Toml\n\nText", "title: \"Toml\"", true));
        assert_eq!(import("empty.md", "").body, "# Empty");
    }

    #[test]
    fn imports_refuse_what_isnt_a_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.png");
        std::fs::write(&path, [0x89, 0x50, 0xff, 0x00]).unwrap();
        assert!(read(&path).unwrap_err().contains("isn't a text file"));
        assert!(resolve_import(dir.path().to_str().unwrap()).unwrap_err().contains("a folder"));
        assert!(resolve_import("/no/such/file.md").unwrap_err().contains("no such file"));
        assert_eq!(resolve_import(path.to_str().unwrap()), Ok(path));
    }
}
