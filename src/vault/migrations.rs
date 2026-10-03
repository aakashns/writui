//! Schema migrations. Append new migrations to the end of `MIGRATIONS`;
//! never edit or reorder existing ones — real vaults have already run them.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
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
    // 2: Recently Deleted. Set when a post is deleted; cleared on restore.
    "ALTER TABLE posts ADD COLUMN deleted_at INTEGER;",
];

/// Bring the vault's schema up to date, backing up the file first if it
/// already holds data.
pub(super) fn run(conn: &mut Connection, path: &Path) -> Result<()> {
    run_list(conn, path, MIGRATIONS)
}

fn run_list(conn: &mut Connection, path: &Path, migrations: &[&str]) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let current = usize::try_from(current).context("invalid schema version")?;
    if current > migrations.len() {
        bail!("this vault was written by a newer version of writui; update writui to open it");
    }
    if current == migrations.len() {
        return Ok(());
    }
    if current > 0 {
        backup(path, current)?;
    }
    for (index, sql) in migrations.iter().enumerate().skip(current) {
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
    let stamp = jiff::Zoned::now().strftime("%Y%m%d-%H%M%S");
    let name = path.file_name().context("vault path has no file name")?;
    let backup = path.with_file_name(format!("{}.v{version}-{stamp}.bak", name.to_string_lossy()));
    fs::copy(path, &backup).with_context(|| format!("backing up vault to {}", backup.display()))?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrating_an_existing_vault_backs_it_up_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("writui.db");
        let mut conn = Connection::open(&path).unwrap();

        run_list(&mut conn, &path, &MIGRATIONS[..1]).unwrap();
        conn.execute("INSERT INTO posts (body, created_at, updated_at) VALUES ('# Hi', 0, 0)", [])
            .unwrap();
        // A fresh vault has nothing to back up.
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);

        run_list(&mut conn, &path, MIGRATIONS).unwrap();
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        let backups: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with("writui.db.v1-") && name.ends_with(".bak"))
            .collect();
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(dir.path().join(&backups[0])).unwrap();
        let body: String = backup.query_row("SELECT body FROM posts", [], |r| r.get(0)).unwrap();
        assert_eq!(body, "# Hi");
    }

    #[test]
    fn refuses_vaults_from_newer_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("writui.db");
        let mut conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        assert!(run(&mut conn, &path).is_err());
    }
}
