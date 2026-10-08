//! Opening links in the browser (or the mail app, for `mailto:`).

use std::process::{Command, Stdio};

/// Open `url` with the system's handler for it. Only web and email links:
/// anything else (a file, an app) could run something.
pub fn open(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !["http://", "https://", "mailto:"].iter().any(|scheme| lower.starts_with(scheme)) {
        return Err("Only web and email links open.".into());
    }
    if cfg!(test) {
        return Ok(());
    }
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut command = Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    } else {
        Command::new("xdg-open")
    };
    let mut child = command
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Couldn't open the link: {e}"))?;
    // Reap it once it's done, without waiting here.
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_and_email_links_open() {
        assert!(open("https://example.com").is_ok());
        assert!(open("HTTP://example.com").is_ok());
        assert!(open("mailto:me@example.com").is_ok());
        assert!(open("file:///Applications/Calculator.app").is_err());
        assert!(open("/bin/sh").is_err());
        assert!(open("notes.md").is_err());
    }
}
