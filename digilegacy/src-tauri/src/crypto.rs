//! AES-256-GCM 加密基础设施
//!
//! 提供两套接口：
//! 1. 密码式 encrypt/decrypt（每次调用派生密钥，适合低频、单值场景）；
//! 2. 密钥式 encrypt_with_key/decrypt_with_key（密钥派生一次、批量复用，
//!    供 vault 模块做主密码字段级加密，避免逐字段 PBKDF2 开销）。
//!
//! KDF 三代并存，盐串自描述算法与参数，新写入一律 v3（Argon2id）：
//! - v1：裸 base64 盐 + PBKDF2-HMAC-SHA256 100k（历史数据，只读兼容）
//! - v2：`v2.` + base64 盐 + PBKDF2-HMAC-SHA256 600k
//! - v3：`v3.<m>.<t>.<p>.` + base64 盐 + Argon2id（参数随盐串存储，便于将来上调）
#![allow(dead_code)]

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroizing;

/// v1 迭代数（历史格式兼容读取）
pub const PBKDF2_ITERATIONS_V1: u32 = 100_000;
/// v2 迭代数（OWASP 2023 对 PBKDF2-HMAC-SHA256 的建议：600k）
pub const PBKDF2_ITERATIONS_V2: u32 = 600_000;
/// v2 盐的自描述前缀（'.' 不在 base64 字母表内，与 v1 裸 base64 盐无碰撞可能）
pub const SALT_PREFIX_V2: &str = "v2.";
/// v3 盐/参数前缀：Argon2id
pub const SALT_PREFIX_V3: &str = "v3.";
/// Argon2id 内存代价（KiB）。OWASP 2024 建议的最低配置 19 MiB，
/// 对桌面端解锁耗时约 20-60ms，远低于 PBKDF2 600k 的 300-600ms。
pub const ARGON2_M_COST: u32 = 19 * 1024;
/// Argon2id 迭代次数
pub const ARGON2_T_COST: u32 = 2;
/// Argon2id 并行度
pub const ARGON2_P_COST: u32 = 1;
/// 产品级 KDF 资源上限：外部备份/数据库盐串不能请求失控的内存或 CPU。
pub const MAX_ARGON2_M_COST: u32 = 64 * 1024;
pub const MAX_ARGON2_T_COST: u32 = 10;
pub const MAX_ARGON2_P_COST: u32 = 4;

/// 解析后的 KDF 参数：盐串自描述，调用方不需要知道版本差异
#[derive(Debug, Clone)]
pub enum KdfParams {
    Pbkdf2 {
        salt: Vec<u8>,
        iterations: u32,
    },
    Argon2id {
        salt: Vec<u8>,
        m_cost: u32,
        t_cost: u32,
        p_cost: u32,
    },
}

impl KdfParams {
    /// 按参数派生 32 字节密钥。Argon2id 失败（内存不足等）返回 Err 而不是 panic。
    pub fn derive(&self, password: &str) -> Result<Zeroizing<[u8; 32]>, String> {
        match self {
            Self::Pbkdf2 { salt, iterations } => {
                Ok(derive_key_bytes_v(password, salt, *iterations))
            }
            Self::Argon2id {
                salt,
                m_cost,
                t_cost,
                p_cost,
            } => {
                let params = Params::new(*m_cost, *t_cost, *p_cost, Some(32))
                    .map_err(|e| format!("Argon2 参数无效: {}", e))?;
                let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
                let mut key = Zeroizing::new([0u8; 32]);
                argon
                    .hash_password_into(password.as_bytes(), salt, key.as_mut())
                    .map_err(|e| format!("密钥派生失败: {}", e))?;
                Ok(key)
            }
        }
    }
}

/// 生成随机12字节nonce
pub fn generate_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce);
    nonce
}

/// v1 兼容入口：按旧迭代数派生（历史调用点/测试使用）
pub fn derive_key_bytes(password: &str, salt: &[u8]) -> Zeroizing<[u8; 32]> {
    derive_key_bytes_v(password, salt, PBKDF2_ITERATIONS_V1)
}

