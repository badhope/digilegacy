//! Named checkpoints (WP-13). A checkpoint is an authenticated DB copy plus
//! metadata; restore is only safe when the live Store is closed.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use digilegacy_security::backup::{create_backup, restore_backup, verify_backup};

use crate::{Result, Store, StoreError};

pub const MAX_CHECKPOINTS: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckpointMeta {
    pub id: String,
    pub label: String,
    pub created_at: String,
    pub schema_version: i64,
    pub size: u64,
    pub auto: bool,
    pub db_path: String,
    pub manifest_path: String,
}

fn checkpoint_dir(dir: &Path) -> PathBuf {
    dir.join("checkpoints")
}

fn meta_path(dir: &Path, id: &str) -> PathBuf {
    checkpoint_dir(dir).join(format!("{id}.checkpoint.json"))
}

/// Snapshot the store and write metadata. The store remains open.
pub fn create_checkpoint(
    store: &Store,
    dir: &Path,
    label: &str,
    auto: bool,
    dek: Option<&[u8; 32]>,
) -> Result<CheckpointMeta> {
    if store.path() == Path::new(":memory:") {
        return Err(StoreError::State(
            "内存数据库不能创建文件检查点".to_string(),
        ));
    }
    store.wal_checkpoint()?;
    let schema_version = store.schema_version()?;
    let info = create_backup(store.path(), &checkpoint_dir(dir), schema_version, dek)
        .map_err(StoreError::State)?;
    let id = Utc::now().format("%Y%m%d-%H%M%S-%f").to_string();
    let meta = CheckpointMeta {
        id: id.clone(),
        label: label.chars().take(80).collect(),
        created_at: Utc::now().to_rfc3339(),
        schema_version,
        size: info.manifest.size,
        auto,
        db_path: info.db_path.to_string_lossy().to_string(),
        manifest_path: info.manifest_path.to_string_lossy().to_string(),
    };
    let bytes = serde_json::to_vec_pretty(&meta)
        .map_err(|e| StoreError::State(format!("序列化检查点失败: {e}")))?;
    fs::write(meta_path(dir, &id), bytes)?;
    prune(dir)?;
    Ok(meta)
}

pub fn list_checkpoints(dir: &Path) -> Vec<CheckpointMeta> {
    let d = checkpoint_dir(dir);
    let Ok(entries) = fs::read_dir(d) else {
        return Vec::new();
    };
    let mut out: Vec<CheckpointMeta> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.ends_with(".checkpoint.json"))
            .unwrap_or(false)
        {
            continue;
        }
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(meta) = serde_json::from_slice::<CheckpointMeta>(&bytes) {
                out.push(meta);
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

fn prune(dir: &Path) -> Result<()> {
    let mut list = list_checkpoints(dir);
    if list.len() <= MAX_CHECKPOINTS {
        return Ok(());
    }
    list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let remove = list.len().saturating_sub(MAX_CHECKPOINTS);
    for meta in list.into_iter().take(remove) {
        let _ = fs::remove_file(&meta.db_path);
        let _ = fs::remove_file(&meta.manifest_path);
        let _ = fs::remove_file(meta_path(dir, &meta.id));
    }
    Ok(())
}

/// Verify a checkpoint without writing.
pub fn verify_checkpoint(meta: &CheckpointMeta, dek: Option<&[u8; 32]>) -> Result<()> {
    verify_backup(
        Path::new(&meta.db_path),
        Path::new(&meta.manifest_path),
        dek,
    )
    .map(|_| ())
    .map_err(StoreError::State)
}

/// Restore into a target path. The caller must close the live DB first.
pub fn restore_checkpoint(
    meta: &CheckpointMeta,
    target: &Path,
    dek: Option<&[u8; 32]>,
) -> Result<()> {
    restore_backup(
        Path::new(&meta.db_path),
        Path::new(&meta.manifest_path),
        target,
        dek,
    )
    .map(|_| ())
    .map_err(StoreError::State)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "dgl_ckpt_{tag}_{}_{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn create_list_verify_and_restore() {
        let dir = temp_dir("roundtrip");
        let db = dir.join("live.db");
        let store = Store::open(&db, None).unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO app_settings (key,value,updated_at) VALUES ('k','v','now')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let meta = create_checkpoint(&store, &dir, "手动检查点", false, None).unwrap();
        assert_eq!(list_checkpoints(&dir).len(), 1);
        verify_checkpoint(&meta, None).unwrap();
        let target = dir.join("restored.db");
        restore_checkpoint(&meta, &target, None).unwrap();
        drop(store);
        let restored = Store::open(&target, None).unwrap();
        let value: String = restored
            .read(|conn| {
                conn.query_row("SELECT value FROM app_settings WHERE key='k'", [], |r| {
                    r.get(0)
                })
            })
            .unwrap();
        assert_eq!(value, "v");
        drop(restored);
        let _ = fs::remove_dir_all(&dir);
    }
}
