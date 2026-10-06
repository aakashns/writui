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
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use argon2::{Algorithm, Argon2, Params, Version};
use jiff::Timestamp;
use rusqlite::{Connection, ErrorCode, OpenFlags, params};
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

#[derive(Debug, Clone)]
pub struct Post {
    pub id: i64,
    pub body: String,
    /// Where the cursor was when the post was last closed (a character
    /// index into `body`). `None` means the end.
    pub cursor: Option<usize>,
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
        let (body, cursor): (String, Option<i64>) = self
            .conn
            .query_row("SELECT body, cursor FROM posts WHERE id = ?1", [id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .with_context(|| format!("loading post {id}"))?;
        let cursor = cursor.and_then(|pos| usize::try_from(pos).ok());
        Ok(Post { id, body, cursor })
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

    /// Permanently delete a post that's in the Trash.
    pub fn delete_post_forever(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM posts WHERE id = ?1 AND deleted_at IS NOT NULL", [id])?;
        Ok(())
    }

    /// Permanently delete posts that have been in the Trash too long.
    fn purge_expired(&self) -> Result<usize> {
        let cutoff = now_millis() - DELETED_RETENTION_DAYS * 24 * 60 * 60 * 1000;
        Ok(self.conn.execute("DELETE FROM posts WHERE deleted_at < ?1", [cutoff])?)
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
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
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
