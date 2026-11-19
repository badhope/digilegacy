//! Shamir 3-of-N recovery shares (SEC-03).
//!
//! Shares never land in the database. A share carries a checksum and a batch
//! fingerprint so mixed/corrupted paper cards fail loudly instead of rebuilding
//! a wrong key. Optional passphrase wraps the secret before splitting.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use blahaj::{Share, Sharks};
use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::convert::TryFrom;
use zeroize::Zeroizing;

use crate::crypto;

pub const SHARE_PREFIX: &str = "DGL1";
pub const MIN_THRESHOLD: u8 = 2;
pub const MAX_SHARES: u8 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KitMeta {
    pub threshold: u8,
    pub total: u8,
    pub fingerprint: String,
    pub created_at: String,
    pub passphrase_protected: bool,
}

#[derive(Debug, Clone)]
pub struct ShareKit {
    pub codes: Vec<String>,
    pub meta: KitMeta,
    pub kdf_salt: Option<String>,
}

pub fn fingerprint_of(key: &[u8; 32]) -> String {
    let digest = Sha256::digest(key);
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

fn build_payload(
    key: &[u8; 32],
    passphrase: Option<&str>,
) -> Result<(Vec<u8>, Option<String>, bool), String> {
    match passphrase.map(str::trim).filter(|p| !p.is_empty()) {
        None => Ok((key.to_vec(), None, false)),
        Some(pass) => {
            if pass.chars().count() < 8 {
                return Err("恢复口令至少需要 8 个字符".to_string());
            }
            let salt = crypto::generate_salt_v3();
            let wrap = crypto::derive_from_salt(pass, &salt)?;
            let sealed = crypto::encrypt_with_key(&BASE64.encode(key), &wrap)?;
            let bytes = BASE64
                .decode(&sealed)
                .map_err(|e| format!("封装失败: {}", e))?;
            Ok((bytes, Some(salt), true))
        }
    }
}

/// `DGL1-<base32(share bytes || sha256_2)>`, eight characters per group.
pub fn encode_share(share: &Share) -> String {
    let mut bytes: Vec<u8> = Vec::from(share);
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest[..2]);
    let b32 = BASE32_NOPAD.encode(&bytes);
    let mut out = String::with_capacity(b32.len() + 16);
    out.push_str(SHARE_PREFIX);
    out.push('-');
    for (i, ch) in b32.chars().enumerate() {
        if i > 0 && i % 8 == 0 {
            out.push('-');
        }
        out.push(ch);
    }
    out
}

/// Decode one paper share; checksum is mandatory.
pub fn decode_share(code: &str) -> Result<Share, String> {
    let trimmed = code.trim();
    let rest = trimmed
        .strip_prefix(SHARE_PREFIX)
        .ok_or("份额格式不正确（缺少 DGL1 前缀）")?;
    let compact: String = rest
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let bytes = BASE32_NOPAD
        .decode(compact.as_bytes())
        .map_err(|_| "份额格式不正确（编码损坏）".to_string())?;
    if bytes.len() < 4 {
        return Err("份额长度不足".to_string());
    }
    let (payload, checksum) = bytes.split_at(bytes.len() - 2);
    let digest = Sha256::digest(payload);
    if digest[..2] != *checksum {
        return Err("份额校验失败（可能抄错了一个字符）".to_string());
    }
    Share::try_from(payload).map_err(|e| format!("份额内容无效: {}", e))
}

/// Split the data key into `total` shares with `threshold` reconstruction.
pub fn generate_shares(
    key: &[u8; 32],
    threshold: u8,
    total: u8,
    passphrase: Option<&str>,
) -> Result<ShareKit, String> {
    if threshold < MIN_THRESHOLD || threshold > total || total > MAX_SHARES {
        return Err(format!(
            "门限与份数不合法（要求 {} ≤ 门限 ≤ 份数 ≤ {}）",
            MIN_THRESHOLD, MAX_SHARES
        ));
    }
    let (payload, kdf_salt, protected) = build_payload(key, passphrase)?;
    let sharks = Sharks(threshold);
    let shares: Vec<Share> = sharks.dealer(&payload).take(total as usize).collect();
    if shares.len() != total as usize {
        return Err("份额生成失败".to_string());
    }
    Ok(ShareKit {
        codes: shares.iter().map(encode_share).collect(),
        meta: KitMeta {
            threshold,
            total,
            fingerprint: fingerprint_of(key),
            created_at: chrono::Utc::now().to_rfc3339(),
            passphrase_protected: protected,
        },
        kdf_salt,
    })
}

