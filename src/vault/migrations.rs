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
    // 2: Trash. Set when a post is deleted; cleared on restore.
    "ALTER TABLE posts ADD COLUMN deleted_at INTEGER;",
    // 3: where the cursor was when the post was last closed, as a character
    // index into `body`. NULL means the end.
    "ALTER TABLE posts ADD COLUMN cursor INTEGER;",
    // 4: saves: named snapshots of a post's full text, made with Ctrl+S.
    "CREATE TABLE saves (
        id         INTEGER PRIMARY KEY,
        post_id    INTEGER NOT NULL REFERENCES posts (id),
        name       TEXT    NOT NULL,
        body       TEXT    NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX saves_post_id ON saves (post_id, created_at);",
    // 5: how each post was last exported, so the next export starts from
    // there: the file, and the front matter as typed and as writui
    // suggested it (to tell which fields were changed by hand).
    "CREATE TABLE exports (
        post_id           INTEGER PRIMARY KEY REFERENCES posts (id),
        path              TEXT    NOT NULL,
        front_matter      TEXT    NOT NULL,
        suggested         TEXT    NOT NULL,
        with_front_matter INTEGER NOT NULL,
        exported_at       INTEGER NOT NULL
    );
    CREATE INDEX exports_exported_at ON exports (exported_at);",
    // 6: whether an export with front matter kept the `# Title` line too.
    "ALTER TABLE exports ADD COLUMN keep_title INTEGER NOT NULL DEFAULT 0;",
];

/// Bring the vault's schema up to date. If the vault already holds data, it's
/// backed up first, and the backup is deleted once the updated vault passes
/// its integrity checks. If anything goes wrong, the backup is kept and the
/// error says where it is.
pub(super) fn run(conn: &mut Connection, path: &Path) -> Result<()> {
    run_list(conn, path, MIGRATIONS, verify)
}

fn run_list(
    conn: &mut Connection,
    path: &Path,
    migrations: &[&str],
    verify: fn(&Connection) -> Result<()>,
) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let current = usize::try_from(current).context("invalid schema version")?;
    if current > migrations.len() {
        bail!("this vault was written by a newer version of writui; update writui to open it");
    }
    if current == migrations.len() {
        return Ok(());
    }
    if current == 0 {
        // A brand new vault: nothing to back up or check.
        return apply(conn, migrations, current);
    }
    let backup = backup(path, current)?;
    match apply(conn, migrations, current).and_then(|()| verify(conn)) {
        Ok(()) => {
            // Not worth failing over: the vault itself is fine.
            let _ = fs::remove_file(&backup);
            Ok(())
        }
        Err(err) => Err(err.context(format!(
            "updating the vault failed; a copy from before the update is at {}",
            backup.display()
        ))),
    }
}

fn apply(conn: &mut Connection, migrations: &[&str], current: usize) -> Result<()> {
    for (index, sql) in migrations.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)
            .with_context(|| format!("running migration {}", index + 1))?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// SQLCipher's check that every page decrypts and is untampered (no rows
/// when all is well), then SQLite's own structural check (a single "ok").
fn verify(conn: &Connection) -> Result<()> {
    let cipher = pragma_rows(conn, "PRAGMA cipher_integrity_check")?;
    if !cipher.is_empty() {
        bail!("the vault failed its encryption check: {}", cipher.join("; "));
    }
    let structure = pragma_rows(conn, "PRAGMA integrity_check")?;
    if structure != ["ok"] {
        bail!("the vault failed its integrity check: {}", structure.join("; "));
    }
    Ok(())
}

fn pragma_rows(conn: &Connection, sql: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
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

    /// A vault with migration 1 done and one post in it, like one made by an
    /// older writui.
    fn old_vault(dir: &Path) -> (Connection, PathBuf) {
        let path = dir.join("writui.db");
        let mut conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "key", "test").unwrap();
        run_list(&mut conn, &path, &MIGRATIONS[..1], verify).unwrap();
        conn.execute("INSERT INTO posts (body, created_at, updated_at) VALUES ('# Hi', 0, 0)", [])
            .unwrap();
        (conn, path)
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn is_backup(name: &str) -> bool {
        name.starts_with("writui.db.v1-") && name.ends_with(".bak")
    }

    #[test]
    fn a_fresh_vault_is_not_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        old_vault(dir.path());
        assert_eq!(files(dir.path()), ["writui.db"]);
    }

    #[test]
    fn the_backup_is_deleted_after_a_verified_migration() {
        let dir = tempfile::tempdir().unwrap();
        let (mut conn, path) = old_vault(dir.path());

        // Check the backup exists while migrating, and holds the old data.
        fn backup_exists(conn: &Connection) -> Result<()> {
            let path = PathBuf::from(conn.path().unwrap());
            let dir = path.parent().unwrap();
            let backups: Vec<_> = files(dir).into_iter().filter(|n| is_backup(n)).collect();
            assert_eq!(backups.len(), 1);
            let backup = Connection::open(dir.join(&backups[0])).unwrap();
            backup.pragma_update(None, "key", "test").unwrap();
            let body: String = backup.query_row("SELECT body FROM posts", [], |r| r.get(0)).unwrap();
            assert_eq!(body, "# Hi");
            verify(conn)
        }
        run_list(&mut conn, &path, MIGRATIONS, backup_exists).unwrap();

        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        assert_eq!(files(dir.path()), ["writui.db"]);
    }

    #[test]
    fn the_backup_is_kept_if_the_check_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (mut conn, path) = old_vault(dir.path());
        let err = run_list(&mut conn, &path, MIGRATIONS, |_| bail!("corrupt")).unwrap_err();
        let backups: Vec<_> = files(dir.path()).into_iter().filter(|n| is_backup(n)).collect();
        assert_eq!(backups.len(), 1);
        assert!(format!("{err:#}").contains(&backups[0]));
    }

    #[test]
    fn the_backup_is_kept_if_a_migration_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (mut conn, path) = old_vault(dir.path());
        let broken = [MIGRATIONS[0], "NOT SQL"];
        let err = run_list(&mut conn, &path, &broken, verify).unwrap_err();
        let backups: Vec<_> = files(dir.path()).into_iter().filter(|n| is_backup(n)).collect();
        assert_eq!(backups.len(), 1);
        assert!(format!("{err:#}").contains(&backups[0]));
        // The failed migration was rolled back.
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn the_checks_pass_on_a_healthy_vault() {
        let dir = tempfile::tempdir().unwrap();
        let (conn, _path) = old_vault(dir.path());
        verify(&conn).unwrap();
    }

    #[test]
    fn the_checks_catch_a_damaged_vault() {
        let dir = tempfile::tempdir().unwrap();
        let (conn, path) = old_vault(dir.path());
        drop(conn);
        let mut bytes = fs::read(&path).unwrap();
        assert!(bytes.len() > 4096 + 200, "expected more than one page");
        bytes[4096 + 200] ^= 0xff;
        fs::write(&path, bytes).unwrap();

        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "key", "test").unwrap();
        assert!(verify(&conn).is_err());
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
