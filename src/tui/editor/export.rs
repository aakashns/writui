//! Exporting a post as a markdown file: where it goes, and writing it.

use std::path::{Path, PathBuf};

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

/// Where an export goes unless changed: the post's file name in the folder
/// writui was started from.
pub fn default_path(title: &str) -> String {
    let dir = std::env::current_dir().unwrap_or_default();
    display(&dir.join(file_name(title)))
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

/// The file a typed path means: `~` is the home folder, a relative path is
/// from the folder writui was started from, and a folder means the post's
/// file name in that folder. The folder has to exist.
pub fn resolve(input: &str, title: &str) -> Result<PathBuf, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Type where to export the post.".into());
    }
    let mut path = match (input.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(input),
    };
    if path.is_relative() {
        path = std::env::current_dir().map_err(|e| format!("Couldn't find the current folder: {e}"))?.join(path);
    }
    if path.is_dir() || input.ends_with('/') {
        path = path.join(file_name(title));
    }
    match path.parent() {
        Some(dir) if dir.is_dir() => Ok(path),
        Some(dir) => Err(format!("There's no folder {}.", display(dir))),
        None => Err("That isn't a file.".into()),
    }
}

/// Write the post's markdown, ending with a newline as text files do.
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    let mut text = text.to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    std::fs::write(path, text).map_err(|e| format!("Couldn't export to {}: {e}", display(path)))
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
    }

    #[test]
    fn missing_folders_and_empty_paths_are_refused() {
        assert!(resolve("", "t").is_err());
        assert!(resolve("/no/such/folder/x.md", "t").is_err());
    }

    #[test]
    fn writes_end_with_a_newline() {
        let path = std::env::temp_dir().join(format!("writui-export-{}.md", std::process::id()));
        write(&path, "# Title\nText").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Title\nText\n");
        std::fs::remove_file(path).unwrap();
    }
}
