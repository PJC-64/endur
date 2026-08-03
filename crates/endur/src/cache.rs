//! Transparent SQLite metadata cache for Endur snapshot information.
//!
//! The cache is entirely optional – if the database file is absent or
//! corrupted every call falls back to raw Git history.  The caller
//! never needs to know whether the data came from the cache or Git.

use rusqlite::{params, Connection, Result as SqlResult};
use std::path::{Path, PathBuf};

use crate::database::RuntimeLock;
use crate::snapshot_info::SnapshotInfo;

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns the path to the SQLite snapshot cache file.
///
/// `~/Library/Caches/endur/snapshot_cache.db` on macOS, or the
/// `ENDUR_CACHE_HOME`-derived equivalent on other platforms.
pub fn cache_db_path() -> PathBuf {
    RuntimeLock::get_endur_cache_home().join("snapshot_cache.db")
}

// ---------------------------------------------------------------------------
// Connection + schema
// ---------------------------------------------------------------------------

/// Open (or create) the cache database and ensure the schema exists.
///
/// Returns `None` on any error so callers can transparently skip the cache.
pub fn open() -> Option<Connection> {
    let path = cache_db_path();

    // Ensure the parent directory exists; ignore failures (will just skip).
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    match Connection::open(&path) {
        Ok(conn) => {
            if init_schema(&conn).is_ok() {
                Some(conn)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

/// Create tables and indices if they don't already exist.
fn init_schema(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous  = NORMAL;

        CREATE TABLE IF NOT EXISTS snapshots (
            repo_path    TEXT    NOT NULL,
            commit_hash  TEXT    NOT NULL,
            base_hash    TEXT    NOT NULL DEFAULT '',
            timestamp    INTEGER NOT NULL,
            message      TEXT    NOT NULL DEFAULT '',
            files_changed INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (repo_path, commit_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_snapshots_repo_ts
            ON snapshots (repo_path, timestamp DESC);
        ",
    )
}

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

/// Fetch all cached snapshots for `repo_path`, ordered newest-first.
///
/// Returns `None` if the cache is unavailable or the query fails.
pub fn get_snapshots(conn: &Connection, repo_path: &Path) -> Option<Vec<SnapshotInfo>> {
    let repo_str = repo_path.to_str()?;
    let mut stmt = conn
        .prepare(
            "SELECT commit_hash, base_hash, timestamp, message, files_changed
             FROM snapshots
             WHERE repo_path = ?1
             ORDER BY timestamp DESC, rowid DESC",
        )
        .ok()?;

    let rows = stmt
        .query_map(params![repo_str], |row| {
            Ok(SnapshotInfo {
                commit_hash: row.get(0)?,
                base_hash: row.get(1)?,
                timestamp: row.get(2)?,
                message: row.get(3)?,
                files_changed: row.get::<_, usize>(4)?,
            })
        })
        .ok()?;

    let mut snapshots = Vec::new();
    for row in rows {
        snapshots.push(row.ok()?);
    }
    Some(snapshots)
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

/// Insert or replace a batch of snapshots for `repo_path` into the cache.
///
/// Silently ignores errors (the snapshot data is the authoritative source).
pub fn upsert_snapshots(conn: &Connection, repo_path: &Path, snapshots: &[SnapshotInfo]) {
    let Some(repo_str) = repo_path.to_str() else {
        return;
    };

    // Use a transaction for bulk performance.
    let _ = conn.execute("BEGIN", []);
    for snap in snapshots {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO snapshots
             (repo_path, commit_hash, base_hash, timestamp, message, files_changed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                repo_str,
                snap.commit_hash,
                snap.base_hash,
                snap.timestamp,
                snap.message,
                snap.files_changed as i64,
            ],
        );
    }
    let _ = conn.execute("COMMIT", []);
}

/// Remove all cached snapshot rows for `repo_path` (e.g. when a repo is
/// unwatched).  Silently ignores errors.
pub fn evict_repo(conn: &Connection, repo_path: &Path) {
    if let Some(repo_str) = repo_path.to_str() {
        let _ = conn.execute(
            "DELETE FROM snapshots WHERE repo_path = ?1",
            params![repo_str],
        );
    }
}

/// Remove all cached snapshot rows for a specific `base_hash` in `repo_path`.
/// Silently ignores errors.
pub fn delete_snapshots_for_base(conn: &Connection, repo_path: &Path, base_hash: &str) {
    if let Some(repo_str) = repo_path.to_str() {
        let _ = conn.execute(
            "DELETE FROM snapshots WHERE repo_path = ?1 AND base_hash = ?2",
            params![repo_str, base_hash],
        );
    }
}

// ---------------------------------------------------------------------------
// Validity check
// ---------------------------------------------------------------------------

/// Returns `true` if the cache has *any* rows for `repo_path`.
///
/// Used to decide whether to trust the cache or do a full Git walk.
pub fn has_entries(conn: &Connection, repo_path: &Path) -> bool {
    let Some(repo_str) = repo_path.to_str() else {
        return false;
    };
    conn.query_row(
        "SELECT COUNT(*) FROM snapshots WHERE repo_path = ?1",
        params![repo_str],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}
