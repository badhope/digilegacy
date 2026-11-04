//! 整库加密的密钥管理（SQLCipher key + Windows DPAPI）。
//!
//! 为什么 key 不用主密码：主密码体系（Argon2id + 包裹 DEK + 字段级加密）已经
//! 有自己的生命周期——锁定/解锁/改密码。若库加密 key 也从主密码派生，
//! 「锁定」就必须连数据库连接一起关掉（否则 key 一直在进程内存里），
//! 而锁定态下应用还要读平台政策等非敏感数据——整套启动/锁定流程都要重构。
//!
//! 因此库加密 key 独立成层：随机 32 字节，DPAPI（CurrentUser 作用域）保护后
//! 落盘。两层加密各管各的：
//!   - 字段级（主密码）：防「拿到文件的人读出内容」
//!   - 整库级（DPAPI key）：防「把 digilegacy.db 文件拷到别的机器/账户直接打开」
//!
//! 威胁模型边界（如实声明）：
//! - 防：db 文件被拷走。DPAPI blob 只能被同一 Windows 账户解开，
//!   拷到别的机器/别的账户是随机字节。
//! - 不防：本机同一用户账户下的攻击者——它能解 DPAPI、能读进程内存。
//!   这个威胁模型本来就超出桌面应用的能力，对策是全盘加密（BitLocker）。
//! - 跨平台：DPAPI 是 Windows 原生设施。非 Windows 编译目标回退为
//!   「无库加密」——E2E 跑的是前端 mock，不触 DB 层，功能不受影响。

#[cfg(windows)]
use std::path::PathBuf;

/// key 文件完整路径。
#[cfg(windows)]
fn key_file_path(app_dir: &std::path::Path) -> PathBuf {
    app_dir.join(KEY_FILE_NAME)
}

/// DPAPI 附加熵：同一 Windows 账户下其它应用即便硬调 CryptUnprotectData，
/// 没有这串应用专属熵也解不开——纵深防御，成本为零。
const ENTROPY: &[u8] = b"digilegacy/db-key/v1";

/// blob 文件名（应用数据目录下）。
pub const KEY_FILE_NAME: &str = "dbkey.bin";

/// blob 文件格式版本。将来换保护机制（如迁移到其它平台设施）时按版本分支。
const BLOB_VERSION: u8 = 1;

/// 整库加密状态（前端展示用）。
#[derive(Debug, serde::Serialize)]
pub struct DbEncryptionStatus {
    pub enabled: bool,
}

/// 生成新的 32 字节库加密密钥（CSPRNG），DPAPI 保护后落盘。
/// 返回明文 key（调用方在进程内使用，不要持久化明文）。
#[cfg(windows)]
pub fn create_db_key(app_dir: &std::path::Path) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut key);
    let blob = dpapi_protect(&key)?;
    let path = key_file_path(app_dir);
    // 原子写：先写临时文件再改名，避免半截 blob 让库永久打不开。
    let tmp = path.with_extension("bin.tmp");
    let mut blob_out = Vec::with_capacity(1 + blob.len());
    blob_out.push(BLOB_VERSION);
    blob_out.extend_from_slice(&blob);
    std::fs::write(&tmp, &blob_out).map_err(|e| format!("写入密钥文件失败: {}", e))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("落盘密钥文件失败: {}", e))?;
    Ok(key)
}

/// 读取并解开库加密密钥。文件不存在返回 Ok(None)——那是「未启用整库加密」，
/// 不是错误；文件存在但解不开才是错误（必须显式报错，不能静默重建：
/// 重建会让加密库永久打不开）。
#[cfg(windows)]
pub fn load_db_key(app_dir: &std::path::Path) -> Result<Option<[u8; 32]>, String> {
    let path = key_file_path(app_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read(&path).map_err(|e| format!("读取密钥文件失败: {}", e))?;
    if raw.first() != Some(&BLOB_VERSION) {
        return Err(format!("密钥文件版本不识别: {}", raw.first().unwrap_or(&0)));
    }
    let plain = dpapi_unprotect(&raw[1..])?;
    let key: [u8; 32] = plain
        .as_slice()
        .try_into()
        .map_err(|_| "密钥文件长度异常".to_string())?;
    Ok(Some(key))
}

/// 删除密钥文件（停用整库加密时调用；调用方必须已先把库转回明文）。
#[cfg(windows)]
pub fn delete_db_key(app_dir: &std::path::Path) -> Result<(), String> {
    let path = key_file_path(app_dir);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("删除密钥文件失败: {}", e))?;
    }
    Ok(())
}

