//! IMAP 入站签到：回一封邮件 = 「我还安好」。
//!
//! 为什么需要：DMS 的签到入口原本只有「打开应用点按钮」。对老年用户这是
//! 一个高频遗忘点——签到链路空转等于没有保护。邮件是所有人都会用的入口：
//! 系统发出 check-in 邮件，用户随手回复（或转发回原地址），后台轮询收件箱，
//! 发现来自受信地址的回信就调 `record_check_in_db`。
//!
//! 安全边界（写进代码）：
//! 1. **默认关闭**。必须用户在界面配置 IMAP 主机/账号/授权码才启用。
//! 2. **凭据走 vault 加密**（复用 SMTP 配置的加密列模式），不明文落库。
//! 3. **只读收件箱**：只做 SELECT + SEARCH + FETCH，绝不删除/移动邮件。
//! 4. **白名单发件人**：只认 `allowed_senders` 里的地址（大小写不敏感），
//!    其余邮件一律忽略——不能让任意广告邮件触发「我还安好」。
//! 5. **去重**：同一条邮件 (uid + date) 只结算一次，记录在 `imap_seen` 表。
//! 6. 拉取窗口固定最近 N 天（默认 14），避免全箱扫描。

use rusqlite::params;

use crate::db;

/// 单次轮询结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImapCheckResult {
    pub enabled: bool,
    pub connected: bool,
    /// 本次结算为「我还安好」的邮件数
    pub checkins: usize,
    /// 检查过的邮件数（窗口内）
    pub scanned: usize,
    pub message: String,
}

/// IMAP 配置（界面可配；密码只落密文不回传）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ImapConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub has_password: bool,
    /// 允许触发签到的发件人（分号分隔；空 = 不启用白名单外的任何地址）
    pub allowed_senders: String,
    pub lookback_days: i64,
}

const CONFIG_ROW: i64 = 1;

/// 读取配置（不回传密码）。
pub fn get_config() -> Result<ImapConfig, String> {
    let guard = db::get_conn()?;
    let conn = guard.as_ref().unwrap();
    match conn.query_row(
        "SELECT enabled, COALESCE(host,''), COALESCE(port,993), COALESCE(username,''), COALESCE(password_enc,''), COALESCE(allowed_senders,''), COALESCE(lookback_days,14)
         FROM imap_config WHERE id=?1",
        params![CONFIG_ROW],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
            ))
        },
    ) {
        Ok((enabled, host, port, username, pass_enc, senders, lookback)) => Ok(ImapConfig {
            enabled: enabled != 0,
            host,
            port: port as u16,
            username,
            has_password: !pass_enc.is_empty(),
            allowed_senders: senders,
            lookback_days: lookback.clamp(1, 90),
        }),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(ImapConfig {
            lookback_days: 14,
            ..Default::default()
        }),
        Err(e) => Err(format!("读取 IMAP 配置失败: {}", e)),
    }
}

