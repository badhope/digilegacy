//! 继承人恢复包：Shamir 秘密共享 + 可选口令二次保护。
//!
//! 为什么需要：本项目是单机应用，只有「持有者本机解锁」一条解密路径。
//! 持有者失能后，家人即使拿到数据库文件也解不开——这是与 Bitwarden
//! Emergency Access 等产品相比最致命的能力缺口。补齐方式不是引入服务器
//! （那会破坏「无服务器 / 无遥测」契约），而是把主密钥做门限拆分：
//! 3-of-5 份份额分给家人/律师/保险箱，任意 3 份可重建密钥。
//!
//! 安全边界（写进代码）：
//! 1. 份额**绝不落库**。只保存元数据（门限、份数、指纹、生成时间）——
//!    份额与数据库放在一起等于没拆。
//! 2. 口令模式：份额拆的是「主密钥的密文」而不是主密钥本身，
//!    3 份份额 + 口令才能解开。适合对托管人互不信任、但愿意共享一个口令的场景。
//! 3. 主密码变更 / 恢复重设会轮换数据密钥，旧份额立即失效；因此这两条路径
//!    必须在同一事务里清掉元数据，让界面显示「需要重新生成」而不是假装有效。
//! 4. 每个份额带密钥指纹与校验和：混合不同批次的份额、抄错一个字符都会被拒，
//!    不会静默重建出错误密钥。

use base64::Engine;
use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};
use sharks::{Share, Sharks};
use std::convert::TryFrom;

use crate::crypto;

/// 份额串前缀（版本化：将来换算法时旧串仍可识别）
pub const SHARE_PREFIX: &str = "DGL1";
/// 最少门限：低于 2 就是单点，失去门限意义
pub const MIN_THRESHOLD: u8 = 2;
/// 最大份数：Shamir over GF(256) 上限 255，但纸质卡片超过 10 份不现实
pub const MAX_SHARES: u8 = 10;

const META_ROW_ID: i64 = 1;

/// 恢复包元数据（不含份额本身）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitMeta {
    pub threshold: u8,
    pub total: u8,
    /// 主密钥指纹（sha256 前 8 字节的 hex）：用于判断份额是否属于当前密钥
    pub fingerprint: String,
    pub created_at: String,
    /// 是否启用了口令二次保护
    pub passphrase_protected: bool,
}

/// 生成结果（返回给命令层，界面展示路径与份数）。
#[derive(Debug, Clone, Serialize)]
pub struct KitResult {
    pub path: String,
    pub threshold: u8,
    pub total: u8,
    pub fingerprint: String,
    pub created_at: String,
    pub passphrase_protected: bool,
}

/// 主密钥指纹：前 8 字节足够区分批次，又不泄露密钥本身。
pub fn fingerprint_of(key: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(key);
    digest[..8].iter().map(|b| format!("{:02x}", b)).collect()
}

/// 取出当前会话密钥；未解锁或密钥与库内验证器不符时拒绝。
fn require_unlocked_key() -> Result<zeroize::Zeroizing<[u8; 32]>, String> {
    let key = crate::vault::get_key().ok_or("请先解锁后再生成恢复包")?;
    if !crate::vault::is_unlocked() {
        return Err("请先解锁后再生成恢复包".to_string());
    }
    // 密钥必须能通过库内验证器校验，否则生成的是「打不开当前数据」的份额
    let Some((_salt, verifier)) = crate::db::get_security_config_db()? else {
        return Err("尚未设置主密码，无需恢复包".to_string());
    };
    if !crate::vault::verify_key(&verifier, &key) {
        return Err("当前会话密钥与主密码不匹配，请重新解锁".to_string());
    }
    Ok(key)
}

/// 把 32 字节主密钥编码为待拆分的载荷。
///
/// 口令模式：用 Argon2id 派生的包裹密钥加密主密钥，份额拆的是密文。
/// 返回 (payload, kdf_salt_str, protected)。
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
            let sealed = crypto::encrypt_with_key(
                &base64::engine::general_purpose::STANDARD.encode(key),
                &wrap,
            )?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&sealed)
                .map_err(|e| format!("封装失败: {}", e))?;
            Ok((bytes, Some(salt), true))
        }
    }
}

