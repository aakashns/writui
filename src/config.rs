//! The small plain config file that lives outside the vault.
//!
//! Only things needed *before* unlocking belong here (currently just where
//! the vault is). Everything else is a setting inside the vault.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    /// Path to the vault file. `~` is expanded.
    vault: Option<PathBuf>,
}

/// Debug builds keep everything in the repo's `.dev-data/`, so working on
/// writui can never touch real writing.
fn dev_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.dev-data"))
}

fn config_path() -> Result<PathBuf> {
    if cfg!(debug_assertions) {
        return Ok(dev_dir().join("config.toml"));
    }
    let home = dirs::home_dir().context("couldn't find your home directory")?;
    Ok(home.join(".config/writui/config.toml"))
}

fn default_vault_path() -> Result<PathBuf> {
    if cfg!(debug_assertions) {
        return Ok(dev_dir().join("writui.db"));
    }
    let data = dirs::data_dir().context("couldn't find your data directory")?;
    Ok(data.join("writui/writui.db"))
}

fn load(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Where the vault lives: the `--db` flag, else the config file, else the
/// default location.
pub fn vault_path(flag: Option<PathBuf>) -> Result<PathBuf> {
    let path = match flag {
        Some(path) => path,
        None => match load(&config_path()?)?.vault {
            Some(path) => path,
            None => default_vault_path()?,
        },
    };
    Ok(expand_tilde(path))
}

fn expand_tilde(path: PathBuf) -> PathBuf {
    match (path.strip_prefix("~"), dirs::home_dir()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path,
    }
}
