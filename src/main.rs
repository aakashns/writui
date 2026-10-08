mod config;
mod tui;
mod upgrade;
mod vault;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// A terminal writing app. Markdown, encrypted, no nonsense.
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
        Some(Command::Migrate) => Ok(()),
    }
}
