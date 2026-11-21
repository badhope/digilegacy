//! SQLite/SQLCipher store (STO-01..04).
//!
//! One writer connection behind a mutex, one read-only connection for queries.
//! SQLCipher raw key is applied before any other statement; an empty key keeps
//! plain SQLite semantics for legacy databases and tests.

#![forbid(unsafe_code)]

mod audit;
mod builtin_skills;
pub mod checkpoint;
pub mod cipher_migrate;
mod diagnostics;
pub mod legacy_import;
mod schema;

pub use audit::{AuditEntry, AuditVerification};
pub use legacy_import::{import_legacy_backup, ImportMode, ImportReport};
pub use schema::{SCHEMA_SQL, SCHEMA_VERSION};

use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("store: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("store: {0}")]
    Io(#[from] std::io::Error),
    #[error("store: {0}")]
    State(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub const MODULE_FAMILY: &str = "STO";
pub const STATUS: &str = "m1-store";

/// What a caller may do with this store. Repositories live here, not in domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreState {
    NotReady,
    Ready,
    Poisoned,
}

pub struct Store {
    path: PathBuf,
    writer: Mutex<Connection>,
    reader: Option<Mutex<Connection>>,
}

fn key_hex(key: &[u8; 32]) -> String {
    key.iter().map(|b| format!("{b:02x}")).collect()
}

/// Must be the first statement after opening a SQLCipher connection.
pub fn apply_db_key(conn: &Connection, key: &[u8; 32]) -> Result<()> {
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key_hex(key)))
        .map_err(StoreError::from)
}

/// A wrong SQLCipher key is only detected on the first real read.
pub fn verify_readable(conn: &Connection) -> Result<()> {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|_| ())
    .map_err(|_| StoreError::State("库密钥不正确或数据库文件已损坏".to_string()))
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "busy_timeout", "5000")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "cache_size", "-65536")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    let _ = conn.pragma_update(None, "mmap_size", "268435456");
    Ok(())
}

fn open_connection(path: &Path, key: Option<&[u8; 32]>, read_only: bool) -> Result<Connection> {
    let flags = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    let conn = Connection::open_with_flags(path, flags)?;
    if let Some(k) = key {
        apply_db_key(&conn, k)?;
    }
    // 明文库也必须验证可读：没有 key 去打开 SQLCipher 库时，
    // sqlite_master 的第一次读取就会以 "file is not a database" 失败。
    verify_readable(&conn)?;
    Ok(conn)
}

/// Read-only connection for MCP/HTTP read paths. No migration, no writes.
pub fn open_readonly(path: &Path, key: Option<&[u8; 32]>) -> Result<Connection> {
    if !path.exists() {
        return Err(StoreError::State(format!(
            "数据库不存在: {}",
            path.display()
        )));
    }
    let conn = open_connection(path, key, true)?;
    conn.pragma_update(None, "busy_timeout", "5000")?;
    conn.pragma_update(None, "query_only", true)?;
    Ok(conn)
}

impl Store {
    pub fn open(path: impl AsRef<Path>, key: Option<&[u8; 32]>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let writer = open_connection(&path, key, false)?;
        configure(&writer)?;
        writer.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))?;
        schema::apply_schema(&writer)?;

        let reader = if path != Path::new(":memory:") {
            let r = open_connection(&path, key, true)?;
            r.pragma_update(None, "busy_timeout", "5000")?;
            r.pragma_update(None, "query_only", true)?;
            Some(Mutex::new(r))
        } else {
            None
        };

        Ok(Self {
            path,
            writer: Mutex::new(writer),
            reader,
        })
    }

    pub fn open_in_memory(key: Option<&[u8; 32]>) -> Result<Self> {
        Self::open(":memory:", key)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn state(&self) -> StoreState {
        match self.writer.lock() {
            Ok(_) => StoreState::Ready,
            Err(_) => StoreState::Poisoned,
        }
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.read(|conn| conn.query_row("PRAGMA user_version", [], |r| r.get(0)))
    }

    pub fn table_count(&self) -> Result<i64> {
        self.read(|conn| {
            conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
        })
    }

    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T> {
        if let Some(reader) = &self.reader {
            let guard = reader
                .lock()
                .map_err(|_| StoreError::State("只读连接锁中毒".to_string()))?;
            f(&guard).map_err(StoreError::from)
        } else {
            let guard = self
                .writer
                .lock()
                .map_err(|_| StoreError::State("写连接锁中毒".to_string()))?;
            f(&guard).map_err(StoreError::from)
        }
    }

    pub fn write<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|_| StoreError::State("写连接锁中毒".to_string()))?;
        f(&mut guard)
    }

    pub fn integrity_check(&self) -> Result<String> {
        self.read(|conn| conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)))
    }

    pub fn wal_checkpoint(&self) -> Result<()> {
        self.write(|conn| Ok(conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "digilegacy_store_{}_{}_{}.db",
            tag,
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn opens_schema_v18_and_integrity_ok() {
        let path = temp_path("v9");
        let store = Store::open(&path, None).unwrap();
        assert_eq!(store.state(), StoreState::Ready);
        assert_eq!(store.schema_version().unwrap(), crate::SCHEMA_VERSION);
        assert_eq!(store.table_count().unwrap(), 32);
        assert_eq!(store.integrity_check().unwrap(), "ok");
        drop(store);
        cleanup(&path);
    }

    #[test]
    fn single_writer_roundtrip_and_readonly_rejects_writes() {
        let path = temp_path("writer");
        let store = Store::open(&path, None).unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO app_settings (key, value, updated_at) VALUES ('locale', 'zh-CN', 'now')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let value: String = store
            .read(|conn| {
                conn.query_row(
                    "SELECT value FROM app_settings WHERE key='locale'",
                    [],
                    |r| r.get(0),
                )
            })
            .unwrap();
        assert_eq!(value, "zh-CN");

        let ro = open_readonly(&path, None).unwrap();
        assert!(ro
            .execute(
                "INSERT INTO app_settings (key, value, updated_at) VALUES ('x','y','z')",
                [],
            )
            .is_err());
        drop(store);
        cleanup(&path);
    }

    #[test]
    fn sqlcipher_key_is_required() {
        let path = temp_path("key");
        let key = [7u8; 32];
        let store = Store::open(&path, Some(&key)).unwrap();
        assert_eq!(store.table_count().unwrap(), 32);
        drop(store);

        assert!(open_readonly(&path, None).is_err(), "无 key 不得读取加密库");
        let wrong = [8u8; 32];
        assert!(
            open_readonly(&path, Some(&wrong)).is_err(),
            "错误 key 不得读取加密库"
        );
        let ro = open_readonly(&path, Some(&key)).unwrap();
        assert_eq!(
            ro.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            crate::SCHEMA_VERSION
        );
        drop(ro);
        cleanup(&path);
    }

    #[test]
    fn checkpoint_keeps_integrity() {
        let path = temp_path("checkpoint");
        let store = Store::open(&path, None).unwrap();
        store.wal_checkpoint().unwrap();
        assert_eq!(store.integrity_check().unwrap(), "ok");
        drop(store);
        cleanup(&path);
    }
}