/// 保存配置；密码为 None 表示保留已存的。
pub fn set_config(cfg: &ImapConfig, password: Option<&str>) -> Result<(), String> {
    db::ensure_vault_writable_db()?;
    let host = cfg.host.trim();
    if cfg.enabled && (host.is_empty() || cfg.username.trim().is_empty()) {
        return Err("启用 IMAP 签到必须填写主机与账号".to_string());
    }
    if !(1..=65535).contains(&cfg.port) {
        return Err("端口不合法".to_string());
    }
    if cfg.lookback_days < 1 || cfg.lookback_days > 90 {
        return Err("回看窗口必须在 1..90 天".to_string());
    }
    // 白名单校验：每个地址必须是合法邮箱
    for s in cfg
        .allowed_senders
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !crate::notify::is_valid_email_pub(s) {
            return Err(format!("白名单地址不合法: {}", s));
        }
    }

    let pass_enc = match password {
        Some(p) if !p.is_empty() => crate::vault::encrypt_if_needed(p)?,
        _ => {
            let guard = db::get_conn()?;
            let conn = guard.as_ref().unwrap();
            conn.query_row(
                "SELECT COALESCE(password_enc,'') FROM imap_config WHERE id=?1",
                params![CONFIG_ROW],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_default()
        }
    };
    if cfg.enabled && pass_enc.is_empty() {
        return Err("启用 IMAP 签到必须填写授权码/密码".to_string());
    }

    let guard = db::get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "INSERT INTO imap_config (id, enabled, host, port, username, password_enc, allowed_senders, lookback_days, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now','localtime'))
         ON CONFLICT(id) DO UPDATE SET enabled=?2, host=?3, port=?4, username=?5, password_enc=?6, allowed_senders=?7, lookback_days=?8, updated_at=datetime('now','localtime')",
        params![
            CONFIG_ROW,
            cfg.enabled as i64,
            host,
            cfg.port as i64,
            cfg.username.trim(),
            pass_enc,
            cfg.allowed_senders.trim(),
            cfg.lookback_days,
        ],
    )
    .map_err(|e| format!("保存 IMAP 配置失败: {}", e))?;
    Ok(())
}

/// 是否已有同 uid 的记录（去重）。
fn already_seen(conn: &rusqlite::Connection, uid: u32) -> Result<bool, String> {
    let seen: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM imap_seen WHERE uid=?1",
            params![uid as i64],
            |r| r.get(0),
        )
        .map_err(|e| format!("查询去重记录失败: {}", e))?;
    Ok(seen > 0)
}

fn mark_seen(conn: &rusqlite::Connection, uid: u32, from_addr: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO imap_seen (uid, sender, seen_at) VALUES (?1, ?2, datetime('now','localtime'))",
        params![uid as i64, from_addr],
    )
    .map_err(|e| format!("记录去重失败: {}", e))?;
    Ok(())
}

fn record_check_in_then_mark_seen<F, G>(
    mut record_check_in: F,
    mut mark_seen: G,
) -> Result<(), String>
where
    F: FnMut() -> Result<(), String>,
    G: FnMut() -> Result<(), String>,
{
    record_check_in()?;
    mark_seen()
}

/// 白名单判断：大小写不敏感、允许 `Name <addr>` 形态。
fn sender_allowed(from_header: &str, allowed: &str) -> bool {
    let lower = from_header.to_lowercase();
    let addr_part = lower.rsplit('<').next().unwrap_or(&lower);
    let addr = addr_part.trim_end_matches('>').trim();
    allowed
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .any(|a| a.to_lowercase() == addr)
}

/// 从原始邮件里抽取 From 头（简化解析：找行首 From: 到行尾，足够处理白名单匹配）。
fn extract_from_header(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    for line in text.lines() {
        if line.len() > 5 && line[..5].eq_ignore_ascii_case("from:") {
            return Some(line[5..].trim().to_string());
        }
        // 头部结束（空行）后不再找
        if line.trim().is_empty() {
            return None;
        }
    }
    None
}

/// 轮询一次收件箱；非启用状态直接返回。
///
/// 失败只记录信息不 panic：IMAP 服务器偶发不可达不应让后台线程崩溃。
pub fn poll_once() -> ImapCheckResult {
    match poll_inner() {
        Ok(r) => r,
        Err(e) => ImapCheckResult {
            enabled: true,
            connected: false,
            checkins: 0,
            scanned: 0,
            message: e,
        },
    }
}

