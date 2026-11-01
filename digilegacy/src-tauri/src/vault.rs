//! 主密码与字段级加密保管
//!
//! **两层密钥**（v2 起）：
//! - 主密码 → Argon2id/PBKDF2 → **KEK**（密钥加密密钥，只在解锁瞬间存在于栈上）；
//! - 数据库里存一把随机 **DEK**（数据加密密钥），用 KEK 包裹后写入 `security_config.wrapped_dek`；
//! - 所有字段用 DEK 加密，DEK 再按用途经 HKDF 分出子密钥（field / backup / kit 三个域）。
//!
//! 为什么这样分层：旧实现「密码派生密钥直接加密字段」，改密码必须全库逐字段重加密
//! （资产 1000 条 = 6000 次 AES + 6000 次 UPDATE）。分层后改密码只是重新包裹一次 DEK，
//! O(n) → O(1)；同时 DEK 保持不变，已发出的继承人恢复份额不会因为改密码而失效。
//!
//! 兼容性：旧库没有 `wrapped_dek`。此时解锁后派生的密钥本身就是数据密钥（legacy 模式），
//! 字段仍是 `enc:v1:`；下一次改密码时一次性迁移到 DEK + `enc:v2:`（带 HKDF 域分离），
//! 此后不再需要全库重加密。

use once_cell::sync::Lazy;
use std::sync::Mutex;
use zeroize::{Zeroize, Zeroizing};

use crate::crypto;

/// v1 字段前缀：直接用数据密钥加密（legacy，无域分离）
pub const ENC_PREFIX: &str = "enc:v1:";
/// v2 字段前缀：用 HKDF 派生的 field 子密钥加密
pub const ENC_PREFIX_V2: &str = "enc:v2:";
const VERIFIER_PLAINTEXT: &str = "digilegacy-verifier-v1";

/// HKDF 域标签。不同用途之间不能互相解密——备份密钥泄露不等于字段可读。
pub const DOMAIN_FIELD: &[u8] = b"digilegacy/field/v2";
// 备份加密与恢复包口令包裹将迁移到这两个域（BENCHMARK P1-2 落地时启用）；
// 域分离测试已锁定三域互斥，先保留常量防止实现时域标签漂移。
#[allow(dead_code)]
pub const DOMAIN_BACKUP: &[u8] = b"digilegacy/backup/v2";
#[allow(dead_code)]
pub const DOMAIN_KIT: &[u8] = b"digilegacy/kit/v2";

static MASTER_KEY: Lazy<Mutex<Option<Zeroizing<[u8; 32]>>>> = Lazy::new(|| Mutex::new(None));

/// 测试串行锁：跨模块共享，避免并行测试互相污染全局状态
#[cfg(test)]
pub(crate) static TEST_SERIAL: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// 用 HKDF-SHA256 从数据密钥派生指定用途的子密钥。
///
/// salt 固定为空、info 为域标签：同一 DEK 下不同域互不可解，
/// 且域标签是常量，不随库变化（换机/换密码都不影响派生结果的语义）。
pub fn derive_domain_key(dek: &[u8; 32], domain: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, dek);
    let mut out = Zeroizing::new([0u8; 32]);
    hk.expand(domain, out.as_mut())
        .map_err(|e| format!("派生密钥失败: {}", e))?;
    Ok(out)
}

/// 解锁后驻留内存的**数据密钥**（Zeroizing：所有副本离开作用域即清零）
pub fn set_key(k: Zeroizing<[u8; 32]>) {
    let mut guard = MASTER_KEY.lock().unwrap();
    *guard = Some(k);
}

/// 锁定：先清零再清除内存中的数据密钥
pub fn clear_key() {
    let mut guard = MASTER_KEY.lock().unwrap();
    if let Some(k) = guard.as_mut() {
        k.zeroize();
    }
    *guard = None;
}

/// 取出数据密钥的临时副本（Zeroizing：用完即清，不残留裸副本）
pub fn get_key() -> Option<Zeroizing<[u8; 32]>> {
    MASTER_KEY.lock().unwrap().clone()
}

