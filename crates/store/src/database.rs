// SPDX-License-Identifier: MIT
use std::{path::Path, time::Duration};

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

const VERSION: i64 = 1;
const INITIAL_SQL: &str = include_str!("../../../contracts/migrations/0001.sql");
const EXPECTED_TABLES: &[&str] = &[
    "accounts",
    "artifacts",
    "instances",
    "plugins",
    "revision_artifacts",
    "revisions",
    "runtimes",
    "schema_migrations",
    "settings",
    "tasks",
    "transactions",
];

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    FutureSchema(i64),
    UnrecognizedDatabase,
    InvalidMigrationHistory,
    IntegrityCheckFailed,
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// A local database handle. The host must eventually obtain the state-root owner
/// lock (T010) before opening this writer; no multi-process ownership is claimed.
pub struct StateStore {
    connection: Connection,
}

impl StateStore {
    /// Initialize a new database or reopen the known schema. Future/unrecognized
    /// databases are rejected before changing journal mode or writing migrations.
    /// No upgrades of an existing schema are implemented; such an upgrade must
    /// add backup/restore policy before it can be enabled.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        let existing_version = inspect_version(&connection)?;
        if existing_version.is_some() {
            validate_integrity(&connection)?;
        }
        let mode: String =
            connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        if mode != "wal" {
            return Err(StoreError::IntegrityCheckFailed);
        }
        if existing_version.is_none() {
            bootstrap(&mut connection, migration_body())?;
        }
        validate_integrity(&connection)?;
        Ok(Self { connection })
    }

    pub fn schema_version(&self) -> Result<i64, StoreError> {
        Ok(self
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })?)
    }
}

fn table_names(connection: &Connection) -> Result<Vec<String>, rusqlite::Error> {
    connection
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
        .query_map([], |row| row.get(0))?
        .collect()
}

fn inspect_version(connection: &Connection) -> Result<Option<i64>, StoreError> {
    let tables = table_names(connection)?;
    if connection
        .prepare("SELECT 1 FROM sqlite_schema WHERE type IN ('view', 'trigger') LIMIT 1")?
        .exists([])?
    {
        return Err(StoreError::UnrecognizedDatabase);
    }
    if tables.is_empty() {
        return Ok(None);
    }
    if !tables.iter().any(|table| table == "schema_migrations") {
        return Err(StoreError::UnrecognizedDatabase);
    }
    let versions = connection
        .prepare("SELECT version FROM schema_migrations ORDER BY version")?
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(&version) = versions.last()
        && version > VERSION
    {
        return Err(StoreError::FutureSchema(version));
    }
    if versions != [VERSION] || tables != EXPECTED_TABLES {
        return Err(StoreError::InvalidMigrationHistory);
    }
    Ok(Some(VERSION))
}

fn migration_body() -> &'static str {
    INITIAL_SQL
        .strip_prefix("PRAGMA foreign_keys = ON;\nPRAGMA journal_mode = WAL;\nBEGIN;\n")
        .and_then(|body| body.strip_suffix("COMMIT;\n"))
        .expect("initial migration transaction wrapper changed; review the migration runner")
}

fn bootstrap(connection: &mut Connection, body: &str) -> Result<(), StoreError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Recheck under the SQLite write transaction in case another initializer
    // completed between the first inspection and acquisition of this lock.
    if inspect_version(&transaction)?.is_none() {
        transaction.execute_batch(body)?;
    }
    transaction.commit()?;
    Ok(())
}

