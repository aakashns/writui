//! The encrypted vault: one SQLCipher database file holding everything.
//!
//! Nothing in here knows about the TUI, so CLI subcommands can reuse it.
//!
//! Encryption: the password goes through Argon2id with a random 16-byte salt
//! to produce a 256-bit key, which is handed to SQLCipher as a raw key along
//! with the salt. SQLCipher stores that salt in the first 16 bytes of the
//! file, so the vault stays a single self-contained file.

mod migrations;

use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use argon2::{Algorithm, Argon2, Params};
use jiff::Timestamp;
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, params};
use zeroize::Zeroizing;

const SALT_LEN: usize = 16;
const KEY_LEN: usize = 32;

/// Argon2id cost: (memory in KiB, iterations, lanes). Changing this makes
/// existing vaults unopenable unless the old values are kept as a fallback.
#[cfg(not(test))]
const KDF_COST: (u32, u32, u32) = (64 * 1024, 3, 1);
#[cfg(test)]
const KDF_COST: (u32, u32, u32) = (8, 1, 1);

/// Every post starts with this: its first line is always the `# ` title.
pub const TITLE_PREFIX: &str = "# ";

/// Deleted posts stay in the Trash for this long.
pub const DELETED_RETENTION_DAYS: i64 = 30;

pub struct Vault {
    conn: Connection,
}

#[derive(Debug)]
pub enum OpenError {
    WrongPassword,
    Other(anyhow::Error),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::WrongPassword => write!(f, "wrong password"),
            OpenError::Other(err) => write!(f, "{err:#}"),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<anyhow::Error> for OpenError {
    fn from(err: anyhow::Error) -> Self {
        OpenError::Other(err)
    }
}

impl From<rusqlite::Error> for OpenError {
    fn from(err: rusqlite::Error) -> Self {
        OpenError::Other(err.into())
    }
}

#[derive(Debug, Clone)]
pub struct PostSummary {
    pub id: i64,
    pub title: String,
    pub updated_at: Timestamp,
    /// Set for posts in the Trash.
    pub deleted_at: Option<Timestamp>,
}

/// A named version of a post, saved with Ctrl+S. (Stored in the `saves`
/// table: they were called saves at first.)
#[derive(Debug, Clone)]
pub struct Version {
    pub id: i64,
    pub name: String,
    pub created_at: Timestamp,
    /// The full text; left empty in lists of versions.
    pub body: String,
}

/// How a post was last exported as markdown.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportSettings {
    pub path: PathBuf,
    /// The front matter as it was written (without the `---` lines).
    pub front_matter: String,
    /// The front matter writui suggested that time, to tell which fields
    /// were changed by hand.
    pub suggested: String,
    pub with_front_matter: bool,
    /// With front matter, the `# Title` line was kept too.
    pub keep_title: bool,
}

#[derive(Debug, Clone)]
pub struct Post {
    pub id: i64,
    pub body: String,
    /// Where the cursor was when the post was last closed (a character
    /// index into `body`). `None` means the end.
    pub cursor: Option<usize>,
    pub created_at: Timestamp,
}

impl Vault {
    /// Create a brand new vault. Fails if a file already exists at `path`.
    pub fn create(path: &Path, password: &str) -> Result<Vault> {
        if path.exists() {
            bail!("a vault already exists at {}", path.display());
        }
        if let Some(dir) = path.parent() {
            create_private_dir(dir)?;
        }
        let mut salt = [0u8; SALT_LEN];
        rand::fill(&mut salt);
        let mut conn = Connection::open(path)
            .with_context(|| format!("creating vault at {}", path.display()))?;
        make_private(path)?;
        apply_key(&conn, password, &salt)?;
        migrations::run(&mut conn, path)?;
        Ok(Vault { conn })
    }

