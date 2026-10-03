//! Schema migrations. Append new migrations to the end of `MIGRATIONS`;
//! never edit or reorder existing ones — real vaults have already run them.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use jiff::Timestamp;
use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    // 1: posts. `body` is the current draft; times are unix milliseconds.
    "CREATE TABLE posts (
        id         INTEGER PRIMARY KEY,
        body       TEXT    NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX posts_updated_at ON posts (updated_at);",
];

/// Bring the vault's schema up to date, backing up the file first if it
/// already holds data.
pub(super) fn run(conn: &mut Connection, path: &Path) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let current = usize::try_from(current).context("invalid schema version")?;
    if current > MIGRATIONS.len() {
        bail!("this vault was written by a newer version of writui; update writui to open it");
    }
    if current == MIGRATIONS.len() {
        return Ok(());
    }
    if current > 0 {
        backup(path, current)?;
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)
            .with_context(|| format!("running migration {}", index + 1))?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// Copy the (still encrypted) vault file next to itself, e.g.
/// `writui.db.v1-20261003-142501.bak`.
fn backup(path: &Path, version: usize) -> Result<PathBuf> {
    let stamp = Timestamp::now().strftime("%Y%m%d-%H%M%S");
    let name = path.file_name().context("vault path has no file name")?;
    let backup = path.with_file_name(format!("{}.v{version}-{stamp}.bak", name.to_string_lossy()));
    fs::copy(path, &backup).with_context(|| format!("backing up vault to {}", backup.display()))?;
    Ok(backup)
}