/// DPAPI 保护（CryptProtectData，CurrentUser 作用域 + 应用熵 + 禁止 UI）。
#[cfg(windows)]
fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    // 描述字符串只进事件日志/调试器视图，给个稳定的标识即可。
    let desc: Vec<u16> = "digilegacy-db-key\0".encode_utf16().collect();
    let entropy_blob = CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptProtectData(
            &input,
            PCWSTR(desc.as_ptr()),
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| format!("DPAPI 保护失败: {}", e))?;
    }

    // 拷出来并立刻释放 LocalAlloc 的缓冲区（CryptProtectData 用 LocalAlloc 分配）。
    let out = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        let _ = windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            output.pbData as _,
        )));
    }
    Ok(out)
}

/// DPAPI 解保护（与 dpapi_protect 对称）。
#[cfg(windows)]
fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let entropy_blob = CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let mut descr = windows::core::PWSTR::null();

    unsafe {
        CryptUnprotectData(
            &input,
            Some(&mut descr),
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| format!("DPAPI 解保护失败（密钥文件可能来自其它账户）: {}", e))?;
    }

    let out = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        let _ = windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(
            output.pbData as _,
        )));
    }
    if !descr.is_null() {
        unsafe { windows::core::HSTRING::from_wide(descr.as_wide()) };
    }
    Ok(out)
}

/// 非 Windows 平台的降级实现：整库加密不可用（DPAPI 是 Windows 设施）。
/// load 返回 Ok(None)——「未启用」语义，启动走明文路径，其余功能零影响；
/// 启用/删除显式报错，绝不假装成功。应用的目标平台是 Windows 安装包，
/// 该分支主要保证 Linux CI 的 clippy/test 可编译、可运行。
#[cfg(not(windows))]
mod fallback {
    use std::path::Path;

    pub const KEY_FILE_NAME: &str = super::KEY_FILE_NAME;

    #[derive(Debug, serde::Serialize)]
    pub struct DbEncryptionStatus {
        pub enabled: bool,
    }

    pub fn create_db_key(_app_dir: &Path) -> Result<[u8; 32], String> {
        Err("整库加密目前仅支持 Windows（依赖 DPAPI）".to_string())
    }

    pub fn load_db_key(_app_dir: &Path) -> Result<Option<[u8; 32]>, String> {
        // 非 Windows 平台恒为「未启用」：启动路径据此走明文库，功能完整可用
        Ok(None)
    }

    pub fn delete_db_key(_app_dir: &Path) -> Result<(), String> {
        Err("整库加密目前仅支持 Windows（依赖 DPAPI）".to_string())
    }

    pub fn is_db_encryption_enabled(_app_dir: &Path) -> bool {
        false
    }
}

#[cfg(not(windows))]
pub use fallback::*;

#[cfg(all(windows, test))]
mod tests {
    use super::*;

    /// DPAPI roundtrip：保护后的 blob 不是明文，且能解回原值。
    #[test]
    fn dpapi_roundtrip() {
        let secret = b"digilegacy-test-key-0123456789ab";
        let blob = dpapi_protect(secret).expect("保护失败");
        assert_ne!(blob.as_slice(), &secret[..], "DPAPI 输出不能是明文");
        let plain = dpapi_unprotect(&blob).expect("解保护失败");
        assert_eq!(plain.as_slice(), &secret[..]);
    }

    /// key 文件 create → load → delete 全生命周期。
    #[test]
    fn key_file_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "digilegacy-keytest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let key = create_db_key(&dir).expect("创建失败");
        let loaded = load_db_key(&dir).expect("读取失败");
        assert_eq!(loaded, Some(key), "roundtrip 必须一致");
        delete_db_key(&dir).unwrap();
        assert_eq!(load_db_key(&dir).unwrap(), None, "删除后应返回 None");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