fn validate_integrity(connection: &Connection) -> Result<(), StoreError> {
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    let foreign_key_violation = connection.prepare("PRAGMA foreign_key_check")?.exists([])?;
    if integrity != "ok" || foreign_key_violation {
        return Err(StoreError::IntegrityCheckFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct TestDatabase(PathBuf);
    impl TestDatabase {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "cobble-sql-test-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root).unwrap();
            Self(root.join("state.db"))
        }
    }
    impl Drop for TestDatabase {
        fn drop(&mut self) {
            for suffix in ["", "-wal", "-shm"] {
                let _ = fs::remove_file(self.0.with_file_name(format!("state.db{suffix}")));
            }
            let _ = fs::remove_dir(self.0.parent().unwrap());
        }
    }

    #[test]
    fn fresh_migration_persists_and_reopen_does_not_apply_it_twice() {
        let path = TestDatabase::new();
        {
            let store = StateStore::open(&path.0).unwrap();
            assert_eq!(store.schema_version().unwrap(), 1);
            assert_eq!(
                store
                    .connection
                    .query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            store
                .connection
                .execute("INSERT INTO settings VALUES('kept', 'true')", [])
                .unwrap();
        }
        let store = StateStore::open(&path.0).unwrap();
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT value_json FROM settings WHERE key='kept'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "true"
        );
    }

    #[test]
    fn deferred_foreign_keys_reject_cross_instance_active_and_parent_revisions() {
        let path = TestDatabase::new();
        let mut store = StateStore::open(&path.0).unwrap();
        store.connection.execute_batch("INSERT INTO instances VALUES('i1','one','root1','{}',NULL); INSERT INTO instances VALUES('i2','two','root2','{}',NULL); INSERT INTO revisions VALUES('r1','i1',NULL,'{}','test'); UPDATE instances SET active_revision='r1' WHERE id='i1';").unwrap();
        let transaction = store.connection.transaction().unwrap();
        transaction
            .execute(
                "UPDATE instances SET active_revision='r1' WHERE id='i2'",
                [],
            )
            .unwrap();
        assert!(transaction.commit().is_err());
        let transaction = store.connection.transaction().unwrap();
        transaction
            .execute(
                "INSERT INTO revisions VALUES('r2','i2','r1','{}','test')",
                [],
            )
            .unwrap();
        assert!(transaction.commit().is_err());
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT active_revision FROM instances WHERE id='i2'",
                    [],
                    |r| r.get::<_, Option<String>>(0)
                )
                .unwrap(),
            None
        );
    }

    #[test]
    fn future_schema_is_rejected_without_modifying_existing_database_bytes() {
        let path = TestDatabase::new();
        {
            let connection = Connection::open(&path.0).unwrap();
            connection.execute_batch("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT); INSERT INTO schema_migrations VALUES(2,'future');").unwrap();
        }
        let before = fs::read(&path.0).unwrap();
        assert!(matches!(
            StateStore::open(&path.0),
            Err(StoreError::FutureSchema(2))
        ));
        assert_eq!(fs::read(&path.0).unwrap(), before);
    }

    #[test]
    fn unrelated_database_is_not_treated_as_an_empty_launcher_database() {
        let path = TestDatabase::new();
        {
            let connection = Connection::open(&path.0).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE precious(data TEXT); INSERT INTO precious VALUES('keep');",
                )
                .unwrap();
        }
        let before = fs::read(&path.0).unwrap();
        assert!(matches!(
            StateStore::open(&path.0),
            Err(StoreError::UnrecognizedDatabase)
        ));
        assert_eq!(fs::read(&path.0).unwrap(), before);

        let view_path = TestDatabase::new();
        {
            let connection = Connection::open(&view_path.0).unwrap();
            connection
                .execute_batch("CREATE VIEW precious AS SELECT 'keep' AS data;")
                .unwrap();
        }
        let before = fs::read(&view_path.0).unwrap();
        assert!(matches!(
            StateStore::open(&view_path.0),
            Err(StoreError::UnrecognizedDatabase)
        ));
        assert_eq!(fs::read(&view_path.0).unwrap(), before);
    }

    #[test]
    fn failed_initial_migration_rolls_back_schema_and_can_be_retried() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        assert!(
            bootstrap(
                &mut connection,
                "CREATE TABLE temporary(data TEXT); INVALID SQL;"
            )
            .is_err()
        );
        assert!(table_names(&connection).unwrap().is_empty());
        assert!(connection.is_autocommit());
        bootstrap(&mut connection, migration_body()).unwrap();
        assert_eq!(inspect_version(&connection).unwrap(), Some(1));
    }
}
