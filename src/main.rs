mod config;
mod tui;
mod vault;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

/// A terminal writing app. Markdown, encrypted, no nonsense.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Path to the vault file (overrides the config file).
    #[arg(long, value_name = "PATH")]
    db: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let vault_path = config::vault_path(cli.db)?;
    tui::run(vault_path)
}
