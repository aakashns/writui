mod config;
mod files;
mod tui;
mod upgrade;
mod vault;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// A terminal writing app. No nonsense. Markdown. Local. Encrypted.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Path to the vault file (overrides the config file).
    #[arg(long, value_name = "PATH")]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Upgrade writui to the latest release.
    Upgrade {
        /// Upgrade without asking first.
        #[arg(short, long)]
        yes: bool,
    },
    /// Show writui's version, and where its vault and config file are.
    Info,
    /// Does nothing. writui 0.5.0 to 0.9.0 run this on the new binary after
    /// upgrading; the vault is now updated the next time it's unlocked.
    #[command(hide = true)]
    Migrate,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            let vault_path = config::vault_path(cli.db)?;
            tui::run(vault_path)
        }
        Some(Command::Upgrade { yes }) => upgrade::run(yes),
        Some(Command::Info) => info(cli.db),
        Some(Command::Migrate) => Ok(()),
    }
}

/// `writui info`: the version, then where the vault and the config file
/// are, saying if one isn't there (yet).
fn info(db: Option<PathBuf>) -> Result<()> {
    let line = |name: &str, path: &std::path::Path| {
        let missing = if path.exists() { "" } else { " (not there yet)" };
        println!("{:<8}{}{missing}", format!("{name}:"), path.display());
    };
    println!("writui {}", env!("CARGO_PKG_VERSION"));
    line("Vault", &config::vault_path(db)?);
    line("Config", &config::config_path()?);
    Ok(())
}