fn poll_inner() -> Result<ImapCheckResult, String> {
    let cfg = get_config()?;
    if !cfg.enabled {
        return Ok(ImapCheckResult {
            enabled: false,
            connected: false,
            checkins: 0,
            scanned: 0,
            message: "IMAP 签到未启用".to_string(),
        });
    }
    let password = {
        let guard = db::get_conn()?;
        let conn = guard.as_ref().unwrap();
        let enc: String = conn
            .query_row(
                "SELECT COALESCE(password_enc,'') FROM imap_config WHERE id=?1",
                params![CONFIG_ROW],
                |r| r.get(0),
            )
            .unwrap_or_default();
        if enc.is_empty() {
            return Ok(ImapCheckResult {
                enabled: true,
                connected: false,
                checkins: 0,
                scanned: 0,
                message: "未配置授权码".to_string(),
            });
        }
        crate::vault::decrypt_if_needed(&enc)?
    };

    let connector = native_tls::TlsConnector::builder()
        .build()
        .map_err(|e| format!("初始化 TLS 失败: {}", e))?;
    let host = cfg.host.trim();
    let stream = std::net::TcpStream::connect((host, cfg.port))
        .map_err(|e| format!("连接 IMAP 失败: {}", e))?;
    let stream = connector
        .connect(host, stream)
        .map_err(|e| format!("TLS 握手失败: {}", e))?;
    let email = imap::Client::new(stream);
    let mut session = email
        .login(cfg.username.trim(), &password)
        .map_err(|e| format!("IMAP 登录失败: {}", e.0))?;

    let poll_result = poll_inbox(&mut session, &cfg);
    let _ = session.logout();
    poll_result
}

