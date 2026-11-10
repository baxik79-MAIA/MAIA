//! Existing health containers are inspected before writable SQLite startup.
//! Tests own private fixtures; DB/WAL content is distinct from SHM coordination.
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use maia_local_intelligence_health::{HealthStore, ObserverError, OperationClass, RetentionPolicy};
use rusqlite::{Connection, OpenFlags};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "maia-health-schema-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self) -> PathBuf {
        self.0.join("health.sqlite3")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn current(path: &Path) {
    drop(HealthStore::open(path, RetentionPolicy::default()).unwrap());
}
fn raw(path: &Path, sql: &str) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(sql).unwrap();
}
fn readonly(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}
fn wal_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push("-wal");
    name.into()
}
fn refuses_schema_without_content_change(path: &Path) {
    let before = std::fs::read(path).unwrap();
    assert!(
        matches!(
            HealthStore::open(path, RetentionPolicy::default()),
            Err(ObserverError::SchemaVersionMismatch)
        ),
        "expected schema refusal for {path:?}"
    );
    assert_eq!(std::fs::read(path).unwrap(), before, "changed {path:?}");
}

#[test]
fn fresh_empty_and_current_one_preserve_recorded_history() {
    for existing_empty in [false, true] {
        let fixture = Fixture::new(if existing_empty { "empty" } else { "fresh" });
        let path = fixture.path();
        if existing_empty {
            drop(Connection::open(&path).unwrap());
        }
        let store = HealthStore::open(&path, RetentionPolicy::default()).unwrap();
        store
            .record_inference_success(
                "fixture-model",
                OperationClass::Complete,
                Duration::from_millis(7),
                UNIX_EPOCH + Duration::from_secs(5),
            )
            .unwrap();
        drop(store);
        let connection = readonly(&path);
        let schema_before: Vec<(String, String)> = connection
            .prepare(
                "SELECT name,sql FROM sqlite_master WHERE name NOT GLOB 'sqlite_*' ORDER BY name",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        drop(connection);
        let reopened = HealthStore::open(&path, RetentionPolicy::default()).unwrap();
        assert_eq!(
            reopened
                .recent_latencies(UNIX_EPOCH, UNIX_EPOCH + Duration::from_secs(10), 10)
                .unwrap(),
            vec![Duration::from_millis(7)]
        );
        drop(reopened);
        let connection = readonly(&path);
        let schema_after: Vec<(String, String)> = connection
            .prepare(
                "SELECT name,sql FROM sqlite_master WHERE name NOT GLOB 'sqlite_*' ORDER BY name",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(schema_after, schema_before);
        let metadata: (String, String) = connection
            .query_row("SELECT key,value FROM schema_meta", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(metadata, ("version".to_owned(), "1".to_owned()));
    }
}

#[test]
fn future_closed_database_is_refused_before_schema_repair() {
    let fixture = Fixture::new("future-closed");
    let path = fixture.path();
    current(&path);
    raw(
        &path,
        "DROP TABLE status_samples; UPDATE schema_meta SET value='2' WHERE key='version';",
    );
    refuses_schema_without_content_change(&path);
    let connection = readonly(&path);
    let missing: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='status_samples'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        missing, 0,
        "refusal must not recreate a future schema table"
    );
    let version: String = connection
        .query_row(
            "SELECT value FROM schema_meta WHERE key='version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(version, "2");
}

#[test]
fn future_version_committed_only_in_wal_is_refused_without_content_writes() {
    let fixture = Fixture::new("future-wal");
    let path = fixture.path();
    current(&path);
    let writer = Connection::open(&path).unwrap();
    writer
        .execute_batch("PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    let main_before_update = std::fs::read(&path).unwrap();
    writer
        .execute("UPDATE schema_meta SET value='2' WHERE key='version'", [])
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), main_before_update);
    let wal = wal_path(&path);
    let wal_before = std::fs::read(&wal).unwrap();
    assert!(!wal_before.is_empty());
    refuses_schema_without_content_change(&path);
    assert_eq!(std::fs::read(&wal).unwrap(), wal_before);
    // The writer stays alive through comparison; a later normal checkpoint
    // during fixture cleanup is outside the preflight refusal observation.
    let version: String = writer
        .query_row(
            "SELECT value FROM schema_meta WHERE key='version'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(version, "2");
}

#[test]
fn malformed_version_metadata_is_refused_without_content_writes() {
    for (label, sql) in [
        (
            "duplicate",
            "CREATE TABLE schema_meta(key TEXT,value TEXT); INSERT INTO schema_meta VALUES('version','1'),('version','1');",
        ),
        (
            "wrong-type",
            "CREATE TABLE schema_meta(key TEXT,value BLOB); INSERT INTO schema_meta VALUES('version',x'31');",
        ),
        ("missing", "CREATE TABLE schema_meta(key TEXT,value TEXT);"),
        (
            "other-key",
            "CREATE TABLE schema_meta(key TEXT,value TEXT); INSERT INTO schema_meta VALUES('other','1');",
        ),
        (
            "zero",
            "CREATE TABLE schema_meta(key TEXT,value TEXT); INSERT INTO schema_meta VALUES('version','0');",
        ),
        (
            "unknown-db",
            "CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES('keep-me');",
        ),
    ] {
        let fixture = Fixture::new(label);
        let path = fixture.path();
        raw(&path, sql);
        refuses_schema_without_content_change(&path);
    }
}

#[test]
fn malformed_current_schema_is_not_silently_repaired() {
    for (label, sql) in [
        ("missing-table", "DROP TABLE inference_outcomes;"),
        ("missing-index", "DROP INDEX idx_status_samples_time;"),
        (
            "wrong-index",
            "DROP INDEX idx_status_samples_time; CREATE INDEX idx_status_samples_time ON status_samples(model);",
        ),
        (
            "trigger-name-collision",
            "CREATE TRIGGER schema_meta AFTER INSERT ON status_samples BEGIN SELECT 1; END; DROP TABLE schema_meta; CREATE TABLE schema_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO schema_meta VALUES('version','1');",
        ),
    ] {
        let fixture = Fixture::new(label);
        let path = fixture.path();
        current(&path);
        raw(&path, sql);
        refuses_schema_without_content_change(&path);
    }
}

#[test]
fn corrupt_container_is_refused_without_overwrite() {
    let fixture = Fixture::new("corrupt");
    let path = fixture.path();
    let before = b"This is not a SQLite database and must never be initialized over.";
    std::fs::write(&path, before).unwrap();
    assert!(matches!(
        HealthStore::open(&path, RetentionPolicy::default()),
        Err(ObserverError::StorageUnavailable)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn file_uri_is_refused_before_target_resolution() {
    let fixture = Fixture::new("file-uri");
    let path = fixture.path();
    current(&path);
    raw(
        &path,
        "UPDATE schema_meta SET value='2' WHERE key='version';",
    );
    let before = std::fs::read(&path).unwrap();
    let uri = PathBuf::from(format!(
        "file:{}?mode=rw",
        path.to_string_lossy().replace('\\', "/")
    ));
    assert!(matches!(
        HealthStore::open(&uri, RetentionPolicy::default()),
        Err(ObserverError::StorageUnavailable)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        // Rusqlite accepts Unix raw filename bytes. An invalid UTF-8 URI
        // query must not bypass the same byte-level file: prefix refusal.
        let mut bytes = b"file:".to_vec();
        bytes.extend_from_slice(path.as_os_str().as_bytes());
        bytes.extend_from_slice(b"?mode=rw&unused=\xff");
        let uri = PathBuf::from(std::ffi::OsString::from_vec(bytes));
        assert!(matches!(
            HealthStore::open(&uri, RetentionPolicy::default()),
            Err(ObserverError::StorageUnavailable)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