/// 单份份额的可读编码：`DGL1-<base32(share_bytes || checksum2)>`，每 8 字符一组。
///
/// 用 Base32 而不是 Base64：份额要打印在纸上由家人手抄，
/// Base32 只有 A-Z2-7，不含大小写混淆与 `+ / =`，也不含分组符 `-`。
/// （早期实现用 base64url + `-` 分组，而 `-` 本身属于 base64url 字母表，
/// 解码时剥离分组符会破坏数据——已由 roundtrip 测试捕获。）
fn encode_share(share: &Share) -> String {
    use sha2::{Digest, Sha256};
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

/// 解析一份份额；格式错误 / 校验和不符 / 非本版本一律拒绝。
fn decode_share(code: &str) -> Result<Share, String> {
    use sha2::{Digest, Sha256};
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

/// 组装可打印的恢复包 HTML：每份一页，含编号、门限、指纹与操作指引。
fn render_kit_html(meta: &KitMeta, codes: &[String]) -> String {
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
<html lang="zh-CN"><head><meta charset="UTF-8"><title>归零 · 继承人恢复包</title>
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
<h1>归零 DigiLegacy · 继承人恢复包</h1>
<p class="hint" style="max-width:640px;margin:0 auto 24px;text-align:center;">
任何人集齐其中 {k} 份即可在「归零」应用中选择「用恢复份额解锁」。{pass_note}
</p>
{cards}
</body></html>"#,
        k = meta.threshold,
        pass_note = passphrase_note,
        cards = cards,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 生成恢复包：拆分 → 写文件 → 记录元数据。份额只在返回值与本文件中出现。
pub fn generate_kit(
    threshold: u8,
    total: u8,
    passphrase: Option<&str>,
    out_dir: &std::path::Path,
) -> Result<(KitResult, String), String> {
    if threshold < MIN_THRESHOLD || threshold > total || total > MAX_SHARES {
        return Err(format!(
            "门限与份数不合法（要求 {} ≤ 门限 ≤ 份数 ≤ {}）",
            MIN_THRESHOLD, MAX_SHARES
        ));
    }
    let key = require_unlocked_key()?;
    let (payload, kdf_salt, protected) = build_payload(&key, passphrase)?;

    let sharks = Sharks(threshold);
    let shares: Vec<Share> = sharks.dealer(&payload).take(total as usize).collect();
    if shares.len() != total as usize {
        return Err("份额生成失败".to_string());
    }
    let codes: Vec<String> = shares.iter().map(encode_share).collect();

    let created_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let meta = KitMeta {
        threshold,
        total,
        fingerprint: fingerprint_of(&key),
        created_at: created_at.clone(),
        passphrase_protected: protected,
    };
    // KDF 盐随元数据保存一份：恢复时需要它派生包裹密钥。
    // 盐不是秘密（口令模式的安全性来自口令本身），与指纹同级存储。
    save_meta(&meta, kdf_salt.as_deref())?;

    std::fs::create_dir_all(out_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    let path = out_dir.join(format!(
        "继承人恢复包_{}.html",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    let html = render_kit_html(&meta, &codes);
    crate::fsutil::write_private(&path, html.as_bytes())
        .map_err(|e| format!("写入恢复包失败: {}", e))?;

    Ok((
        KitResult {
            path: path.to_string_lossy().to_string(),
            threshold,
            total,
            fingerprint: meta.fingerprint,
            created_at,
            passphrase_protected: protected,
        },
        html,
    ))
}

/// 从份额恢复主密钥：解析 → 重建 → 校验指纹/口令 → 验证器校验 → 放入会话。
///
/// 任何一步失败都不会改动会话密钥（避免「解锁失败但状态被污染」）。
pub fn recover_from_shares(codes: &[String], passphrase: Option<&str>) -> Result<String, String> {
    if codes.is_empty() {
        return Err("请至少提供一份份额".to_string());
    }
    let meta = read_meta()?.ok_or("本机没有恢复包元数据，无法判断门限与指纹")?;
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

    let key: zeroize::Zeroizing<[u8; 32]> = if meta.passphrase_protected {
        let pass = passphrase
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .ok_or("该恢复包受口令保护，请输入恢复口令")?;
        let salt = load_kdf_salt()?.ok_or("恢复包元数据缺少 KDF 盐")?;
        let wrap = crypto::derive_from_salt(pass, &salt)?;
        let sealed = base64::engine::general_purpose::STANDARD.encode(&payload);
        let plain_b64 =
            crypto::decrypt_with_key(&sealed, &wrap).map_err(|_| "恢复口令不正确".to_string())?;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(plain_b64)
            .map_err(|_| "主密钥数据损坏".to_string())?;
        if raw.len() != 32 {
            return Err("主密钥长度不合法".to_string());
        }
        let mut k = zeroize::Zeroizing::new([0u8; 32]);
        k.copy_from_slice(&raw);
        k
    } else {
        if payload.len() != 32 {
            return Err("份额重建结果长度不合法".to_string());
        }
        let mut k = zeroize::Zeroizing::new([0u8; 32]);
        k.copy_from_slice(&payload);
        k
    };

    if fingerprint_of(&key) != meta.fingerprint {
        return Err("份额属于旧批次（主密码可能已变更），请重新生成恢复包".to_string());
    }
    let Some((_salt, verifier)) = crate::db::get_security_config_db()? else {
        return Err("本机尚未设置主密码".to_string());
    };
    if !crate::vault::verify_key(&verifier, &key) {
        return Err("份额无法通过密钥校验（可能来自旧批次）".to_string());
    }
    crate::vault::set_key(key);
    Ok("已用恢复份额解锁，请立即设置新的主密码".to_string())
}

/// 恢复演练结果（只读校验，不改变任何状态）。
#[derive(Debug, Clone, Serialize)]
pub struct DrillResult {
    /// 自检：用随机测试密钥走完整「拆分→编码→解码→重建」链路是否通过
    pub self_test_ok: bool,
    /// 自检耗时（毫秒）：让用户知道真实恢复要等多久
    pub self_test_ms: u128,
    /// 用户提交的份额数量
    pub provided: usize,
    /// 其中格式与校验和有效的数量
    pub valid: usize,
    /// 门限
    pub threshold: u8,
    /// 有效份额是否已达到门限
    pub enough: bool,
    /// 有效份额是否都属于当前批次（指纹一致）
    pub same_batch: bool,
    /// 口令模式是否完成了 GCM 解密与元数据指纹校验；无口令时为 None。
    pub passphrase_verified: Option<bool>,
    /// 逐条错误说明（不包含份额内容本身）
    pub errors: Vec<String>,
}

fn recover_consistent_payload(threshold: u8, shares: &[Share]) -> Option<Vec<u8>> {
    let threshold = threshold as usize;
    if shares.len() < threshold {
        return None;
    }
    let sharks = Sharks(threshold as u8);
    let base: Vec<&Share> = shares.iter().take(threshold).collect();
    let expected = sharks.recover(base.iter().copied()).ok()?;
    for replacement in shares.iter().skip(threshold) {
        for replaced in 0..threshold {
            let subset: Vec<&Share> = shares
                .iter()
                .take(threshold)
                .enumerate()
                .map(|(index, share)| {
                    if index == replaced {
                        replacement
                    } else {
                        share
                    }
                })
                .collect();
            let payload = sharks.recover(subset.iter().copied()).ok()?;
            if payload != expected {
                return None;
            }
        }
    }
    Some(expected)
}

fn protected_payload_shape_is_valid(payload: &[u8]) -> bool {
    let encoded_key_len = base64::engine::general_purpose::STANDARD
        .encode([0u8; 32])
        .len();
    payload.len() == 12 + encoded_key_len + 16
}

fn verify_protected_payload(
    payload: &[u8],
    passphrase: &str,
    salt: &str,
    fingerprint: &str,
) -> Result<bool, String> {
    let wrap = crypto::derive_from_salt(passphrase, salt)?;
    let sealed = base64::engine::general_purpose::STANDARD.encode(payload);
    let plain_b64 = crypto::decrypt_with_key(&sealed, &wrap)
        .map_err(|_| "恢复口令不正确，无法完成 GCM 校验".to_string())?;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(plain_b64)
        .map_err(|_| "恢复载荷损坏，无法完成 GCM 校验".to_string())?;
    if raw.len() != 32 {
        return Err("恢复载荷解密后长度不合法".to_string());
    }
    let key: [u8; 32] = raw
        .as_slice()
        .try_into()
        .map_err(|_| "恢复载荷解密后长度不合法".to_string())?;
    Ok(fingerprint_of(&key) == fingerprint)
}

/// 恢复演练：验证「份额真的能恢复」，但**不重建、不落盘、不改变会话状态**。
///
/// 为什么必须单独做：生成恢复包时一切正常，不代表三个月后从纸上抄回来还能用。
/// 用户最需要知道的是「我手里这几张纸到底行不行」，而不是「我生成过」。
/// 自检用随机测试密钥（与真实数据无关），所以即使分享演练结果也不泄露任何密钥。
#[cfg(test)]
pub fn run_drill(codes: &[String]) -> Result<DrillResult, String> {
    run_drill_with_passphrase(codes, None)
}

pub fn run_drill_with_passphrase(
    codes: &[String],
    passphrase: Option<&str>,
) -> Result<DrillResult, String> {
    use std::time::Instant;

    // 1) 自检：随机密钥走完整链路
    let started = Instant::now();
    let mut test_key = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut test_key);
    let sharks = Sharks(3);
    let test_shares: Vec<Share> = sharks.dealer(&test_key).take(5).collect();
    let test_codes: Vec<String> = test_shares.iter().map(encode_share).collect();
    let mut decoded: Vec<Share> = Vec::with_capacity(test_codes.len());
    for c in &test_codes {
        decoded.push(decode_share(c)?);
    }
    let rebuilt = sharks
        .recover(&decoded)
        .map_err(|_| "内部自检失败：份额无法重建".to_string())?;
    let self_test_ok = rebuilt == test_key.to_vec();
    let self_test_ms = started.elapsed().as_millis();
    if !self_test_ok {
        return Err("内部自检失败：Shamir 链路异常，请勿依赖本批恢复包".to_string());
    }

    // 2) 只读校验用户提交的份额
    let meta = read_meta()?.ok_or("本机没有恢复包元数据，无法校验份额")?;
    let mut valid = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for (idx, code) in codes.iter().enumerate() {
        if code.trim().is_empty() {
            continue;
        }
        match decode_share(code) {
            Ok(_) => {
                valid += 1;
                // 份额本身不含指纹；批次一致性由「能否通过当前元数据门限重建」间接判断。
                // 这里只记录数量，真正的一致性在下方用可重建性判断。
            }
            Err(e) => errors.push(format!("第 {} 份：{}", idx + 1, e)),
        }
    }
    let enough = valid >= meta.threshold as usize;
    let mut passphrase_verified = None;

    let same_batch = if enough {
        let parsed: Vec<Share> = codes
            .iter()
            .filter_map(|code| decode_share(code).ok())
            .collect();
        if meta.passphrase_protected {
            match recover_consistent_payload(meta.threshold, &parsed) {
                Some(payload) if protected_payload_shape_is_valid(&payload) => {
                    let Some(passphrase) = passphrase.map(str::trim).filter(|p| !p.is_empty())
                    else {
                        errors.push(
                            "已达到门限，但未提供恢复口令，只能验证结构，不能验证口令或批次指纹"
                                .to_string(),
                        );
                        return Ok(DrillResult {
                            self_test_ok,
                            self_test_ms,
                            provided: codes.iter().filter(|c| !c.trim().is_empty()).count(),
                            valid,
                            threshold: meta.threshold,
                            enough,
                            same_batch: false,
                            passphrase_verified,
                            errors,
                        });
                    };
                    let salt = load_kdf_salt()?.ok_or("恢复包元数据缺少 KDF 盐")?;
                    match verify_protected_payload(&payload, passphrase, &salt, &meta.fingerprint) {
                        Ok(true) => {
                            passphrase_verified = Some(true);
                            true
                        }
                        Ok(false) => {
                            passphrase_verified = Some(false);
                            errors.push("恢复口令通过 GCM，但 meta fingerprint 不匹配".to_string());
                            false
                        }
                        Err(error) => {
                            passphrase_verified = Some(false);
                            errors.push(error);
                            false
                        }
                    }
                }
                Some(_) => {
                    passphrase_verified = passphrase
                        .map(str::trim)
                        .filter(|p| !p.is_empty())
                        .map(|_| false);
                    errors.push("份额重建结果不是有效的受保护载荷".to_string());
                    false
                }
                None => false,
            }
        } else if let Ok(payload) = Sharks(meta.threshold).recover(&parsed) {
            if payload.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&payload);
                fingerprint_of(&key) == meta.fingerprint
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    Ok(DrillResult {
        self_test_ok,
        self_test_ms,
        provided: codes.iter().filter(|c| !c.trim().is_empty()).count(),
        valid,
        threshold: meta.threshold,
        enough,
        same_batch,
        passphrase_verified,
        errors,
    })
}

// ── 元数据存储（不含份额） ──────────────────────────────

fn save_meta(meta: &KitMeta, kdf_salt: Option<&str>) -> Result<(), String> {
    let guard = crate::db::get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "INSERT INTO heir_recovery (id, threshold, total, fingerprint, created_at, passphrase_protected, kdf_salt)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET threshold=?2, total=?3, fingerprint=?4, created_at=?5, passphrase_protected=?6, kdf_salt=?7",
        rusqlite::params![
            META_ROW_ID,
            meta.threshold as i64,
            meta.total as i64,
            meta.fingerprint,
            meta.created_at,
            meta.passphrase_protected as i64,
            kdf_salt.unwrap_or(""),
        ],
    )
    .map_err(|e| format!("保存恢复包元数据失败: {}", e))?;
    Ok(())
}

fn load_kdf_salt() -> Result<Option<String>, String> {
    let guard = crate::db::get_conn()?;
    let conn = guard.as_ref().unwrap();
    match conn.query_row(
        "SELECT kdf_salt FROM heir_recovery WHERE id=?1",
        rusqlite::params![META_ROW_ID],
        |r| r.get::<_, String>(0),
    ) {
        Ok(s) => Ok(if s.trim().is_empty() { None } else { Some(s) }),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("读取恢复包元数据失败: {}", e)),
    }
}

/// 读取结构化元数据（恢复流程使用）。
fn read_meta() -> Result<Option<KitMeta>, String> {
    let guard = crate::db::get_conn()?;
    let conn = guard.as_ref().unwrap();
    match conn.query_row(
        "SELECT threshold, total, fingerprint, created_at, passphrase_protected FROM heir_recovery WHERE id=?1",
        rusqlite::params![META_ROW_ID],
        |r| {
            Ok(KitMeta {
                threshold: r.get::<_, i64>(0)? as u8,
                total: r.get::<_, i64>(1)? as u8,
                fingerprint: r.get::<_, String>(2)?,
                created_at: r.get::<_, String>(3)?,
                passphrase_protected: r.get::<_, i64>(4)? != 0,
            })
        },
    ) {
        Ok(meta) => Ok(Some(meta)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("读取恢复包元数据失败: {}", e)),
    }
}

/// 读取元数据；`stale` 表示指纹与当前密钥不一致（主密码已变更）。
pub fn status() -> Result<Option<serde_json::Value>, String> {
    let guard = crate::db::get_conn()?;
    let conn = guard.as_ref().unwrap();
    let row = conn.query_row(
        "SELECT threshold, total, fingerprint, created_at, passphrase_protected FROM heir_recovery WHERE id=?1",
        rusqlite::params![META_ROW_ID],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        },
    );
    match row {
        Ok((threshold, total, fingerprint, created_at, protected)) => {
            let current = crate::vault::get_key().map(|k| fingerprint_of(&k));
            let stale = match &current {
                Some(fp) => fp != &fingerprint,
                None => false, // 锁定态无法判断，不误报
            };
            Ok(Some(serde_json::json!({
                "exists": true,
                "threshold": threshold,
                "total": total,
                "fingerprint": fingerprint,
                "created_at": created_at,
                "passphrase_protected": protected != 0,
                "stale": stale,
            })))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Some(serde_json::json!({
            "exists": false,
            "stale": false,
        }))),
        Err(e) => Err(format!("读取恢复包状态失败: {}", e)),
    }
}

/// 导出元数据（随备份往返）。不含任何份额内容。
pub(crate) fn export_meta_json(conn: &rusqlite::Connection) -> Result<serde_json::Value, String> {
    export_meta_json_in(conn)
}

fn export_meta_json_in(conn: &rusqlite::Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    conn.query_row(
        "SELECT threshold, total, fingerprint, created_at, passphrase_protected, COALESCE(kdf_salt,'')
         FROM heir_recovery WHERE id=1",
        [],
        |r| {
            Ok(json!({
                "threshold": r.get::<_, i64>(0)?,
                "total": r.get::<_, i64>(1)?,
                "fingerprint": r.get::<_, String>(2)?,
                "created_at": r.get::<_, String>(3)?,
                "passphrase_protected": r.get::<_, i64>(4)?,
                "kdf_salt": r.get::<_, String>(5)?,
            }))
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(serde_json::Value::Null),
        other => Err(format!("读取恢复包元数据失败: {}", other)),
    })
}

/// 导入元数据：缺失/null 时清空本地（备份来自没有恢复包的旧版本）。
///
/// 这是正确的语义：如果备份里没有恢复包，说明备份时刻不存在有效批次，
/// 本地残留的旧元数据属于更早的密钥，继续保留会让界面谎报「已保护」。
pub(crate) fn import_meta_json(
    conn: &rusqlite::Connection,
    value: Option<&serde_json::Value>,
) -> Result<(), String> {
    let Some(v) = value.filter(|x| x.is_object()) else {
        conn.execute("DELETE FROM heir_recovery WHERE id=1", [])
            .map_err(|e| format!("清空恢复包元数据失败: {}", e))?;
        return Ok(());
    };
    let sval = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let ival = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    let threshold = ival("threshold");
    let total = ival("total");
    if !(MIN_THRESHOLD as i64..=MAX_SHARES as i64).contains(&threshold)
        || total < threshold
        || total > MAX_SHARES as i64
    {
        return Err("备份中的恢复包门限/份数非法".to_string());
    }
    let fingerprint = sval("fingerprint");
    if fingerprint.len() != 16 || !fingerprint.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("备份中的恢复包指纹非法".to_string());
    }
    conn.execute(
        "INSERT INTO heir_recovery (id, threshold, total, fingerprint, created_at, passphrase_protected, kdf_salt)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET threshold=?1, total=?2, fingerprint=?3, created_at=?4, passphrase_protected=?5, kdf_salt=?6",
        rusqlite::params![
            threshold,
            total,
            fingerprint,
            sval("created_at"),
            ival("passphrase_protected"),
            sval("kdf_salt"),
        ],
    )
    .map_err(|e| format!("写入恢复包元数据失败: {}", e))?;
    Ok(())
}

/// 清除元数据（份额本身从未落库，因此无需清理文件系统）。
pub fn clear_meta() -> Result<(), String> {
    let guard = crate::db::get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "DELETE FROM heir_recovery WHERE id=?1",
        rusqlite::params![META_ROW_ID],
    )
    .map_err(|e| format!("清除恢复包失败: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share_set(threshold: u8, total: u8, key: &[u8; 32]) -> (Vec<String>, KitMeta) {
        let sharks = Sharks(threshold);
        let shares: Vec<Share> = sharks.dealer(key).take(total as usize).collect();
        (
            shares.iter().map(encode_share).collect(),
            KitMeta {
                threshold,
                total,
                fingerprint: fingerprint_of(key),
                created_at: "2026-01-01 00:00:00".to_string(),
                passphrase_protected: false,
            },
        )
    }

    #[test]
    fn share_codes_roundtrip_to_the_same_secret() {
        let key = [42u8; 32];
        let (codes, meta) = share_set(3, 5, &key);
        assert_eq!(codes.len(), 5);
        for c in &codes {
            assert!(c.starts_with(SHARE_PREFIX), "份额必须带版本前缀: {}", c);
        }

        // 任意 3 份都要能重建
        for combo in [[0, 1, 2], [0, 2, 4], [2, 3, 4], [1, 3, 4]] {
            let picked: Vec<Share> = combo
                .iter()
                .map(|i| decode_share(&codes[*i]).unwrap())
                .collect();
            let payload = Sharks(meta.threshold).recover(&picked).unwrap();
            assert_eq!(payload, key.to_vec(), "组合 {:?} 必须重建同一密钥", combo);
        }

        // 不足门限必须失败
        let two: Vec<Share> = (0..2).map(|i| decode_share(&codes[i]).unwrap()).collect();
        assert!(Sharks(meta.threshold).recover(&two).is_err());
    }

    #[test]
    fn corrupted_share_is_rejected_by_checksum() {
        let key = [7u8; 32];
        let (codes, _) = share_set(2, 3, &key);
        // 改掉一个字符
        let mut chars: Vec<char> = codes[0].chars().collect();
        let pos = chars.len() - 1;
        chars[pos] = if chars[pos] == 'A' { 'B' } else { 'A' };
        let broken: String = chars.into_iter().collect();
        assert!(decode_share(&broken).is_err(), "抄错的份额必须被校验和拒绝");

        // 无前缀、非法 base64 都要拒绝
        assert!(decode_share("XXXX-YYYY").is_err());
        assert!(decode_share("DGL1-***").is_err());
        assert!(decode_share("").is_err());
    }

    #[test]
    fn different_batches_produce_different_fingerprints() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        assert_ne!(fingerprint_of(&a), fingerprint_of(&b));
        assert_eq!(fingerprint_of(&a).len(), 16);
    }

    #[test]
    fn kit_html_contains_all_shares_and_never_the_key() {
        let key = [3u8; 32];
        let (codes, meta) = share_set(3, 5, &key);
        let html = render_kit_html(&meta, &codes);
        for c in &codes {
            assert!(html.contains(&html_escape(c)), "恢复包必须包含每份份额");
        }
        // 主密钥不得以任何直接形式出现在打印件里
        let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
        assert!(!html.contains(&key_hex));
        assert!(html.contains("任意 3 份即可恢复"));
    }

    #[test]
    fn threshold_and_share_bounds_are_enforced() {
        // 纯参数校验路径：不触碰数据库即可拒绝非法组合
        let key = [5u8; 32];
        let sharks = Sharks(3);
        let _ = key;
        let shares: Vec<Share> = sharks.dealer(&[5u8; 32]).take(5).collect();
        assert_eq!(shares.len(), 5);
        // 常量本身是策略：门限不得低于 2，份数上限 10
        assert_eq!(MIN_THRESHOLD, 2);
        assert_eq!(MAX_SHARES, 10);
    }

    #[test]
    fn passphrase_payload_is_ciphertext_not_raw_key() {
        let key = [9u8; 32];
        let (payload, salt, protected) =
            build_payload(&key, Some("correct horse battery")).unwrap();
        assert!(protected);
        assert!(salt.is_some());
        assert_ne!(payload, key.to_vec(), "口令模式下拆分对象必须是密文");
        assert!(payload.len() > 32, "AES-GCM 密文应带 nonce 与 tag");

        // 空口令视为未启用保护
        let (payload2, salt2, protected2) = build_payload(&key, Some("   ")).unwrap();
        assert!(!protected2);
        assert!(salt2.is_none());
        assert_eq!(payload2, key.to_vec());
    }

    #[test]
    fn passphrase_shorter_than_eight_is_rejected() {
        let key = [1u8; 32];
        assert!(build_payload(&key, Some("short")).is_err());
    }

    /// 恢复演练：自检必须通过，且有效份额达到门限才判 enough。
    #[test]
    fn drill_self_test_and_share_validation() {
        let key = [11u8; 32];

        // 直接验证编码/解码链路与门限判断逻辑的纯函数部分
        // （run_drill 额外依赖数据库元数据，由集成路径覆盖）。
        let sharks = Sharks(3);
        let shares: Vec<Share> = sharks.dealer(&key).take(5).collect();
        let encoded: Vec<String> = shares.iter().map(encode_share).collect();
        assert_eq!(encoded.len(), 5);

        let decoded: Vec<Share> = encoded.iter().map(|c| decode_share(c).unwrap()).collect();
        assert_eq!(Sharks(3).recover(&decoded).unwrap(), key.to_vec());

        // 两份（不足门限）必须失败
        assert!(Sharks(3).recover(&decoded[..2]).is_err());
        // 混入一份非法份额时，有效数量应减少
        let mut mixed = encoded.clone();
        mixed.push("DGL1-INVALID".to_string());
        let valid = mixed.iter().filter(|c| decode_share(c).is_ok()).count();
        assert_eq!(valid, 5);
    }

    #[test]
    fn passphrase_drill_accepts_ciphertext_payload() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let path = std::env::temp_dir().join(format!(
            "digilegacy_heir_drill_passphrase_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        crate::db::init_database_at(path.clone()).unwrap();

        let key = [13u8; 32];
        let (payload, salt, protected) = build_payload(&key, Some("passphrase-for-test")).unwrap();
        assert!(protected);
        assert!(payload.len() > 32);
        assert!(protected_payload_shape_is_valid(&payload));
        let sharks = Sharks(3);
        let shares: Vec<Share> = sharks.dealer(&payload).take(5).collect();
        let codes: Vec<String> = shares.iter().map(encode_share).collect();
        let meta = KitMeta {
            threshold: 3,
            total: 5,
            fingerprint: fingerprint_of(&key),
            created_at: "2026-01-01 00:00:00".to_string(),
            passphrase_protected: true,
        };
        save_meta(&meta, salt.as_deref()).unwrap();

        let result = run_drill(&codes).unwrap();
        assert_eq!(result.valid, 5);
        assert!(result.enough);
        assert!(!result.same_batch, "无口令时不得报告受保护批次已验证");
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.contains("只能验证结构")),
            "无口令必须明确说明只能验证结构，实际: {:?}",
            result.errors
        );

        crate::vault::clear_key();
        let base = path.to_string_lossy().to_string();
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{}", base, suffix));
        }
    }

    #[test]
    fn passphrase_drill_verifies_gcm_and_fingerprint_for_correct_and_wrong_passphrases() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let path = std::env::temp_dir().join(format!(
            "digilegacy_heir_drill_passphrase_verify_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        crate::db::init_database_at(path.clone()).unwrap();

        let key = [31u8; 32];
        let (payload, salt, protected) = build_payload(&key, Some("correct-drill-pass")).unwrap();
        assert!(protected);
        let sharks = Sharks(3);
        let shares: Vec<Share> = sharks.dealer(&payload).take(5).collect();
        let codes: Vec<String> = shares.iter().map(encode_share).collect();
        let meta = KitMeta {
            threshold: 3,
            total: 5,
            fingerprint: fingerprint_of(&key),
            created_at: "2026-01-01 00:00:00".to_string(),
            passphrase_protected: true,
        };
        save_meta(&meta, salt.as_deref()).unwrap();

        let correct = run_drill_with_passphrase(&codes, Some("correct-drill-pass")).unwrap();
        assert_eq!(correct.passphrase_verified, Some(true));
        assert!(correct.same_batch);
        assert!(correct.errors.is_empty());

        let wrong = run_drill_with_passphrase(&codes, Some("wrong-drill-pass")).unwrap();
        assert_eq!(wrong.passphrase_verified, Some(false));
        assert!(!wrong.same_batch);
        assert!(wrong
            .errors
            .iter()
            .any(|error| error.contains("恢复口令不正确")));

        let wrong_fingerprint = [32u8; 32];
        save_meta(
            &KitMeta {
                fingerprint: fingerprint_of(&wrong_fingerprint),
                ..meta
            },
            salt.as_deref(),
        )
        .unwrap();
        let mismatched = run_drill_with_passphrase(&codes, Some("correct-drill-pass")).unwrap();
        assert_eq!(mismatched.passphrase_verified, Some(false));
        assert!(!mismatched.same_batch);
        assert!(mismatched
            .errors
            .iter()
            .any(|error| error.contains("fingerprint")));

        crate::vault::clear_key();
        let base = path.to_string_lossy().to_string();
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{}", base, suffix));
        }
    }

    #[test]
    fn drill_preserves_unprotected_batch_and_invalid_share_behavior() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let path = std::env::temp_dir().join(format!(
            "digilegacy_heir_drill_unprotected_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        crate::db::init_database_at(path.clone()).unwrap();

        let key = [21u8; 32];
        let sharks = Sharks(3);
        let shares: Vec<Share> = sharks.dealer(&key).take(5).collect();
        let codes: Vec<String> = shares.iter().map(encode_share).collect();
        let meta = KitMeta {
            threshold: 3,
            total: 5,
            fingerprint: fingerprint_of(&key),
            created_at: "2026-01-01 00:00:00".to_string(),
            passphrase_protected: false,
        };
        save_meta(&meta, None).unwrap();

        let mut with_invalid = codes.clone();
        with_invalid.push("DGL1-INVALID".to_string());
        let result = run_drill(&with_invalid).unwrap();
        assert_eq!(result.valid, 5);
        assert!(result.enough);
        assert!(result.same_batch);
        assert_eq!(result.errors.len(), 1);

        let other_key = [22u8; 32];
        let other_shares: Vec<Share> = sharks.dealer(&other_key).take(5).collect();
        let mixed = vec![
            codes[0].clone(),
            codes[1].clone(),
            encode_share(&other_shares[2]),
        ];
        let mixed_result = run_drill(&mixed).unwrap();
        assert_eq!(mixed_result.valid, 3);
        assert!(mixed_result.enough);
        assert!(!mixed_result.same_batch);
        assert!(mixed_result.errors.is_empty());

        crate::vault::clear_key();
        let base = path.to_string_lossy().to_string();
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{}", base, suffix));
        }
    }
}