pub fn is_unlocked() -> bool {
    MASTER_KEY.lock().unwrap().is_some()
}

pub fn is_encrypted(s: &str) -> bool {
    s.starts_with(ENC_PREFIX) || s.starts_with(ENC_PREFIX_V2)
}

/// 生成数据密钥（CSPRNG，32 字节）
pub fn generate_data_key() -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    rand::RngCore::fill_bytes(&mut rand::rng(), key.as_mut());
    key
}

/// 用 KEK 包裹 DEK，返回可直接入库的字符串（含 nonce，AES-GCM 自带完整性）
pub fn wrap_data_key(dek: &[u8; 32], kek: &[u8; 32]) -> Result<String, String> {
    crypto::encrypt_with_key(
        &base64::Engine::encode(&base64::engine::general_purpose::STANDARD, dek),
        kek,
    )
}

/// 用 KEK 解开 DEK；密码错误时 AES-GCM 认证失败，返回 Err
pub fn unwrap_data_key(wrapped: &str, kek: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, String> {
    let plain = crypto::decrypt_with_key(wrapped, kek).map_err(|_| "主密码错误".to_string())?;
    let raw = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, plain)
        .map_err(|_| "数据密钥损坏".to_string())?;
    if raw.len() != 32 {
        return Err("数据密钥长度不合法".to_string());
    }
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&raw);
    Ok(key)
}

/// 用指定密钥加密字段（v1 格式：直接用数据密钥，无域分离）
pub fn encrypt_field(plain: &str, key: &[u8; 32]) -> Result<String, String> {
    Ok(format!(
        "{}{}",
        ENC_PREFIX,
        crypto::encrypt_with_key(plain, key)?
    ))
}

/// 用域子密钥加密字段（v2 格式）
pub fn encrypt_field_v2(plain: &str, dek: &[u8; 32]) -> Result<String, String> {
    let sub = derive_domain_key(dek, DOMAIN_FIELD)?;
    Ok(format!(
        "{}{}",
        ENC_PREFIX_V2,
        crypto::encrypt_with_key(plain, &sub)?
    ))
}

/// 解密带 `enc:v1:` 前缀的字段
pub fn decrypt_field(enc: &str, key: &[u8; 32]) -> Result<String, String> {
    let body = enc.strip_prefix(ENC_PREFIX).ok_or("非加密字段")?;
    crypto::decrypt_with_key(body, key)
}

/// 解密任意版本前缀的字段（v1 用裸密钥，v2 用 field 域子密钥）
pub fn decrypt_field_any(enc: &str, dek: &[u8; 32]) -> Result<String, String> {
    if let Some(body) = enc.strip_prefix(ENC_PREFIX_V2) {
        let sub = derive_domain_key(dek, DOMAIN_FIELD)?;
        crypto::decrypt_with_key(body, &sub)
    } else if let Some(body) = enc.strip_prefix(ENC_PREFIX) {
        crypto::decrypt_with_key(body, dek)
    } else {
        Err("非加密字段".to_string())
    }
}

/// 写入前加密：空值/已加密/未解锁时原样返回。新写入一律 v2（域分离）。
pub fn encrypt_if_needed(plain: &str) -> Result<String, String> {
    if plain.is_empty() || is_encrypted(plain) {
        return Ok(plain.to_string());
    }
    match get_key() {
        Some(k) => encrypt_field_v2(plain, &k),
        None => Ok(plain.to_string()),
    }
}

/// 读取时解密：空值/明文原样返回；密文在未解锁时报错
pub fn decrypt_if_needed(value: &str) -> Result<String, String> {
    if value.is_empty() || !is_encrypted(value) {
        return Ok(value.to_string());
    }
    let key = get_key().ok_or("数据已加密，请先解锁")?;
    decrypt_field_any(value, &key)
}

