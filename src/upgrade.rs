//! `writui upgrade`: replace this binary with the latest GitHub release.
//!
//! Downloads the same files as `install.sh` (the binary for this platform and
//! `SHA256SUMS`), checks the binary against its checksum, and swaps it in
//! with a rename, so writui is never left half-written. The vault isn't
//! touched by the old binary: once the new one is in place, it's run as
//! `writui migrate` to update the vault (with a backup) right away. If that
//! doesn't happen, the vault is updated on the next unlock instead.

use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::config;
use crate::vault::{OpenError, Vault};

const REPO: &str = "aakashns/writui";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Release binaries are about 10 MB; this only guards against nonsense.
const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

pub fn run(yes: bool, db: Option<PathBuf>) -> Result<()> {
    if cfg!(debug_assertions) {
        bail!("upgrade is turned off in debug builds, since it would replace the binary in target/");
    }
    let Some(binary) = binary_name() else {
        bail!(
            "there are no ready-made writui builds for this system; see \
             https://github.com/{REPO}#install"
        );
    };
    let exe = env::current_exe()
        .and_then(fs::canonicalize)
        .context("finding the running writui")?;

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(300)))
        .build()
        .into();

    let release: Release = agent
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .and_then(|mut response| response.body_mut().read_json())
        .context("checking GitHub for the latest release")?;
    let latest = release.tag_name.trim_start_matches('v');
    if parse_version(latest)? <= parse_version(VERSION)? {
        println!("writui {VERSION} is up to date.");
        return Ok(());
    }

    println!("writui {VERSION} → {latest}");
    println!("What's new: {}", release.html_url);
    if !yes && !confirm("Upgrade? [y/N] ")? {
        println!("Not upgraded.");
        return Ok(());
    }

    let base = format!("https://github.com/{REPO}/releases/download/{}", release.tag_name);
    println!("Downloading {binary}...");
    let bytes = download(&agent, &format!("{base}/{binary}"))?;
    let sums = String::from_utf8(download(&agent, &format!("{base}/SHA256SUMS"))?)
        .context("reading SHA256SUMS")?;
    let expected = checksum_for(&sums, binary).context("SHA256SUMS doesn't list this binary")?;
    if hex(&Sha256::digest(&bytes)) != expected {
        bail!("the download doesn't match its checksum; please try again");
    }

    replace(&exe, &bytes)?;
    println!("Upgraded {} to writui {latest}.", exe.display());

    // Let the new version bring the vault up to date now, while we're here.
    let mut migrate = Command::new(&exe);
    if let Some(db) = &db {
        migrate.arg("--db").arg(db);
    }
    if !matches!(migrate.arg("migrate").status(), Ok(status) if status.success()) {
        println!(
            "Your vault wasn't updated now; writui updates it, after making a backup, the \
             next time you unlock it."
        );
    }
    Ok(())
}

/// `writui migrate`: ask for the password and open the vault, which updates
/// it to this version's format (backing it up first, and deleting the backup
/// once the updated vault checks out). Leaving the password empty skips it.
/// Always fine to skip: the next unlock does the same.
pub fn migrate(db: Option<PathBuf>) -> Result<()> {
    let path = config::vault_path(db)?;
    if !path.exists() || !io::stdin().is_terminal() {
        return Ok(());
    }
    println!("Updating your vault ({}).", path.display());
    for _ in 0..3 {
        let password = Zeroizing::new(
            rpassword::prompt_password("Vault password (leave empty to do it on next unlock): ")
                .context("reading the password")?,
        );
        if password.is_empty() {
            bail!("skipped");
        }
        match Vault::open(&path, &password) {
            Ok(_) => {
                println!("Vault is up to date.");
                return Ok(());
            }
            Err(OpenError::WrongPassword) => println!("Wrong password."),
            Err(err) => bail!("couldn't update the vault: {err}"),
        }
    }
    bail!("too many wrong passwords")
}

/// The release file for this computer, as named by the release workflow.
fn binary_name() -> Option<&'static str> {
    match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => Some("writui-macos-arm64"),
        ("macos", "x86_64") => Some("writui-macos-x86_64"),
        ("linux", "aarch64") => Some("writui-linux-arm64"),
        ("linux", "x86_64") => Some("writui-linux-x86_64"),
        _ => None,
    }
}

/// `1.2.3` → `(1, 2, 3)`, for comparing versions.
fn parse_version(version: &str) -> Result<(u64, u64, u64)> {
    let parts: Vec<u64> = version
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .with_context(|| format!("unexpected version number {version:?}"))?;
    match parts[..] {
        [major, minor, patch] => Ok((major, minor, patch)),
        _ => bail!("unexpected version number {version:?}"),
    }
}

fn confirm(question: &str) -> Result<bool> {
    if !io::stdin().is_terminal() {
        bail!("can't ask for confirmation here; run `writui upgrade --yes` to upgrade");
    }
    print!("{question}");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn download(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>> {
    agent
        .get(url)
        .call()
        .and_then(|mut response| response.body_mut().with_config().limit(MAX_DOWNLOAD).read_to_vec())
        .with_context(|| format!("downloading {url}"))
}

/// The checksum listed for `name` in a `sha256sum`-style file.
fn checksum_for<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        (file.trim_start().trim_start_matches('*') == name).then_some(hash)
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Write the new binary next to the old one, then rename it over the top.
fn replace(exe: &Path, bytes: &[u8]) -> Result<()> {
    let dir = exe.parent().context("the running writui has no folder")?;
    let new = dir.join(format!(".writui-upgrade-{}", process::id()));
    let result = write_executable(&new, bytes).and_then(|()| fs::rename(&new, exe));
    if let Err(err) = result {
        let _ = fs::remove_file(&new);
        if err.kind() == io::ErrorKind::PermissionDenied {
            bail!(
                "can't write to {}; upgrade with the install script instead, or rerun with \
                 permission to change that folder",
                dir.display()
            );
        }
        return Err(err).with_context(|| format!("replacing {}", exe.display()));
    }
    Ok(())
}

fn write_executable(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(parse_version("0.10.0").unwrap() > parse_version("0.9.1").unwrap());
        assert!(parse_version("1.0.0").unwrap() > parse_version("0.99.99").unwrap());
        assert_eq!(parse_version("0.2.0").unwrap(), (0, 2, 0));
        assert!(parse_version("0.2").is_err());
        assert!(parse_version("0.2.0-beta").is_err());
    }

    #[test]
    fn finds_the_checksum_for_a_binary() {
        let sums = "aaa  writui-linux-arm64\nbbb  writui-macos-arm64\nccc *writui-macos-x86_64\n";
        assert_eq!(checksum_for(sums, "writui-macos-arm64"), Some("bbb"));
        assert_eq!(checksum_for(sums, "writui-macos-x86_64"), Some("ccc"));
        assert_eq!(checksum_for(sums, "writui-macos"), None);
    }

    #[test]
    fn replaces_the_binary() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("writui");
        fs::write(&exe, "old").unwrap();
        replace(&exe, b"new").unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"new");
        // No temporary file left behind.
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