/// 按显式迭代数派生 32 字节密钥；Zeroizing 包装保证离开作用域即清零
pub fn derive_key_bytes_v(password: &str, salt: &[u8], iterations: u32) -> Zeroizing<[u8; 32]> {
    let mut key_bytes = Zeroizing::new([0u8; 32]);
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, key_bytes.as_mut());
    key_bytes
}

/// 生成 v2 随机盐（自描述前缀 + base64），存库为 TEXT
pub fn generate_salt_v2() -> String {
    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);
    format!("{}{}", SALT_PREFIX_V2, BASE64.encode(salt))
}

/// 生成 v3 随机盐（Argon2id 参数随串存储，跨版本可解析）
pub fn generate_salt_v3() -> String {
    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);
    format!(
        "{}{}.{}.{}.{}",
        SALT_PREFIX_V3,
        ARGON2_M_COST,
        ARGON2_T_COST,
        ARGON2_P_COST,
        BASE64.encode(salt)
    )
}

/// 解析盐串中的 KDF 参数（v1/v2/v3 自动识别）。
///
/// 用参数枚举而不是「(盐, 迭代数)」二元组：Argon2id 有三个参数，
/// 继续用迭代数会强迫调用方知道版本细节，任何新增算法都要改签名。
pub fn parse_kdf_params(salt_str: &str) -> Result<KdfParams, String> {
    if let Some(rest) = salt_str.strip_prefix(SALT_PREFIX_V3) {
        let mut parts = rest.splitn(4, '.');
        let m = parts.next().and_then(|s| s.parse::<u32>().ok());
        let t = parts.next().and_then(|s| s.parse::<u32>().ok());
        let p = parts.next().and_then(|s| s.parse::<u32>().ok());
        let salt_b64 = parts.next();
        let (Some(m_cost), Some(t_cost), Some(p_cost), Some(salt_b64)) = (m, t, p, salt_b64) else {
            return Err("盐格式无效".to_string());
        };
        if !(8 * 1024..=MAX_ARGON2_M_COST).contains(&m_cost)
            || !(1..=MAX_ARGON2_T_COST).contains(&t_cost)
            || !(1..=MAX_ARGON2_P_COST).contains(&p_cost)
        {
            return Err("Argon2id 参数超出产品安全上限".to_string());
        }
        let salt = BASE64
            .decode(salt_b64)
            .map_err(|_| "盐格式无效".to_string())?;
        Ok(KdfParams::Argon2id {
            salt,
            m_cost,
            t_cost,
            p_cost,
        })
    } else if let Some(rest) = salt_str.strip_prefix(SALT_PREFIX_V2) {
        let salt = BASE64.decode(rest).map_err(|_| "盐格式无效".to_string())?;
        Ok(KdfParams::Pbkdf2 {
            salt,
            iterations: PBKDF2_ITERATIONS_V2,
        })
    } else {
        let salt = BASE64
            .decode(salt_str)
            .map_err(|_| "盐格式无效".to_string())?;
        Ok(KdfParams::Pbkdf2 {
            salt,
            iterations: PBKDF2_ITERATIONS_V1,
        })
    }
}

/// 按盐前缀解析出真实盐字节与应使用的迭代数（无前缀 = v1 裸 base64 盐）
///
/// 仅供 PBKDF2 路径的旧调用点使用；新代码请用 `parse_kdf_params`。
pub fn resolve_salt(salt_str: &str) -> Result<(Vec<u8>, u32), String> {
    match parse_kdf_params(salt_str)? {
        KdfParams::Pbkdf2 { salt, iterations } => Ok((salt, iterations)),
        KdfParams::Argon2id { .. } => Err("该盐使用 Argon2id，不能用 PBKDF2 解析".to_string()),
    }
}

/// 便捷入口：从盐串解析参数并派生密钥（新代码统一走这里）
pub fn derive_from_salt(password: &str, salt_str: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    parse_kdf_params(salt_str)?.derive(password)
}

/// 用已派生密钥加密，输出 base64(nonce(12) || ciphertext)
pub fn encrypt_with_key(plaintext: &str, key: &[u8; 32]) -> Result<String, String> {
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let nonce_bytes = generate_nonce();
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| format!("加密失败: {:?}", e))?;

    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&out))
}