/// 解密失败时的占位符。界面据此提示「只读」，写路径据此拒绝回写。
pub const PLACEHOLDER_LOCKED: &str = "【加密数据 · 需解锁】";

/// 读取容错版：解密失败返回占位符，避免单行异常导致整表加载失败
pub fn decrypt_or_placeholder(value: &str) -> String {
    if value.is_empty() || !is_encrypted(value) {
        return value.to_string();
    }
    match decrypt_if_needed(value) {
        Ok(v) => v,
        Err(_) => PLACEHOLDER_LOCKED.to_string(),
    }
}

/// 该值是否来自「解密失败的占位符」。
///
/// 存在的理由：占位符是读路径的降级产物，**绝不能**被写路径当成真实内容。
/// 用户看到占位符后点保存（资产详情、我的档案都会原样提交表单），
/// 旧实现会把占位符当作明文重新加密写回，原始密文被永久覆盖。
pub fn is_placeholder(value: &str) -> bool {
    value == PLACEHOLDER_LOCKED
}

/// 生成主密码验证器：用**数据密钥**加密固定串。
///
/// 验证器跟数据密钥绑定而不是跟密码派生密钥绑定：
/// 只有持有正确 DEK（即成功解开 wrapped_dek）才能通过校验，
/// 同时该值也用于 `vault_key_matches_config` 判断会话密钥是否与库匹配。
pub fn make_verifier(key: &[u8; 32]) -> Result<String, String> {
    encrypt_field(VERIFIER_PLAINTEXT, key)
}

/// 重加密单值：密文用旧钥解开后以新钥加密；历史明文直接以新钥加密；空值原样。
///
/// 输出统一为 v2（域分离）格式，因此这条路径同时完成「换钥」与「格式升级」。
/// 两层密钥落地后生产路径不再调用（改密 = 重包裹 DEK，O(1)）；保留供
/// legacy 全库升级（`reencrypt_all_to_v2`）与测试使用。
#[cfg_attr(not(test), allow(dead_code))]
pub fn rekey_value(raw: &str, old_key: &[u8; 32], new_key: &[u8; 32]) -> Result<String, String> {
    if raw.is_empty() {
        return Ok(String::new());
    }
    let plain = if is_encrypted(raw) {
        decrypt_field_any(raw, old_key)?
    } else {
        raw.to_string()
    };
    encrypt_field_v2(&plain, new_key)
}

pub fn verify_key(verifier: &str, key: &[u8; 32]) -> bool {
    decrypt_field(verifier, key)
        .map(|s| s == VERIFIER_PLAINTEXT)
        .unwrap_or(false)
}

