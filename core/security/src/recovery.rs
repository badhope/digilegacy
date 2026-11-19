//! Recovery-code sealing (SEC-03).
//!
//! Migrated from the legacy desktop app: the 24-character code derives a key
//! with Argon2id and seals the 32-byte data key. v1/v2 seals stay readable.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use zeroize::Zeroizing;

use crate::crypto;

const SPLIT_DASH_EVERY: usize = 4;
const SEAL_PREFIX_V3: &str = "RS3.";
const SEAL_PREFIX_V2: &str = "RS2.";

/// Generate a 24-character recovery code grouped as XXXX-XXXX-XXXX-XXXX-XXXX-XXXX.
pub fn generate_recovery_code() -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 15];
    rand::rng().fill_bytes(&mut bytes);
    let mut out = String::with_capacity(29);
    let mut bits: u32 = 0;
    let mut acc: u32 = 0;
    for &b in bytes.iter() {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((acc >> bits) & 0x1f) as usize;
            out.push(ALPHABET[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((acc << (5 - bits)) & 0x1f) as usize;
        out.push(ALPHABET[idx] as char);
    }
    let mut grouped = String::new();
    for (i, ch) in out.chars().take(24).enumerate() {
        if i > 0 && i % SPLIT_DASH_EVERY == 0 {
            grouped.push('-');
        }
        grouped.push(ch);
    }
    grouped
}

/// Normalize a user-entered code: strip whitespace/dashes, uppercase.
pub fn normalize_recovery_code(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Seal the data key with a fresh recovery code. Returns (code, RS3 seal).
pub fn create_recovery_seal(master_key: &[u8; 32]) -> Result<(String, String), String> {
    let code = generate_recovery_code();
    let kdf_salt = crypto::generate_salt_v3();
    let rec_key = crypto::derive_from_salt(&normalize_recovery_code(&code), &kdf_salt)?;
    let sealed = crypto::encrypt_with_key(&BASE64.encode(master_key), &rec_key)?;
    let sealed_bytes = BASE64
        .decode(&sealed)
        .map_err(|e| format!("封装修理失败: {}", e))?;
    let salt_bytes = kdf_salt.as_bytes();
    if salt_bytes.len() > 255 {
        return Err("内部错误：KDF 盐过长".to_string());
    }
    let mut combined = Vec::with_capacity(1 + salt_bytes.len() + sealed_bytes.len());
    combined.push(salt_bytes.len() as u8);
    combined.extend_from_slice(salt_bytes);
    combined.extend_from_slice(&sealed_bytes);
    Ok((
        code,
        format!("{}{}", SEAL_PREFIX_V3, BASE64.encode(&combined)),
    ))
}

pub fn validate_recovery_seal_structure(sealed_b64: &str) -> Result<(), String> {
    if sealed_b64.trim().is_empty() {
        return Err("恢复封印为空".to_string());
    }
    let (body, v3) = if let Some(rest) = sealed_b64.strip_prefix(SEAL_PREFIX_V3) {
        (rest, true)
    } else if let Some(rest) = sealed_b64.strip_prefix(SEAL_PREFIX_V2) {
        (rest, false)
    } else {
        (sealed_b64, false)
    };
    let combined = BASE64
        .decode(body)
        .map_err(|_| "恢复封印不是有效 Base64".to_string())?;
    if v3 {
        let salt_len = *combined.first().ok_or("恢复封印长度不合法")? as usize;
        if salt_len == 0 || combined.len() < 1 + salt_len + 13 {
            return Err("恢复封印长度不合法".to_string());
        }
        let salt = std::str::from_utf8(&combined[1..1 + salt_len])
            .map_err(|_| "恢复封印 KDF 盐损坏".to_string())?;
        crypto::parse_kdf_params(salt).map_err(|_| "恢复封印 KDF 参数无效".to_string())?;
    } else if combined.len() < 16 + 13 {
        return Err("恢复封印长度不合法".to_string());
    }
    Ok(())
}

/// Open a sealed data key. RS3=Argon2id, RS2=PBKDF2 600k, bare=v1 100k.
pub fn unseal_with_recovery(code: &str, sealed_b64: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    let normalized = normalize_recovery_code(code);

    let (rec_key, sealed) = if let Some(rest) = sealed_b64.strip_prefix(SEAL_PREFIX_V3) {
        let combined = BASE64
            .decode(rest)
            .map_err(|_| "恢复密钥数据损坏".to_string())?;
        let salt_len = *combined.first().ok_or("恢复密钥数据长度不合法")? as usize;
        if salt_len == 0 || combined.len() < 1 + salt_len + 1 {
            return Err("恢复密钥数据长度不合法".to_string());
        }
        let kdf_salt = std::str::from_utf8(&combined[1..1 + salt_len])
            .map_err(|_| "恢复密钥数据损坏".to_string())?;
        let key = crypto::derive_from_salt(&normalized, kdf_salt)?;
        (key, BASE64.encode(&combined[1 + salt_len..]))
    } else {
        let (iterations, body) = if let Some(rest) = sealed_b64.strip_prefix(SEAL_PREFIX_V2) {
            (crypto::PBKDF2_ITERATIONS_V2, rest)
        } else {
            (crypto::PBKDF2_ITERATIONS_V1, sealed_b64)
        };
        let combined = BASE64
            .decode(body)
            .map_err(|_| "恢复密钥数据损坏".to_string())?;
        if combined.len() < 17 {
            return Err("恢复密钥数据长度不合法".to_string());
        }
        let salt = &combined[0..16];
        let key = crypto::derive_key_bytes_v(&normalized, salt, iterations);
        (key, BASE64.encode(&combined[16..]))
    };

    let master_b64 = crypto::decrypt_with_key(&sealed, &rec_key)?;
    let master = BASE64
        .decode(master_b64)
        .map_err(|_| "主密钥数据损坏".to_string())?;
    if master.len() != 32 {
        return Err("主密钥长度不合法".to_string());
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&master);
    Ok(Zeroizing::new(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_code_shape() {
        let code = generate_recovery_code();
        assert_eq!(code.len(), 29, "24 字符 + 5 个连字符");
        assert_eq!(code.matches('-').count(), 5);
        let bad: Vec<char> = code
            .chars()
            .filter(|c| ['I', 'O', '0', '1'].contains(c))
            .collect();
        assert!(bad.is_empty(), "出现易混字符: {:?} in {}", bad, code);
    }

    #[test]
    fn seal_unseal_roundtrip() {
        let master = [7u8; 32];
        let (code, sealed) = create_recovery_seal(&master).unwrap();
        let unsealed = unseal_with_recovery(&code, &sealed).unwrap();
        assert_eq!(*unsealed, master);
    }

    #[test]
    fn wrong_code_rejected() {
        let master = [7u8; 32];
        let (_code, sealed) = create_recovery_seal(&master).unwrap();
        assert!(unseal_with_recovery("AAAA-BBBB-CCCC-DDDD-EEEE-FFFF", &sealed).is_err());
    }

    #[test]
    fn normalization_strips_dashes_and_case() {
        assert_eq!(normalize_recovery_code("abcd-efgh"), "ABCDEFGH");
        assert_eq!(normalize_recovery_code(" ab cd "), "ABCD");
    }

    #[test]
    fn codes_are_unique() {
        let a = generate_recovery_code();
        let b = generate_recovery_code();
        assert_ne!(a, b);
    }

    #[test]
    fn legacy_v1_seal_without_prefix_still_opens() {
        let master = [3u8; 32];
        let code = "ABCD-EFGH-JKLM-NPQR-STUV-WXYZ";
        let salt = crypto::generate_salt();

        let rec_key = crypto::derive_key_bytes_v(
            &normalize_recovery_code(code),
            &salt,
            crypto::PBKDF2_ITERATIONS_V1,
        );
        let sealed = crypto::encrypt_with_key(&BASE64.encode(master), &rec_key).unwrap();
        let mut combined = salt.to_vec();
        combined.extend_from_slice(&BASE64.decode(&sealed).unwrap());
        let legacy_seal = BASE64.encode(&combined);
        assert!(!legacy_seal.starts_with(SEAL_PREFIX_V2));

        let opened = unseal_with_recovery(code, &legacy_seal).expect("v1 旧封印必须仍可解开");
        assert_eq!(*opened, master);
    }

    #[test]
    fn v1_v2_v3_seals_coexist() {
        let master = [9u8; 32];
        let (code, v3_seal) = create_recovery_seal(&master).unwrap();
        assert!(v3_seal.starts_with(SEAL_PREFIX_V3), "新封印必须是 RS3");
        assert_eq!(*unseal_with_recovery(&code, &v3_seal).unwrap(), master);

        let salt = crypto::generate_salt();
        let rec_key_v2 = crypto::derive_key_bytes_v(
            &normalize_recovery_code(&code),
            &salt,
            crypto::PBKDF2_ITERATIONS_V2,
        );
        let sealed_v2 = crypto::encrypt_with_key(&BASE64.encode(master), &rec_key_v2).unwrap();
        let mut combined_v2 = salt.to_vec();
        combined_v2.extend_from_slice(&BASE64.decode(&sealed_v2).unwrap());
        let v2_seal = format!("{}{}", SEAL_PREFIX_V2, BASE64.encode(&combined_v2));
        assert_eq!(*unseal_with_recovery(&code, &v2_seal).unwrap(), master);

        let salt1 = crypto::generate_salt();
        let rec_key_v1 = crypto::derive_key_bytes_v(
            &normalize_recovery_code(&code),
            &salt1,
            crypto::PBKDF2_ITERATIONS_V1,
        );
        let sealed_v1 = crypto::encrypt_with_key(&BASE64.encode(master), &rec_key_v1).unwrap();
        let mut combined_v1 = salt1.to_vec();
        combined_v1.extend_from_slice(&BASE64.decode(&sealed_v1).unwrap());
        let v1_seal = BASE64.encode(&combined_v1);

        assert_eq!(*unseal_with_recovery(&code, &v1_seal).unwrap(), master);
    }

    #[test]
    fn corrupt_v3_seal_rejected() {
        for bad in ["RS3.", "RS3.AAAA", "RS3."] {
            assert!(unseal_with_recovery("ABCD-EFGH-JKLM-NPQR-STUV-WXYZ", bad).is_err());
        }
        let payload = BASE64.encode([200u8, 1, 2, 3]);
        assert!(
            unseal_with_recovery("ABCD-EFGH-JKLM-NPQR-STUV-WXYZ", &format!("RS3.{}", payload))
                .is_err()
        );
    }
}