/// 用已派生密钥解密 encrypt_with_key 的输出
pub fn decrypt_with_key(encrypted: &str, key: &[u8; 32]) -> Result<String, String> {
    let data = BASE64
        .decode(encrypted)
        .map_err(|_| "数据格式无效".to_string())?;

    if data.len() < 13 {
        return Err("数据长度不合法".to_string());
    }

    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let nonce_bytes: [u8; 12] = data[0..12]
        .try_into()
        .map_err(|_| "数据长度不合法".to_string())?;
    let nonce = Nonce::from(nonce_bytes);

    let plaintext = cipher
        .decrypt(&nonce, &data[12..])
        .map_err(|_| "解密失败，请检查主密码".to_string())?;

    String::from_utf8(plaintext).map_err(|_| "解密后数据编码错误".to_string())
}

/// 生成随机盐
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 迭代次数是密钥强度本身：必须用字面量钉死，
    /// 如果断言写成 `== PBKDF2_ITERATIONS_V2` 就是同义反复——
    /// 把常量从 600_000 改成 1_000，测试依然全绿（变异实测确认过）。
    #[test]
    fn test_salt_prefix_selects_iterations() {
        assert_eq!(PBKDF2_ITERATIONS_V1, 100_000, "v1 兼容路径迭代数不得下调");
        assert_eq!(
            PBKDF2_ITERATIONS_V2, 600_000,
            "v2 迭代数必须保持 60 万（OWASP 2023 建议量级）"
        );

        // v1 裸盐与 v2 前缀盐分别解析出不同迭代数
        let raw = [1u8; 16];
        let (bytes1, iters1) = resolve_salt(&BASE64.encode(raw)).unwrap();
        assert_eq!(iters1, 100_000);
        assert_eq!(bytes1, raw.to_vec());

        let salt_v2 = generate_salt_v2();
        assert!(salt_v2.starts_with(SALT_PREFIX_V2));
        let (bytes2, iters2) = resolve_salt(&salt_v2).unwrap();
        assert_eq!(iters2, 600_000);
        assert_eq!(bytes2.len(), 16);
    }

    /// v3（Argon2id）：参数自描述、可解析、可复现；错误输入的盐必须报错而不是静默降级。
    #[test]
    fn test_v3_rejects_product_limit_excesses() {
        let salt = BASE64.encode([7u8; 16]);
        for bad in [
            format!("v3.{}.2.1.{}", MAX_ARGON2_M_COST + 1, salt),
            format!("v3.19456.{}.1.{}", MAX_ARGON2_T_COST + 1, salt),
            format!("v3.19456.2.{}.{}", MAX_ARGON2_P_COST + 1, salt),
            format!("v3.{}.{}.{}.{}", u32::MAX, u32::MAX, u32::MAX, salt),
        ] {
            assert!(
                parse_kdf_params(&bad).is_err(),
                "恶意 KDF 参数必须拒绝: {}",
                bad
            );
        }
    }

    #[test]
    fn test_v3_argon2id_params_roundtrip() {
        assert_eq!(ARGON2_M_COST, 19 * 1024, "内存代价不得低于 OWASP 最低配置");
        assert_eq!(ARGON2_T_COST, 2, "迭代次数不得下调");

        let salt_v3 = generate_salt_v3();
        assert!(salt_v3.starts_with(SALT_PREFIX_V3));
        match parse_kdf_params(&salt_v3).unwrap() {
            KdfParams::Argon2id {
                salt,
                m_cost,
                t_cost,
                p_cost,
            } => {
                assert_eq!(salt.len(), 16);
                assert_eq!(m_cost, ARGON2_M_COST);
                assert_eq!(t_cost, ARGON2_T_COST);
                assert_eq!(p_cost, ARGON2_P_COST);
            }
            other => panic!("v3 盐必须解析为 Argon2id，实际 {:?}", other),
        }

        // 同一密码 + 同一盐必须得到同一密钥（确定性），不同盐不同密钥
        let k1 = derive_from_salt("pw", &salt_v3).unwrap();
        let k2 = derive_from_salt("pw", &salt_v3).unwrap();
        assert_eq!(*k1, *k2);
        let salt_other = generate_salt_v3();
        let k3 = derive_from_salt("pw", &salt_other).unwrap();
        assert_ne!(*k1, *k3);

        // 密文往返：v3 派生的密钥能正常加密/解密
        let ct = encrypt_with_key("敏感数据", &k1).unwrap();
        assert_eq!(decrypt_with_key(&ct, &k1).unwrap(), "敏感数据");
    }

    /// v3 盐不能被 PBKDF2 的旧入口误解析——静默用错算法会得到错误密钥。
    #[test]
    fn test_resolve_salt_rejects_argon2_salt() {
        let salt_v3 = generate_salt_v3();
        assert!(resolve_salt(&salt_v3).is_err(), "v3 盐不得被 PBKDF2 解析");

        // 损坏的 v3 串必须报错
        for bad in ["v3.", "v3.abc.2.1.c2FsdA==", "v3.19456", "v3.19456.2.1"] {
            assert!(
                parse_kdf_params(bad).is_err(),
                "损坏的 v3 盐必须拒绝: {}",
                bad
            );
        }
    }

    #[test]
    fn test_v2_derivation_matches_v1_semantics() {
        // 相同密码与盐字节：显式传 v1 迭代数的结果必须与兼容入口一致
        let a = derive_key_bytes("pw", &[9u8; 16]);
        let b = derive_key_bytes_v("pw", &[9u8; 16], PBKDF2_ITERATIONS_V1);
        assert_eq!(*a, *b);
        // v2 迭代数不同 → 密钥不同
        let c = derive_key_bytes_v("pw", &[9u8; 16], PBKDF2_ITERATIONS_V2);
        assert_ne!(*a, *c);
    }

    #[test]
    fn test_wrong_password_fails() {
        let key = derive_key_bytes("correct_password", &[3u8; 16]);
        let encrypted = encrypt_with_key("test", &key).unwrap();
        let wrong = derive_key_bytes("wrong_password", &[3u8; 16]);
        assert!(decrypt_with_key(&encrypted, &wrong).is_err());
    }

    #[test]
    fn test_deterministic_key_derivation() {
        let password = "my_secure_password";
        let salt = [1u8; 16];
        let key1 = derive_key_bytes(password, &salt);
        let key2 = derive_key_bytes(password, &salt);
        assert_eq!(key1, key2, "相同密码和盐应产生相同密钥");
    }

    #[test]
    fn test_different_salts_produce_different_keys() {
        let password = "my_secure_password";
        let salt1 = [1u8; 16];
        let salt2 = [2u8; 16];
        let key1 = derive_key_bytes(password, &salt1);
        let key2 = derive_key_bytes(password, &salt2);
        assert_ne!(key1, key2, "不同盐应产生不同密钥");
    }

    #[test]
    fn test_unicode_password() {
        let key = derive_key_bytes("密码测试123!@#", &[5u8; 16]);
        let encrypted = encrypt_with_key("敏感数据", &key).unwrap();
        assert_eq!(decrypt_with_key(&encrypted, &key).unwrap(), "敏感数据");
    }

    #[test]
    fn test_key_based_roundtrip() {
        let key = derive_key_bytes("master-pw", &[7u8; 16]);
        let ct = encrypt_with_key("敏感内容-账号密码", &key).unwrap();
        assert_ne!(ct, "敏感内容-账号密码");
        assert_eq!(decrypt_with_key(&ct, &key).unwrap(), "敏感内容-账号密码");
    }

    #[test]
    fn test_key_based_tamper_fails() {
        let key = derive_key_bytes("master-pw", &[7u8; 16]);
        let ct = encrypt_with_key("data", &key).unwrap();
        let mut bytes = BASE64.decode(&ct).unwrap();
        let n = bytes.len();
        bytes[n - 1] ^= 0x01;
        let tampered = BASE64.encode(&bytes);
        assert!(decrypt_with_key(&tampered, &key).is_err());
    }

    #[test]
    fn test_key_based_empty_roundtrip() {
        let key = derive_key_bytes("master-pw", &[7u8; 16]);
        let ct = encrypt_with_key("", &key).unwrap();
        assert_eq!(decrypt_with_key(&ct, &key).unwrap(), "");
    }
}
