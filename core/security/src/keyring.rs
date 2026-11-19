//! File-backed database key custody (SEC-02).
//!
//! The platform process stores a 32-byte SQLCipher key in a private file
//! (0600 on Unix). Desktop DPAPI wrapping is a later shell concern; this is the
//! server/CI-safe equivalent. Losing this file means losing the database key,
//! so backups must include it or the recovery kit must be used.

use rand::RngCore;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub struct FileKeyring {
    path: PathBuf,
}

impl FileKeyring {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load_or_create(&self) -> Result<Zeroizing<[u8; 32]>, String> {
        if self.path.exists() {
            return self.load();
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建密钥目录失败: {e}"))?;
        }
        let mut key = Zeroizing::new([0u8; 32]);
        rand::rng().fill_bytes(key.as_mut());
        let tmp = self.path.with_extension("tmp");
        let mut file = fs::File::create(&tmp).map_err(|e| format!("创建密钥文件失败: {e}"))?;
        file.write_all(key.as_ref())
            .map_err(|e| format!("写入密钥文件失败: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("同步密钥文件失败: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("设置密钥文件权限失败: {e}"))?;
        }
        fs::rename(&tmp, &self.path).map_err(|e| format!("替换密钥文件失败: {e}"))?;
        Ok(key)
    }

    pub fn load(&self) -> Result<Zeroizing<[u8; 32]>, String> {
        let bytes = fs::read(&self.path).map_err(|e| format!("读取密钥文件失败: {e}"))?;
        if bytes.len() != 32 {
            return Err("数据库密钥长度不合法".to_string());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&self.path)
                .map_err(|e| format!("读取密钥权限失败: {e}"))?
                .permissions()
                .mode()
                & 0o777;
            if mode & 0o077 != 0 {
                return Err(format!("密钥文件权限过宽（{mode:o}），要求仅本人可读"));
            }
        }
        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&bytes);
        Ok(key)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "dgl_keyring_{tag}_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        p
    }

    #[test]
    fn creates_then_loads_same_key() {
        let path = temp("roundtrip");
        let kr = FileKeyring::new(&path);
        let a = kr.load_or_create().unwrap();
        let b = kr.load().unwrap();
        assert_eq!(*a, *b);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_short_key_file() {
        let path = temp("short");
        fs::write(&path, b"short").unwrap();
        let kr = FileKeyring::new(&path);
        assert!(kr.load().is_err());
        let _ = fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn key_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let path = temp("perms");
        let kr = FileKeyring::new(&path);
        kr.load_or_create().unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = fs::remove_file(&path);
    }
}