    /// Open an existing vault with its password.
    pub fn open(path: &Path, password: &str) -> Result<Vault, OpenError> {
        let salt = read_salt(path)?;
        let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        apply_key(&conn, password, &salt)?;
        // SQLCipher only notices a wrong key on the first real read.
        match conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(())) {
            Ok(()) => {}
            Err(rusqlite::Error::SqliteFailure(err, _)) if err.code == ErrorCode::NotADatabase => {
                return Err(OpenError::WrongPassword);
            }
            Err(err) => return Err(err.into()),
        }
        migrations::run(&mut conn, path)?;
        let vault = Vault { conn };
        vault.purge_expired()?;
        Ok(vault)
    }

    /// All posts (not counting the Trash), most recently updated first.
    pub fn list_posts(&self) -> Result<Vec<PostSummary>> {
        self.summaries("deleted_at IS NULL ORDER BY updated_at DESC, id DESC")
    }

    /// Posts in the Trash, most recently deleted first.
    pub fn list_deleted(&self) -> Result<Vec<PostSummary>> {
        self.summaries("deleted_at IS NOT NULL ORDER BY deleted_at DESC, id DESC")
    }

    pub fn count_deleted(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM posts WHERE deleted_at IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    fn summaries(&self, filter_and_order: &str) -> Result<Vec<PostSummary>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT id, substr(body, 1, instr(body || char(10), char(10)) - 1),
                    updated_at, deleted_at
             FROM posts WHERE {filter_and_order}"
        ))?;
        let rows = stmt.query_map([], |row| {
            let first_line: String = row.get(1)?;
            let deleted_at: Option<i64> = row.get(3)?;
            Ok((row.get(0)?, first_line, row.get(2)?, deleted_at))
        })?;
        rows.map(|row| {
            let (id, first_line, updated_at, deleted_at) = row?;
            Ok(PostSummary {
                id,
                title: title_from_first_line(&first_line),
                updated_at: from_millis(updated_at)?,
                deleted_at: deleted_at.map(from_millis).transpose()?,
            })
        })
        .collect()
    }

    pub fn post(&self, id: i64) -> Result<Post> {
        let (body, cursor, created_at): (String, Option<i64>, i64) = self
            .conn
            .query_row("SELECT body, cursor, created_at FROM posts WHERE id = ?1", [id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .with_context(|| format!("loading post {id}"))?;
        let cursor = cursor.and_then(|pos| usize::try_from(pos).ok());
        Ok(Post { id, body, cursor, created_at: from_millis(created_at)? })
    }

    /// Create an empty post (just the `# ` title prefix). Returns its id.
    pub fn create_post(&self) -> Result<i64> {
        let now = now_millis();
        self.conn.execute(
            "INSERT INTO posts (body, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params![TITLE_PREFIX, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Replace a post's draft. Keeps the `# ` title prefix in place.
    pub fn update_post_body(&self, id: i64, body: &str) -> Result<()> {
        let body = with_title_prefix(body);
        let changed = self.conn.execute(
            "UPDATE posts SET body = ?1, updated_at = ?2 WHERE id = ?3",
            params![body, now_millis(), id],
        )?;
        if changed == 0 {
            bail!("post {id} doesn't exist");
        }
        Ok(())
    }

    /// Remember where the cursor was in a post. Doesn't count as an update.
    pub fn set_post_cursor(&self, id: i64, cursor: usize) -> Result<()> {
        self.conn.execute(
            "UPDATE posts SET cursor = ?1 WHERE id = ?2",
            params![i64::try_from(cursor)?, id],
        )?;
        Ok(())
    }

    /// Move a post to the Trash.
    pub fn delete_post(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE posts SET deleted_at = ?1 WHERE id = ?2",
            params![now_millis(), id],
        )?;
        Ok(())
    }

    /// Bring a post back from the Trash.
    pub fn restore_post(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE posts SET deleted_at = NULL WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Permanently delete a post that's in the Trash, with its versions.
    pub fn delete_post_forever(&self, id: i64) -> Result<()> {
        self.delete_posts_where("id = ?1 AND deleted_at IS NOT NULL", id)?;
        Ok(())
    }

    /// Permanently delete posts that have been in the Trash too long.
    fn purge_expired(&self) -> Result<usize> {
        let cutoff = now_millis() - DELETED_RETENTION_DAYS * 24 * 60 * 60 * 1000;
        self.delete_posts_where("deleted_at < ?1", cutoff)
    }

    /// Delete the posts matching `filter` (with one parameter) and their
    /// versions, all or nothing.
    fn delete_posts_where(&self, filter: &str, param: i64) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(&format!("DELETE FROM saves WHERE post_id IN (SELECT id FROM posts WHERE {filter})"), [param])?;
        tx.execute(&format!("DELETE FROM exports WHERE post_id IN (SELECT id FROM posts WHERE {filter})"), [param])?;
        let count = tx.execute(&format!("DELETE FROM posts WHERE {filter}"), [param])?;
        tx.commit()?;
        Ok(count)
    }

    /// Record a version of a post: `body` under `name`, now.
    pub fn create_version(&self, post_id: i64, name: &str, body: &str) -> Result<Version> {
        let body = with_title_prefix(body);
        let now = now_millis();
        self.conn.execute(
            "INSERT INTO saves (post_id, name, body, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![post_id, name, body, now],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(Version { id, name: name.to_string(), created_at: from_millis(now)?, body })
    }

    /// A post's versions, newest first, without their text.
    pub fn list_versions(&self, post_id: i64) -> Result<Vec<Version>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, created_at FROM saves WHERE post_id = ?1
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([post_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.map(|row| {
            let (id, name, created_at) = row?;
            Ok(Version { id, name, created_at: from_millis(created_at)?, body: String::new() })
        })
        .collect()
    }

    /// One version, with its full text.
    pub fn version(&self, id: i64) -> Result<Version> {
        let (name, created_at, body): (String, i64, String) = self
            .conn
            .query_row("SELECT name, created_at, body FROM saves WHERE id = ?1", [id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .with_context(|| format!("loading version {id}"))?;
        Ok(Version { id, name, created_at: from_millis(created_at)?, body })
    }

    /// A post's most recent version, with its full text.
    pub fn latest_version(&self, post_id: i64) -> Result<Option<Version>> {
        let id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM saves WHERE post_id = ?1 ORDER BY created_at DESC, id DESC LIMIT 1",
                [post_id],
                |row| row.get(0),
            )
            .optional()?;
        id.map(|id| self.version(id)).transpose()
    }

    /// How a post was last exported, if it has been.
    pub fn export_settings(&self, post_id: i64) -> Result<Option<ExportSettings>> {
        self.exports_where("post_id = ?1", post_id)
    }

    /// How the most recent export of any post went (the Trash included).
    pub fn latest_export_settings(&self) -> Result<Option<ExportSettings>> {
        self.exports_where("?1 ORDER BY exported_at DESC LIMIT 1", 1)
    }

    fn exports_where(&self, filter: &str, param: i64) -> Result<Option<ExportSettings>> {
        let row: Option<(String, String, String, bool, bool)> = self
            .conn
            .query_row(
                &format!(
                    "SELECT path, front_matter, suggested, with_front_matter, keep_title FROM exports WHERE {filter}"
                ),
                [param],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?;
        Ok(row.map(|(path, front_matter, suggested, with_front_matter, keep_title)| ExportSettings {
            path: PathBuf::from(path),
            front_matter,
            suggested,
            with_front_matter,
            keep_title,
        }))
    }

    /// The file each post (not counting the Trash) was last exported to or
    /// imported from.
    pub fn export_paths(&self) -> Result<Vec<(i64, PathBuf)>> {
        let mut stmt = self.conn.prepare(
            "SELECT post_id, path FROM exports JOIN posts ON posts.id = post_id
             WHERE deleted_at IS NULL ORDER BY exported_at DESC",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, PathBuf::from(row.get::<_, String>(1)?))))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Make a post of an imported file's text, or update post `into` with
    /// it (keeping its current text as a version named `version_name`,
    /// unless that's the same), and remember the file as the post's own.
    /// All or nothing. Returns the post's id.
    pub fn import_post(
        &self,
        body: &str,
        settings: &ExportSettings,
        into: Option<i64>,
        version_name: &str,
    ) -> Result<i64> {
        let tx = self.conn.unchecked_transaction()?;
        let id = match into {
            Some(id) => {
                let old = self.post(id)?.body;
                if old != with_title_prefix(body) {
                    self.create_version(id, version_name, &old)?;
                    self.update_post_body(id, body)?;
                }
                id
            }
            None => {
                let id = self.create_post()?;
                self.update_post_body(id, body)?;
                id
            }
        };
        self.record_export(id, settings)?;
        tx.commit()?;
        Ok(id)
    }

    /// Remember how a post was just exported.
    pub fn record_export(&self, post_id: i64, settings: &ExportSettings) -> Result<()> {
        self.conn.execute(
            "INSERT INTO exports (post_id, path, front_matter, suggested, with_front_matter, keep_title, exported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (post_id) DO UPDATE SET path = ?2, front_matter = ?3, suggested = ?4,
                 with_front_matter = ?5, keep_title = ?6, exported_at = ?7",
            params![
                post_id,
                settings.path.to_string_lossy(),
                settings.front_matter,
                settings.suggested,
                settings.with_front_matter,
                settings.keep_title,
                now_millis()
            ],
        )?;
        Ok(())
    }
}

/// The title shown for a post, from its first line (without the `#`).
pub fn title_from_first_line(line: &str) -> String {
    line.strip_prefix('#').unwrap_or(line).trim().to_string()
}

/// `body`, made to start with the `# ` title prefix.
pub fn with_title_prefix(body: &str) -> String {
    if body.starts_with(TITLE_PREFIX) {
        return body.to_string();
    }
    let rest = body.strip_prefix('#').unwrap_or(body).trim_start_matches(' ');
    format!("{TITLE_PREFIX}{rest}")
}

fn derive_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let (memory, iterations, lanes) = KDF_COST;
    let params = Params::new(memory, iterations, lanes, Some(KEY_LEN))
        .map_err(|err| anyhow!("invalid key derivation parameters: {err}"))?;
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    Argon2::new(Algorithm::Argon2id, argon2::Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|err| anyhow!("deriving key: {err}"))?;
    Ok(key)
}

fn apply_key(conn: &Connection, password: &str, salt: &[u8; SALT_LEN]) -> Result<()> {
    let key = derive_key(password, salt)?;
    // Raw key + explicit salt: "x'<64 hex chars of key><32 hex chars of salt>'".
    let pragma = Zeroizing::new(format!(
        "PRAGMA key = \"x'{}{}'\";",
        hex(key.as_ref()).as_str(),
        hex(salt).as_str()
    ));
    conn.execute_batch(&pragma)?;
    Ok(())
}

fn hex(bytes: &[u8]) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(bytes.len() * 2));
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn read_salt(path: &Path) -> Result<[u8; SALT_LEN]> {
    let mut salt = [0u8; SALT_LEN];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut salt))
        .with_context(|| format!("reading vault at {}", path.display()))?;
    Ok(salt)
}

fn create_private_dir(dir: &Path) -> Result<()> {
    if dir.as_os_str().is_empty() || dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    make_private(dir)
}

/// Owner-only permissions. The vault is encrypted anyway; this is just tidy.
fn make_private(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if path.is_dir() { 0o700 } else { 0o600 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .with_context(|| format!("setting permissions on {}", path.display()))?;
    }
    // Elsewhere (Windows), files in the user's profile are already private.
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn now_millis() -> i64 {
    Timestamp::now().as_millisecond()
}

fn from_millis(ms: i64) -> Result<Timestamp> {
    Timestamp::from_millisecond(ms).context("invalid timestamp in vault")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_vault() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/writui.db");
        (dir, path)
    }

    #[test]
    fn create_then_reopen_with_right_password() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "correct horse").unwrap();
        let id = vault.create_post().unwrap();
        vault.update_post_body(id, "# Hello\n\nWorld").unwrap();
        drop(vault);

        let vault = Vault::open(&path, "correct horse").unwrap();
        let posts = vault.list_posts().unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].title, "Hello");
        assert_eq!(vault.post(id).unwrap().body, "# Hello\n\nWorld");
    }

    #[test]
    fn wrong_password_is_reported_as_such() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "correct horse").unwrap();
        vault.create_post().unwrap();
        drop(vault);
        assert!(matches!(Vault::open(&path, "battery staple"), Err(OpenError::WrongPassword)));
    }

    #[test]
    fn file_is_encrypted_and_starts_with_salt() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "correct horse").unwrap();
        let id = vault.create_post().unwrap();
        vault.update_post_body(id, "# Secret diary").unwrap();
        drop(vault);

        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3"));
        assert!(!bytes.windows(6).any(|w| w == b"Secret"));
    }

    #[test]
    fn create_refuses_to_overwrite() {
        let (_dir, path) = temp_vault();
        Vault::create(&path, "correct horse").unwrap();
        assert!(Vault::create(&path, "correct horse").is_err());
    }

    fn ids(posts: Vec<PostSummary>) -> Vec<i64> {
        posts.iter().map(|p| p.id).collect()
    }

    #[test]
    fn posts_list_most_recent_first() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "pw").unwrap();
        let first = vault.create_post().unwrap();
        let second = vault.create_post().unwrap();
        // Times are in milliseconds; make sure the edit is in a later one.
        std::thread::sleep(std::time::Duration::from_millis(2));
        vault.update_post_body(first, "# First, edited later").unwrap();
        assert_eq!(ids(vault.list_posts().unwrap()), [first, second]);
    }

    #[test]
    fn the_cursor_is_remembered_without_counting_as_an_update() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "pw").unwrap();
        let first = vault.create_post().unwrap();
        let second = vault.create_post().unwrap();
        assert_eq!(vault.post(first).unwrap().cursor, None);
        vault.update_post_body(first, "# Hello").unwrap();
        vault.update_post_body(second, "# There").unwrap();
        vault.set_post_cursor(first, 4).unwrap();
        assert_eq!(vault.post(first).unwrap().cursor, Some(4));
        assert_eq!(ids(vault.list_posts().unwrap()), [second, first]);
    }

    #[test]
    fn delete_restore_and_delete_forever() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "pw").unwrap();
        let first = vault.create_post().unwrap();
        let second = vault.create_post().unwrap();

        vault.delete_post(first).unwrap();
        assert_eq!(ids(vault.list_posts().unwrap()), [second]);
        assert_eq!(ids(vault.list_deleted().unwrap()), [first]);
        assert_eq!(vault.count_deleted().unwrap(), 1);

        vault.restore_post(first).unwrap();
        assert_eq!(vault.list_posts().unwrap().len(), 2);
        assert_eq!(vault.count_deleted().unwrap(), 0);

        // Only posts already in the Trash can be deleted forever.
        vault.delete_post_forever(second).unwrap();
        assert_eq!(vault.list_posts().unwrap().len(), 2);
        vault.delete_post(second).unwrap();
        vault.delete_post_forever(second).unwrap();
        assert_eq!(ids(vault.list_posts().unwrap()), [first]);
        assert_eq!(vault.count_deleted().unwrap(), 0);
    }

    #[test]
    fn old_deleted_posts_are_purged_on_open() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "pw").unwrap();
        let old = vault.create_post().unwrap();
        let recent = vault.create_post().unwrap();
        let day = 24 * 60 * 60 * 1000;
        let set_deleted = |id: i64, days_ago: i64| {
            vault
                .conn
                .execute(
                    "UPDATE posts SET deleted_at = ?1 WHERE id = ?2",
                    params![now_millis() - days_ago * day, id],
                )
                .unwrap();
        };
        set_deleted(old, DELETED_RETENTION_DAYS + 1);
        set_deleted(recent, DELETED_RETENTION_DAYS - 1);
        drop(vault);

        let vault = Vault::open(&path, "pw").unwrap();
        assert_eq!(ids(vault.list_deleted().unwrap()), [recent]);
    }

    fn version_count(vault: &Vault) -> i64 {
        vault.conn.query_row("SELECT count(*) FROM saves", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn versions_are_listed_newest_first() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "pw").unwrap();
        let post = vault.create_post().unwrap();
        let other = vault.create_post().unwrap();
        assert!(vault.latest_version(post).unwrap().is_none());

        let first = vault.create_version(post, "First draft", "# Hi\none").unwrap();
        let second = vault.create_version(post, "", "# Hi\ntwo").unwrap();
        vault.create_version(other, "Elsewhere", "# Other").unwrap();
        let versions = vault.list_versions(post).unwrap();
        assert_eq!(versions.iter().map(|s| s.id).collect::<Vec<_>>(), [second.id, first.id]);
        assert_eq!(versions[1].name, "First draft");
        assert_eq!(versions[1].body, ""); // lists leave the text out

        assert_eq!(vault.version(first.id).unwrap().body, "# Hi\none");
        let latest = vault.latest_version(post).unwrap().unwrap();
        assert_eq!((latest.id, latest.body.as_str()), (second.id, "# Hi\ntwo"));
        // Saving doesn't touch the draft.
        assert_eq!(vault.post(post).unwrap().body, "# ");
    }

    #[test]
    fn deleting_a_post_forever_deletes_its_versions() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "pw").unwrap();
        let gone = vault.create_post().unwrap();
        let kept = vault.create_post().unwrap();
        vault.create_version(gone, "a", "# a").unwrap();
        vault.create_version(kept, "b", "# b").unwrap();
        vault.delete_post(gone).unwrap();
        assert_eq!(version_count(&vault), 2); // still restorable, with its versions
        vault.delete_post_forever(gone).unwrap();
        assert_eq!(version_count(&vault), 1);
        assert_eq!(vault.list_versions(kept).unwrap().len(), 1);

        // Purging after 30 days too.
        vault.delete_post(kept).unwrap();
        vault.conn.execute("UPDATE posts SET deleted_at = 0", []).unwrap();
        drop(vault);
        let vault = Vault::open(&path, "pw").unwrap();
        assert_eq!(version_count(&vault), 0);
    }

    #[test]
    fn exports_are_remembered_per_post() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "pw").unwrap();
        let first = vault.create_post().unwrap();
        let second = vault.create_post().unwrap();
        assert_eq!(vault.export_settings(first).unwrap(), None);
        assert_eq!(vault.latest_export_settings().unwrap(), None);

        let settings = |path: &str, with_front_matter| ExportSettings {
            path: PathBuf::from(path),
            front_matter: format!("title: \"{path}\""),
            suggested: "title: \"Hi\"".into(),
            with_front_matter,
            keep_title: !with_front_matter,
        };
        vault.record_export(first, &settings("/a/first.md", true)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        vault.record_export(second, &settings("/b/second.md", false)).unwrap();
        assert_eq!(vault.export_settings(first).unwrap(), Some(settings("/a/first.md", true)));
        assert_eq!(vault.latest_export_settings().unwrap(), Some(settings("/b/second.md", false)));

        // Exporting again replaces what was remembered.
        std::thread::sleep(std::time::Duration::from_millis(2));
        vault.record_export(first, &settings("/c/first.md", false)).unwrap();
        assert_eq!(vault.export_settings(first).unwrap(), Some(settings("/c/first.md", false)));
        assert_eq!(vault.latest_export_settings().unwrap(), Some(settings("/c/first.md", false)));

        // Deleting a post forever forgets it.
        vault.delete_post(first).unwrap();
        vault.delete_post_forever(first).unwrap();
        assert_eq!(vault.export_settings(first).unwrap(), None);
        assert_eq!(vault.latest_export_settings().unwrap(), Some(settings("/b/second.md", false)));
    }

    #[test]
    fn imports_make_or_update_a_post_and_remember_the_file() {
        let (_dir, path) = temp_vault();
        let vault = Vault::create(&path, "pw").unwrap();
        let settings = ExportSettings {
            path: PathBuf::from("/blog/hi.md"),
            front_matter: "date: 2026-01-01".into(),
            suggested: String::new(),
            with_front_matter: true,
            keep_title: false,
        };
        let id = vault.import_post("# Hi\n\nOne", &settings, None, "Before importing hi.md").unwrap();
        assert_eq!(vault.post(id).unwrap().body, "# Hi\n\nOne");
        assert_eq!(vault.export_settings(id).unwrap(), Some(settings.clone()));
        assert_eq!(vault.export_paths().unwrap(), [(id, PathBuf::from("/blog/hi.md"))]);
        assert!(vault.list_versions(id).unwrap().is_empty());

        // Again, into the same post: the old text is kept as a version.
        assert_eq!(vault.import_post("# Hi\n\nTwo", &settings, Some(id), "Before importing hi.md").unwrap(), id);
        assert_eq!(vault.post(id).unwrap().body, "# Hi\n\nTwo");
        let versions = vault.list_versions(id).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].name, "Before importing hi.md");
        assert_eq!(vault.version(versions[0].id).unwrap().body, "# Hi\n\nOne");
        // Nothing changed: no version.
        vault.import_post("# Hi\n\nTwo", &settings, Some(id), "x").unwrap();
        assert_eq!(vault.list_versions(id).unwrap().len(), 1);

        // Posts in the Trash don't count as the file's.
        vault.delete_post(id).unwrap();
        assert!(vault.export_paths().unwrap().is_empty());
    }

    #[test]
    fn title_prefix_is_always_kept() {
        assert_eq!(with_title_prefix("# Hi"), "# Hi");
        assert_eq!(with_title_prefix("#Hi"), "# Hi");
        assert_eq!(with_title_prefix("Hi"), "# Hi");
        assert_eq!(with_title_prefix(""), "# ");
        assert_eq!(title_from_first_line("# "), "");
        assert_eq!(title_from_first_line("#   Spaced  "), "Spaced");
    }
}