fn poll_inbox(
    session: &mut imap::Session<native_tls::TlsStream<std::net::TcpStream>>,
    cfg: &ImapConfig,
) -> Result<ImapCheckResult, String> {
    session
        .select("INBOX")
        .map_err(|e| format!("打开收件箱失败: {}", e))?;

    let since = chrono::Local::now() - chrono::Duration::days(cfg.lookback_days);
    let since_str = since.format("%d-%b-%Y").to_string();
    let uids = session
        .uid_search(format!("SINCE {}", since_str))
        .map_err(|e| format!("搜索邮件失败: {}", e))?;
    let mut sorted: Vec<u32> = uids.into_iter().collect();
    sorted.sort_unstable();

    let mut result = ImapCheckResult {
        enabled: true,
        connected: true,
        checkins: 0,
        scanned: 0,
        message: String::new(),
    };

    // 分批取（每批 50 封），只取头 + 文本
    for chunk in sorted.chunks(50) {
        if chunk.is_empty() {
            continue;
        }
        let set = chunk
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let fetches = session
            .uid_fetch(set, "(BODY.PEEK[HEADER] UID)")
            .map_err(|e| format!("读取邮件失败: {}", e))?;
        for f in fetches.iter() {
            result.scanned += 1;
            if f.uid.is_none() {
                continue;
            }
            let seen = {
                let guard = db::get_conn()?;
                let conn = guard.as_ref().unwrap();
                already_seen(conn, f.uid.unwrap())?
            };
            if seen {
                continue;
            }
            let uid = match f.uid {
                Some(u) => u,
                None => continue,
            };
            let Some(header) = f.header() else { continue };
            let Some(from) = extract_from_header(header) else {
                continue;
            };
            if !sender_allowed(&from, &cfg.allowed_senders) {
                continue;
            }
            // 白名单命中 → 记一次签到
            record_check_in_then_mark_seen(db::record_check_in_internal_db, || {
                let guard = db::get_conn()?;
                let conn = guard.as_ref().unwrap();
                mark_seen(conn, uid, &from)
            })?;
            result.checkins += 1;
        }
    }
    if result.checkins > 0 {
        result.message = format!("已按 {} 封回信更新「我还安好」状态", result.checkins);
    } else {
        result.message = "收件箱无新回信".to_string();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_checkin_does_not_mark_uid_and_retries_next_poll() {
        use std::cell::{Cell, RefCell};
        use std::collections::HashSet;

        let uid = 42u32;
        let attempts = Cell::new(0);
        let events = RefCell::new(Vec::new());
        let seen = RefCell::new(HashSet::new());
        let first = record_check_in_then_mark_seen(
            || {
                events.borrow_mut().push("record");
                attempts.set(attempts.get() + 1);
                Err("database unavailable".to_string())
            },
            || {
                events.borrow_mut().push("mark");
                seen.borrow_mut().insert(uid);
                Ok(())
            },
        );
        assert!(first.is_err());
        assert_eq!(*events.borrow(), vec!["record"]);
        assert!(!seen.borrow().contains(&uid));

        events.borrow_mut().clear();
        let second = record_check_in_then_mark_seen(
            || {
                events.borrow_mut().push("record");
                assert_eq!(attempts.get(), 1);
                attempts.set(attempts.get() + 1);
                Ok(())
            },
            || {
                events.borrow_mut().push("mark");
                seen.borrow_mut().insert(uid);
                Ok(())
            },
        );
        assert!(second.is_ok());
        assert_eq!(*events.borrow(), vec!["record", "mark"]);
        assert!(seen.borrow().contains(&uid));
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn sender_allows_exact_and_display_name_form() {
        assert!(sender_allowed("heir@example.com", "heir@example.com"));
        assert!(sender_allowed("HEIR@EXAMPLE.COM", "heir@example.com"));
        assert!(sender_allowed(
            "李四 <heir@example.com>",
            "heir@example.com"
        ));
        assert!(sender_allowed("a@x.com", "b@y.com;a@x.com;c@z.com"));
        assert!(!sender_allowed("stranger@evil.com", "heir@example.com"));
        assert!(!sender_allowed(
            "heir@example.com.evil.com",
            "heir@example.com"
        ));
        assert!(!sender_allowed("", "heir@example.com"));
        // 空白名单 = 不允许任何人（防广告邮件触发）
        assert!(!sender_allowed("heir@example.com", ""));
    }

    #[test]
    fn from_header_extraction_stops_at_blank_line() {
        assert_eq!(
            extract_from_header(b"From: a@b.com\r\nTo: c@d.com\r\n\r\nbody"),
            Some("a@b.com".to_string())
        );
        assert_eq!(
            extract_from_header(b"To: c@d.com\r\nFrom: x@y.com\r\n\r\n"),
            Some("x@y.com".to_string())
        );
        // From 出现在正文（空行之后）不算
        assert_eq!(
            extract_from_header(b"Subject: hi\r\n\r\nFrom: fake@evil.com"),
            None
        );
        assert_eq!(extract_from_header(b""), None);
    }

    #[test]
    fn config_roundtrip_and_validation() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let path = std::env::temp_dir().join(format!("digilegacy_imap_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        crate::db::init_database_at(path.clone()).unwrap();

        // 未配置 → 默认关闭
        let cfg = get_config().unwrap();
        assert!(!cfg.enabled);

        // 启用必须带完整字段
        let bad = ImapConfig {
            enabled: true,
            host: "".into(),
            port: 993,
            username: "u".into(),
            has_password: false,
            allowed_senders: "a@b.com".into(),
            lookback_days: 14,
        };
        assert!(
            set_config(&bad, Some("pw123456")).is_err(),
            "空主机必须拒绝"
        );

        // 非法白名单地址必须拒绝
        let bad2 = ImapConfig {
            enabled: false,
            host: "imap.example.com".into(),
            port: 993,
            username: "u".into(),
            has_password: false,
            allowed_senders: "not-an-email".into(),
            lookback_days: 14,
        };
        assert!(set_config(&bad2, None).is_err());

        // 合法配置 → 回读一致（密码不回传）
        let good = ImapConfig {
            enabled: false,
            host: "imap.example.com".into(),
            port: 993,
            username: "me@example.com".into(),
            has_password: false,
            allowed_senders: "heir@example.com;son@example.com".into(),
            lookback_days: 21,
        };
        set_config(&good, Some("auth-code-1")).unwrap();
        let back = get_config().unwrap();
        assert_eq!(back.host, "imap.example.com");
        assert_eq!(back.lookback_days, 21);
        assert!(back.has_password);
        assert!(!back.enabled);
        // 更新配置但不传密码 → 保留已存的
        set_config(&good, None).unwrap();
        assert!(get_config().unwrap().has_password);

        let _ = std::fs::remove_file(&path);
    }
}
