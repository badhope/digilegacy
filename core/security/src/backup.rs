//! Authenticated backup manifests (SEC-04).
//!
//! The database file itself is already SQLCipher/field encrypted. The manifest
//! adds SHA-256 for corruption and an HMAC over the canonical metadata using the
//! backup domain key, so swapping the DB or editing the manifest is detected.

use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use crate::vault::{derive_domain_key, DOMAIN_BACKUP};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupManifest {
    pub format: u32,
    pub created_at: String,
    pub schema_version: i64,
    pub db_sha256: String,
    pub size: u64,
    pub mac: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub db_path: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: BackupManifest,
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("读取备份失败: {e}"))?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn canonical(m: &BackupManifest) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        m.format, m.created_at, m.schema_version, m.db_sha256, m.size
    )
}

fn mac_for(m: &BackupManifest, dek: &[u8; 32]) -> Result<String, String> {
    let key = derive_domain_key(dek, DOMAIN_BACKUP)?;
    let mut mac =
        HmacSha256::new_from_slice(key.as_ref()).map_err(|_| "HMAC 密钥初始化失败".to_string())?;
    mac.update(canonical(m).as_bytes());
    Ok(mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建备份目录失败: {e}"))?;
    }
    fs::write(path, bytes).map_err(|e| format!("写入备份失败: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Copy a checkpointed DB file and write a hash/MAC manifest beside it.
pub fn create_backup(
    db_path: &Path,
    backup_dir: &Path,
    schema_version: i64,
    dek: Option<&[u8; 32]>,
) -> Result<BackupInfo, String> {
    if !db_path.exists() {
        return Err("源数据库不存在".to_string());
    }
    let created_at = Utc::now().to_rfc3339();
    let stamp = Utc::now().format("%Y%m%d-%H%M%S-%3f").to_string();
    let db_target = backup_dir.join(format!("digilegacy-{stamp}.db"));
    let manifest_path = backup_dir.join(format!("digilegacy-{stamp}.backup.json"));
    fs::create_dir_all(backup_dir).map_err(|e| format!("创建备份目录失败: {e}"))?;

    let bytes = fs::read(db_path).map_err(|e| format!("读取源数据库失败: {e}"))?;
    private_write(&db_target, &bytes)?;
    let db_sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();

    let mut manifest = BackupManifest {
        format: 1,
        created_at,
        schema_version,
        db_sha256,
        size: bytes.len() as u64,
        mac: None,
    };
    if let Some(k) = dek {
        manifest.mac = Some(mac_for(&manifest, k)?);
    }
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| format!("序列化清单失败: {e}"))?;
    private_write(&manifest_path, &json)?;

    Ok(BackupInfo {
        db_path: db_target,
        manifest_path,
        manifest,
    })
}

/// Verify size, SHA-256 and (when present) HMAC without restoring anything.
pub fn verify_backup(
    db_path: &Path,
    manifest_path: &Path,
    dek: Option<&[u8; 32]>,
) -> Result<BackupManifest, String> {
    let raw = fs::read(manifest_path).map_err(|e| format!("读取备份清单失败: {e}"))?;
    let manifest: BackupManifest =
        serde_json::from_slice(&raw).map_err(|e| format!("备份清单格式无效: {e}"))?;
    if manifest.format != 1 {
        return Err("备份清单版本不支持".to_string());
    }
    let meta = fs::metadata(db_path).map_err(|e| format!("备份文件不存在: {e}"))?;
    if meta.len() != manifest.size {
        return Err("备份大小与清单不一致".to_string());
    }
    let actual = sha256_file(db_path)?;
    if actual != manifest.db_sha256 {
        return Err("备份内容哈希不一致，文件已损坏或被替换".to_string());
    }
    if let Some(expected) = &manifest.mac {
        let key = dek.ok_or("该备份需要数据密钥校验，但当前未解锁")?;
        let actual_mac = mac_for(&manifest, key)?;
        if &actual_mac != expected {
            return Err("备份认证标签不匹配，备份可能被篡改".to_string());
        }
    }
    Ok(manifest)
}

/// Verify then copy a backup to `target`. Caller must close/relocate the live DB.
pub fn restore_backup(
    db_path: &Path,
    manifest_path: &Path,
    target: &Path,
    dek: Option<&[u8; 32]>,
) -> Result<BackupManifest, String> {
    let manifest = verify_backup(db_path, manifest_path, dek)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目标目录失败: {e}"))?;
    }
    fs::copy(db_path, target).map_err(|e| format!("恢复数据库失败: {e}"))?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "digilegacy_backup_{tag}_{}_{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn create_verify_and_restore_roundtrip() {
        let dir = temp_dir("roundtrip");
        let db = dir.join("live.db");
        fs::write(&db, b"encrypted-db-bytes").unwrap();
        let dek = [9u8; 32];
        let info = create_backup(&db, &dir.join("backups"), 9, Some(&dek)).unwrap();
        assert!(info.db_path.exists());
        assert!(info.manifest_path.exists());
        let m = verify_backup(&info.db_path, &info.manifest_path, Some(&dek)).unwrap();
        assert_eq!(m.db_sha256, info.manifest.db_sha256);
        assert!(m.mac.is_some());

        let restored = dir.join("restored.db");
        restore_backup(&info.db_path, &info.manifest_path, &restored, Some(&dek)).unwrap();
        assert_eq!(fs::read(&restored).unwrap(), b"encrypted-db-bytes");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tampered_db_is_rejected() {
        let dir = temp_dir("tamper_db");
        let db = dir.join("live.db");
        fs::write(&db, b"good").unwrap();
        let info = create_backup(&db, &dir.join("backups"), 9, Some(&[3u8; 32])).unwrap();
        fs::write(&info.db_path, b"evil").unwrap();
        assert!(verify_backup(&info.db_path, &info.manifest_path, Some(&[3u8; 32])).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tampered_manifest_mac_is_rejected() {
        let dir = temp_dir("tamper_manifest");
        let db = dir.join("live.db");
        fs::write(&db, b"good").unwrap();
        let info = create_backup(&db, &dir.join("backups"), 9, Some(&[3u8; 32])).unwrap();
        let mut m = info.manifest.clone();
        m.mac = Some("00".repeat(32));
        fs::write(&info.manifest_path, serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(verify_backup(&info.db_path, &info.manifest_path, Some(&[3u8; 32])).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
