//! Whole-database encryption migration (SEC-02).
//!
//! Uses SQLCipher's official `sqlcipher_export()` path, never a row-by-row
//! rewrite. Existing plaintext databases can be encrypted in place to a new
//! file, and encrypted databases can be exported back for recovery/testing.

use std::path::Path;

use rusqlite::Connection;

use crate::{apply_db_key, verify_readable, Result, StoreError};

fn sql_path(p: &Path) -> String {
    p.to_string_lossy().replace('\'', "''")
}

fn key_hex(key: &[u8; 32]) -> String {
    key.iter().map(|b| format!("{b:02x}")).collect()
}

/// Export `source` plaintext DB to a new SQLCipher file. No existing target may
/// be overwritten; caller chooses a fresh path (backup before migration).
pub fn migrate_to_encrypted(source: &Path, target: &Path, key: &[u8; 32]) -> Result<()> {
    if !source.exists() {
        return Err(StoreError::State("源数据库不存在".to_string()));
    }
    if target.exists() {
        return Err(StoreError::State(
            "目标数据库已存在，请先备份/移除".to_string(),
        ));
    }
    let conn = Connection::open(source)?;
    conn.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS encrypted KEY \"x'{}'\";\
         SELECT sqlcipher_export('encrypted');\
         PRAGMA encrypted.user_version = {};\
         DETACH DATABASE encrypted;",
        sql_path(target),
        key_hex(key),
        crate::SCHEMA_VERSION
    ))?;
    let check = Connection::open(target)?;
    apply_db_key(&check, key)?;
    verify_readable(&check)?;
    let version: i64 = check.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version != crate::SCHEMA_VERSION {
        return Err(StoreError::State(format!(
            "迁移后 schema 版本不一致: {version}"
        )));
    }
    Ok(())
}

/// Export an encrypted DB back to plaintext. Kept for rollback and test fixtures;
/// production backups should stay encrypted.
pub fn migrate_to_plaintext(source: &Path, target: &Path, key: &[u8; 32]) -> Result<()> {
    if !source.exists() {
        return Err(StoreError::State("源数据库不存在".to_string()));
    }
    if target.exists() {
        return Err(StoreError::State(
            "目标数据库已存在，请先备份/移除".to_string(),
        ));
    }
    let conn = Connection::open(source)?;
    apply_db_key(&conn, key)?;
    verify_readable(&conn)?;
    conn.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS plain KEY '';\
         SELECT sqlcipher_export('plain');\
         PRAGMA plain.user_version = {};\
         DETACH DATABASE plain;",
        sql_path(target),
        crate::SCHEMA_VERSION
    ))?;
    let check = Connection::open(target)?;
    verify_readable(&check)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;

    fn temp(tag: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "dgl_cipher_{tag}_{}_{}.db",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        p
    }

    #[test]
    fn plaintext_to_encrypted_and_back() {
        let plain = temp("plain");
        let enc = temp("enc");
        let plain_target = temp("back");
        {
            let store = Store::open(&plain, None).unwrap();
            store
                .write(|conn| {
                    conn.execute(
                        "INSERT INTO app_settings (key,value,updated_at) VALUES ('k','v','now')",
                        [],
                    )?;
                    Ok(())
                })
                .unwrap();
        }
        let key = [5u8; 32];
        migrate_to_encrypted(&plain, &enc, &key).unwrap();
        assert!(Store::open(&enc, Some(&key)).is_ok());
        assert!(Store::open(&enc, None).is_err(), "无密钥不得打开加密库");

        migrate_to_plaintext(&enc, &plain_target, &key).unwrap();
        let store = Store::open(&plain_target, None).unwrap();
        let value: String = store
            .read(|conn| {
                conn.query_row("SELECT value FROM app_settings WHERE key='k'", [], |r| {
                    r.get(0)
                })
            })
            .unwrap();
        assert_eq!(value, "v");
        drop(store);
        for p in [&plain, &enc, &plain_target] {
            let _ = std::fs::remove_file(p);
        }
    }
}