/// 校验外部链接：仅允许 http/https、必须有主机名、不得带 userinfo。
///
/// 为什么拒绝 userinfo：`https://trusted.example.com@evil.example.com/` 在浏览器里
/// 真实主机是 evil.example.com，但肉眼会读成 trusted。政策补丁里的
/// `official_policy_url` 是用户可导入的外部数据，必须挡住这种钓鱼构造。
pub fn validate_external_url(url: &str) -> Result<String, String> {
    let u = url.trim();
    if u.is_empty() {
        return Err("链接为空".to_string());
    }
    if u.chars().any(|c| c.is_control()) {
        return Err("链接包含非法字符".to_string());
    }
    let lower = u.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("仅允许打开 http/https 链接".to_string());
    }
    let rest = &u[u.find("://").map(|i| i + 3).unwrap_or(u.len())..];
    if rest.is_empty() || rest.starts_with('/') {
        return Err("链接缺少主机名".to_string());
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() {
        return Err("链接缺少主机名".to_string());
    }
    if authority.contains('@') {
        return Err("链接不得包含账号信息（@ 前的主机名不是真实访问目标）".to_string());
    }
    Ok(u.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unlocked_key() -> Zeroizing<[u8; 32]> {
        crypto::derive_key_bytes("master-pw-123", &[9u8; 16])
    }

    #[test]
    fn test_field_encrypt_prefix_and_roundtrip() {
        let _g = test_lock();
        set_key(unlocked_key());
        let enc = encrypt_if_needed("my-account@example.com").unwrap();
        assert!(is_encrypted(&enc), "加密结果应带 enc: 前缀");
        assert!(
            enc.starts_with(ENC_PREFIX_V2),
            "新写入必须是 v2（HKDF 域分离），实际 {}",
            &enc[..8.min(enc.len())]
        );
        assert_eq!(decrypt_if_needed(&enc).unwrap(), "my-account@example.com");
        clear_key();
    }

    #[test]
    fn test_plaintext_passthrough() {
        let _g = test_lock();
        set_key(unlocked_key());
        assert_eq!(decrypt_if_needed("旧的明文数据").unwrap(), "旧的明文数据");
        assert_eq!(decrypt_if_needed("").unwrap(), "");
        clear_key();
    }

    #[test]
    fn test_lock_clears_key_and_blocks_decrypt() {
        let _g = test_lock();
        set_key(unlocked_key());
        let enc = encrypt_if_needed("secret").unwrap();
        clear_key();
        assert!(!is_unlocked());
        assert!(get_key().is_none());
        assert!(decrypt_if_needed(&enc).is_err(), "锁定状态不得解密返回明文");
    }

    #[test]
    fn test_no_key_encrypt_passthrough() {
        let _g = test_lock();
        clear_key();
        assert_eq!(encrypt_if_needed("未解锁状态").unwrap(), "未解锁状态");
        assert_eq!(encrypt_if_needed("").unwrap(), "");
    }

    #[test]
    fn test_verifier_accepts_correct_rejects_wrong() {
        let _g = test_lock();
        let k1 = unlocked_key();
        let v = make_verifier(&k1).unwrap();
        assert!(verify_key(&v, &k1));
        let k2 = crypto::derive_key_bytes("wrong-password", &[9u8; 16]);
        assert!(!verify_key(&v, &k2));
        clear_key();
    }

    #[test]
    fn test_url_whitelist() {
        assert!(validate_external_url("https://www.bilibili.com").is_ok());
        assert!(validate_external_url("http://example.com/path").is_ok());
        assert!(validate_external_url("file:///etc/passwd").is_err());
        assert!(validate_external_url("javascript:alert(1)").is_err());
        assert!(validate_external_url("ftp://example.com").is_err());
        assert!(validate_external_url("").is_err());
        assert!(validate_external_url("https://").is_err());
    }

    /// 安全回归：政策补丁里的 `official_policy_url` 可被任意构造，
    /// 必须挡住 userinfo 欺骗（真实主机在 @ 之后）与控制字符。
    #[test]
    fn url_validation_rejects_userinfo_and_control_chars() {
        for bad in [
            "https://trusted.example.com@evil.example.com/steal",
            "https://user:pass@evil.example.com/",
            "http://a@b@c.com",
            "https://example.com/\r\nX-Injected: 1",
        ] {
            assert!(
                validate_external_url(bad).is_err(),
                "必须拒绝可疑链接: {}",
                bad.escape_debug()
            );
        }

        for good in [
            "https://www.alipay.com",
            "http://127.0.0.1:8080/ok",
            "https://example.com/path?q=1#anchor",
            "https://example.com:443/a/b",
        ] {
            assert!(
                validate_external_url(good).is_ok(),
                "正常链接必须放行: {}",
                good
            );
        }
    }

    #[test]
    fn test_rekey_value() {
        let _g = test_lock();
        let old = crypto::derive_key_bytes("old-pw", &[1u8; 16]);
        let new = crypto::derive_key_bytes("new-pw", &[2u8; 16]);

        let enc_old = encrypt_field("敏感内容", &old).unwrap();
        let enc_new = rekey_value(&enc_old, &old, &new).unwrap();
        assert!(enc_new.starts_with(ENC_PREFIX_V2), "换钥后应升级为 v2 格式");
        assert_eq!(decrypt_field_any(&enc_new, &new).unwrap(), "敏感内容");
        assert!(
            decrypt_field_any(&enc_new, &old).is_err(),
            "旧密钥不得解开新密文"
        );

        let upgraded = rekey_value("历史明文", &old, &new).unwrap();
        assert!(is_encrypted(&upgraded), "明文在重加密时应被顺带加密");
        assert_eq!(decrypt_field_any(&upgraded, &new).unwrap(), "历史明文");

        assert_eq!(rekey_value("", &old, &new).unwrap(), "");
    }

    // ── 两层密钥 ──────────────────────────────────────────

    #[test]
    fn data_key_wrap_unwrap_roundtrip_and_wrong_password() {
        let _g = test_lock();
        let dek = generate_data_key();
        let kek = crypto::derive_key_bytes("master-pw", &[4u8; 16]);
        let wrapped = wrap_data_key(&dek, &kek).unwrap();

        let back = unwrap_data_key(&wrapped, &kek).unwrap();
        assert_eq!(*back, *dek);

        let wrong = crypto::derive_key_bytes("other-pw", &[4u8; 16]);
        assert!(
            unwrap_data_key(&wrapped, &wrong).is_err(),
            "错误密码必须解不开 wrapped DEK"
        );
    }

    #[test]
    fn wrapped_dek_is_rotatable_without_touching_fields() {
        let _g = test_lock();
        let dek = generate_data_key();
        let kek_old = crypto::derive_key_bytes("old", &[1u8; 16]);
        let kek_new = crypto::derive_key_bytes("new", &[2u8; 16]);

        let wrapped_old = wrap_data_key(&dek, &kek_old).unwrap();
        let recovered = unwrap_data_key(&wrapped_old, &kek_old).unwrap();
        let wrapped_new = wrap_data_key(&recovered, &kek_new).unwrap();

        assert_eq!(*unwrap_data_key(&wrapped_new, &kek_new).unwrap(), *dek);
        assert!(unwrap_data_key(&wrapped_new, &kek_old).is_err());

        // 字段用 DEK 加密，KEK 轮换完全不影响
        let field = encrypt_field_v2("hello", &dek).unwrap();
        assert_eq!(decrypt_field_any(&field, &dek).unwrap(), "hello");
    }

    /// HKDF 域分离：同一 DEK 下 field / backup / kit 三个域互不可解。
    #[test]
    fn hkdf_domains_are_separated() {
        let dek = generate_data_key();
        let field = derive_domain_key(&dek, DOMAIN_FIELD).unwrap();
        let backup = derive_domain_key(&dek, DOMAIN_BACKUP).unwrap();
        let kit = derive_domain_key(&dek, DOMAIN_KIT).unwrap();

        assert_ne!(*field, *backup);
        assert_ne!(*field, *kit);
        assert_ne!(*backup, *kit);
        assert_ne!(*field, *dek, "子密钥不得等于数据密钥本身");

        let ct = crypto::encrypt_with_key("secret", &field).unwrap();
        assert!(crypto::decrypt_with_key(&ct, &backup).is_err());
        assert!(crypto::decrypt_with_key(&ct, &kit).is_err());
        assert_eq!(crypto::decrypt_with_key(&ct, &field).unwrap(), "secret");
    }

    /// v1 与 v2 密文在同一把 DEK 下都要能读（滚动升级不丢历史数据）。
    #[test]
    fn v1_and_v2_ciphertexts_coexist() {
        let dek = generate_data_key();
        let v1 = encrypt_field("legacy", &dek).unwrap();
        let v2 = encrypt_field_v2("modern", &dek).unwrap();
        assert!(v1.starts_with(ENC_PREFIX) && !v1.starts_with(ENC_PREFIX_V2));
        assert!(v2.starts_with(ENC_PREFIX_V2));
        assert_eq!(decrypt_field_any(&v1, &dek).unwrap(), "legacy");
        assert_eq!(decrypt_field_any(&v2, &dek).unwrap(), "modern");
    }
}