/// Rebuild a data key from at least `meta.threshold` valid shares.
pub fn recover_from_shares(
    codes: &[String],
    meta: &KitMeta,
    passphrase: Option<&str>,
    kdf_salt: Option<&str>,
) -> Result<Zeroizing<[u8; 32]>, String> {
    if codes.len() < meta.threshold as usize {
        return Err(format!(
            "份额不足：需要至少 {} 份，当前 {} 份",
            meta.threshold,
            codes.len()
        ));
    }
    let mut parsed: Vec<Share> = Vec::with_capacity(codes.len());
    for code in codes {
        parsed.push(decode_share(code)?);
    }
    let sharks = Sharks(meta.threshold);
    let payload = sharks
        .recover(&parsed)
        .map_err(|_| "份额无法重建密钥（可能来自不同批次）".to_string())?;

    let key: Zeroizing<[u8; 32]> = if meta.passphrase_protected {
        let pass = passphrase
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .ok_or("该恢复包受口令保护，请输入恢复口令")?;
        let salt = kdf_salt.ok_or("恢复包元数据缺少 KDF 盐")?;
        let wrap = crypto::derive_from_salt(pass, salt)?;
        let sealed = BASE64.encode(&payload);
        let plain_b64 =
            crypto::decrypt_with_key(&sealed, &wrap).map_err(|_| "恢复口令不正确".to_string())?;
        let raw = BASE64
            .decode(plain_b64)
            .map_err(|_| "主密钥数据损坏".to_string())?;
        if raw.len() != 32 {
            return Err("主密钥长度不合法".to_string());
        }
        let mut k = Zeroizing::new([0u8; 32]);
        k.copy_from_slice(&raw);
        k
    } else {
        if payload.len() != 32 {
            return Err("份额重建结果长度不合法".to_string());
        }
        let mut k = Zeroizing::new([0u8; 32]);
        k.copy_from_slice(&payload);
        k
    };

    if fingerprint_of(&key) != meta.fingerprint {
        return Err("份额属于旧批次（主密码可能已变更），请重新生成恢复包".to_string());
    }
    Ok(key)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Printable HTML recovery kit, one card per page.
pub fn render_kit_html(meta: &KitMeta, codes: &[String]) -> String {
    let passphrase_note = if meta.passphrase_protected {
        "本批份额受恢复口令保护：还需要所有人共用的口令才能解开。"
    } else {
        "本批份额未设置额外口令：集齐任意若干份即可重建密钥，请分散保管。"
    };
    let mut cards = String::new();
    for (idx, code) in codes.iter().enumerate() {
        cards.push_str(&format!(
            r#"<section class="card">
  <h2>继承人恢复份额 {n} / {total}</h2>
  <dl>
    <dt>批次指纹</dt><dd>{fp}</dd>
    <dt>门限</dt><dd>任意 {k} 份即可恢复</dd>
    <dt>生成时间</dt><dd>{at}</dd>
  </dl>
  <p class="code">{code}</p>
  <p class="hint">请抄写或打印后单独存放。不要把多份放在同一个地方。</p>
  <p class="hint">{pass_note}</p>
</section>"#,
            n = idx + 1,
            total = meta.total,
            fp = meta.fingerprint,
            k = meta.threshold,
            at = meta.created_at,
            code = html_escape(code),
            pass_note = passphrase_note,
        ));
    }
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN"><head><meta charset="UTF-8"><title>DigiLegacy · 继承人恢复包</title>
<style>
body{{font-family:-apple-system,"Segoe UI","Microsoft YaHei",sans-serif;background:#f6f3ee;color:#232019;margin:0;padding:24px;}}
.card{{background:#fff;border:1px solid #e4ddd3;border-radius:12px;padding:24px;margin:0 auto 24px;max-width:640px;page-break-after:always;}}
h1{{text-align:center;font-size:20px;margin:8px 0 24px;}}
h2{{font-size:16px;margin:0 0 12px;color:#9a473b;}}
dl{{display:grid;grid-template-columns:96px 1fr;gap:6px 12px;font-size:13px;margin:0 0 12px;}}
dt{{color:#6b6157;}} dd{{margin:0;}}
.code{{font-family:ui-monospace,Consolas,monospace;font-size:13px;line-height:1.7;background:#f6f3ee;border:1px dashed #cfc6ba;border-radius:8px;padding:12px;word-break:break-all;}}
.hint{{font-size:12px;color:#6b6157;margin:8px 0 0;}}
@media print{{body{{background:#fff;padding:0;}}.card{{border:none;box-shadow:none;}}}}
</style></head><body>
<h1>DigiLegacy · 继承人恢复包</h1>
<p class="hint" style="max-width:640px;margin:0 auto 24px;text-align:center;">
任何人集齐其中 {k} 份即可在 DigiLegacy 中选择「用恢复份额解锁」。{pass_note}
</p>
{cards}
</body></html>"#,
        k = meta.threshold,
        pass_note = passphrase_note,
        cards = cards,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_of_five_recovers_same_key() {
        let key = [7u8; 32];
        let kit = generate_shares(&key, 3, 5, None).unwrap();
        assert_eq!(kit.codes.len(), 5);
        assert_eq!(kit.meta.threshold, 3);
        assert_eq!(kit.meta.total, 5);
        assert_eq!(kit.meta.fingerprint, fingerprint_of(&key));
        let recovered = recover_from_shares(&kit.codes[0..3], &kit.meta, None, None).unwrap();
        assert_eq!(*recovered, key);
    }

    #[test]
    fn two_of_five_is_rejected() {
        let key = [7u8; 32];
        let kit = generate_shares(&key, 3, 5, None).unwrap();
        assert!(recover_from_shares(&kit.codes[0..2], &kit.meta, None, None).is_err());
    }

    #[test]
    fn corrupted_share_is_rejected() {
        let key = [7u8; 32];
        let kit = generate_shares(&key, 2, 3, None).unwrap();
        let mut broken = kit.codes[0].clone();
        let last = broken.pop().unwrap();
        broken.push(if last == 'A' { 'B' } else { 'A' });
        assert!(decode_share(&broken).is_err());
        assert!(
            recover_from_shares(&[broken, kit.codes[1].clone()], &kit.meta, None, None).is_err()
        );
    }

    #[test]
    fn mixed_batches_are_rejected() {
        let key = [7u8; 32];
        let kit = generate_shares(&key, 2, 3, None).unwrap();
        let other = generate_shares(&[8u8; 32], 2, 3, None).unwrap();
        assert!(recover_from_shares(
            &[kit.codes[0].clone(), other.codes[1].clone()],
            &kit.meta,
            None,
            None
        )
        .is_err());
    }

    #[test]
    fn passphrase_protection_roundtrip() {
        let key = [7u8; 32];
        let kit = generate_shares(&key, 3, 5, Some("correct horse battery")).unwrap();
        assert!(kit.meta.passphrase_protected);
        let recovered = recover_from_shares(
            &kit.codes[0..3],
            &kit.meta,
            Some("correct horse battery"),
            kit.kdf_salt.as_deref(),
        )
        .unwrap();
        assert_eq!(*recovered, key);
        assert!(recover_from_shares(
            &kit.codes[0..3],
            &kit.meta,
            Some("wrong"),
            kit.kdf_salt.as_deref()
        )
        .is_err());
    }

    #[test]
    fn bounds_are_enforced() {
        let key = [1u8; 32];
        assert!(generate_shares(&key, 1, 3, None).is_err());
        assert!(generate_shares(&key, 4, 3, None).is_err());
        assert!(generate_shares(&key, 3, 11, None).is_err());
    }

    #[test]
    fn kit_html_contains_codes_but_not_key_bytes() {
        let key = [11u8; 32];
        let kit = generate_shares(&key, 3, 5, None).unwrap();
        let html = render_kit_html(&kit.meta, &kit.codes);
        for code in &kit.codes {
            assert!(html.contains(code));
        }
        assert!(!html.contains(&BASE64.encode(key)));
    }
}
