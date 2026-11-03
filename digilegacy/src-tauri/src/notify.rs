//! DMS 通知链路：到期检测 → 分级 → 本地发件箱 → (可选) SMTP 投递
//!
//! 离线优先设计：无网络 / 无 SMTP 凭据时写入本地发件箱，不阻塞主流程。
//! 邮件投递使用 lettre（Rust 原生 SMTP 客户端，含 STARTTLS 与 AUTH），
//! 不依赖系统 curl 是否存在。配置方式（界面优先，环境变量兜底）：
//!   DMS_SMTP_HOST / DMS_SMTP_PORT / SMTP_FROM      服务器与发件人
//!   DMS_SMTP_USER / DMS_SMTP_PASS                  SMTP 登录凭据（建议启用）
//!   DMS_SMTP_ALLOW_INSECURE=1                      允许无 STARTTLS 的明文中继（默认拒绝）

use std::fs;
use std::path::Path;

use chrono::{Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::db;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DmsNotification {
    pub stage: i64,
    pub recipient: String,
    pub subject: String,
    pub body: String,
    pub sent_at: String,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DmsStatus {
    pub enabled: bool,
    pub overdue: bool,
    pub current_stage: i64,
    pub days_overdue: i64,
    /// 外出模式暂停截止日（YYYY-MM-DD）；处于暂停期才有值。
    pub paused_until: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DmsEvaluation {
    pub enabled: bool,
    pub overdue: bool,
    pub current_stage: i64,
    pub days_overdue: i64,
    pub notifications: Vec<DmsNotification>,
}

/// 只读状态快照：不发邮件、不写发件箱。
/// 界面展示与后台投递必须分开——旧实现让前端直接调用 `evaluate_dms`，
/// 打开仪表板就等于执行一轮通知投递（可能真的发出 SMTP 邮件）。
pub fn status_snapshot() -> Result<DmsStatus, String> {
    let cfg = read_config()?;
    if !cfg.enabled {
        return Ok(DmsStatus {
            enabled: false,
            overdue: false,
            current_stage: 0,
            days_overdue: 0,
            paused_until: None,
        });
    }
    let (overdue_days, current_stage) = stage_snapshot(&cfg);
    Ok(DmsStatus {
        enabled: true,
        overdue: current_stage > 0,
        current_stage,
        days_overdue: overdue_days,
        paused_until: cfg.paused_until,
    })
}

fn stage_snapshot(cfg: &DmsConfig) -> (i64, i64) {
    // 外出模式：暂停期内一律不判逾期。出差一周回来不该收到「家人已收到通知」。
    if let Some(until) = &cfg.paused_until {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(until, "%Y-%m-%d") {
            if d >= chrono::Local::now().date_naive() {
                return (0, 0);
            }
        }
    }
    match &cfg.last_check_in {
        Some(last) => match days_since_last_check_in(last) {
            Some(days) => {
                let overdue = (days - cfg.interval_days).max(0);
                (overdue, compute_stage(overdue, cfg))
            }
            None => (0, 0),
        },
        None => (0, 0),
    }
}

/// 读取本地发件箱历史（按日期倒序，最多 `limit` 条）。
/// 界面的「通知记录」此前只显示当前这一轮评估的返回值，历史投递完全不可见。
pub fn outbox_history(
    app_handle: &AppHandle,
    limit: usize,
) -> Result<Vec<DmsNotification>, String> {
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {}", e))?
        .join("dms_outbox");
    let mut files: Vec<std::path::PathBuf> = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jsonl"))
            .collect(),
        Err(_) => return Ok(Vec::new()),
    };
    files.sort();
    files.reverse();

    let mut out = Vec::new();
    for file in files {
        if out.len() >= limit {
            break;
        }
        append_history_from(&file, limit, &mut out);
    }
    Ok(out)
}

fn append_history_from(path: &Path, limit: usize, out: &mut Vec<DmsNotification>) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    for line in content.lines().rev() {
        if out.len() >= limit {
            return;
        }
        if let Ok(entry) = serde_json::from_str::<DmsNotification>(line) {
            out.push(entry);
        }
    }
}

/// DMS 配置快照。
///
/// 分级只看两级窗口（stage1 / stage2）；三级没有「下一级」，
/// 超过 stage1+stage2 即进入 stage3 并保持，因此不需要第三段窗口长度。
/// 该字段保留是为了兼容既有数据库与将来的「三级后重发」策略；
/// 界面不暴露它，避免用户以为调它能让提醒更早/更晚触发。
#[derive(Debug, Clone)]
struct DmsConfig {
    enabled: bool,
    interval_days: i64,
    stage1_wait_days: i64,
    stage2_wait_days: i64,
    /// 外出模式：暂停到该日期（YYYY-MM-DD）；None = 未暂停。
    paused_until: Option<String>,
    #[allow(dead_code)]
    last_check_in: Option<String>,
}

fn read_config() -> Result<DmsConfig, String> {
    let guard = db::get_conn()?;
    let conn = guard.as_ref().unwrap();
    let enabled: i64 = conn
        .query_row("SELECT enabled FROM dms_config WHERE id=1", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let interval: i64 = conn
        .query_row(
            "SELECT check_in_interval_days FROM dms_config WHERE id=1",
            [],
            |r| r.get(0),
        )
        .unwrap_or(30);
    let s1: i64 = conn
        .query_row(
            "SELECT stage1_wait_days FROM dms_config WHERE id=1",
            [],
            |r| r.get(0),
        )
        .unwrap_or(3);
    let s2: i64 = conn
        .query_row(
            "SELECT stage2_wait_days FROM dms_config WHERE id=1",
            [],
            |r| r.get(0),
        )
        .unwrap_or(7);
    let last: Option<String> = conn
        .query_row("SELECT last_check_in FROM dms_config WHERE id=1", [], |r| {
            r.get(0)
        })
        .ok();
    let paused_until: Option<String> = conn
        .query_row("SELECT paused_until FROM dms_config WHERE id=1", [], |r| {
            r.get(0)
        })
        .ok()
        .filter(|s: &String| !s.trim().is_empty());
    Ok(DmsConfig {
        enabled: enabled != 0,
        interval_days: interval,
        stage1_wait_days: s1,
        stage2_wait_days: s2,
        paused_until,
        last_check_in: last,
    })
}

fn days_since_last_check_in(last: &str) -> Option<i64> {
    let dt = NaiveDateTime::parse_from_str(last, "%Y-%m-%d %H:%M:%S").ok()?;
    let now = Local::now().naive_local();
    Some(now.signed_duration_since(dt).num_days())
}

fn compute_stage(overdue_days: i64, cfg: &DmsConfig) -> i64 {
    if overdue_days <= 0 {
        0
    } else if overdue_days <= cfg.stage1_wait_days {
        1
    } else if overdue_days <= cfg.stage1_wait_days + cfg.stage2_wait_days {
        2
    } else {
        3
    }
}

fn outbox_path(app_handle: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {}", e))?
        .join("dms_outbox");
    fs::create_dir_all(&dir).map_err(|e| format!("创建发件箱目录失败: {}", e))?;
    Ok(dir.join(format!("{}.jsonl", Local::now().format("%Y-%m-%d"))))
}

fn already_notified(path: &std::path::Path, stage: i64, recipient: &str) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    content.lines().any(|line| {
        line.contains(&format!("\"stage\":{}", stage))
            && line.contains(&format!("\"recipient\":\"{}\"", recipient))
    })
}

fn append_outbox(path: &std::path::Path, entry: &DmsNotification) -> Result<(), String> {
    let line = serde_json::to_string(entry).map_err(|e| format!("序列化通知失败: {}", e))?;
    // 发件箱含收件人邮箱/手机号与通知正文，按「仅本人可读」追加写。
    crate::fsutil::append_private(
        path,
        format!(
            "{}
",
            line
        )
        .as_bytes(),
    )
    .map_err(|e| format!("写入发件箱失败: {}", e))?;
    Ok(())
}

/// 一次 SMTP 投递所需的全部参数。集中成一个结构，是为了让
/// 「界面配置」与「环境变量」两条来源走完全相同的投递路径——
/// 旧实现只认环境变量，普通用户在发布版里根本配不起来。
#[derive(Debug, Clone)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub from: String,
    pub user: Option<String>,
    pub pass: Option<String>,
    pub allow_insecure: bool,
}

/// 解析 SMTP 配置：优先读界面（数据库），没有则回退环境变量。
///
/// 顺序刻意如此：界面配置是用户可见可改的，环境变量是历史兼容路径。
/// 两者都没有时返回 `None`，调用方降级到本地发件箱。
pub fn resolve_smtp_settings() -> Option<SmtpSettings> {
    if let Ok(cfg) = db::get_smtp_config_db() {
        if !cfg.host.trim().is_empty() && !cfg.from_addr.trim().is_empty() {
            let pass = db::get_smtp_password_plain().ok().flatten();
            return Some(SmtpSettings {
                host: cfg.host.trim().to_string(),
                port: if (1..=65535).contains(&cfg.port) {
                    cfg.port as u16
                } else {
                    587
                },
                from: cfg.from_addr.trim().to_string(),
                user: Some(cfg.username).filter(|u| !u.trim().is_empty()),
                pass,
                allow_insecure: cfg.allow_insecure,
            });
        }
    }

    let (Ok(host), Ok(port), Ok(from)) = (
        std::env::var("DMS_SMTP_HOST"),
        std::env::var("DMS_SMTP_PORT"),
        std::env::var("SMTP_FROM"),
    ) else {
        return None;
    };
    if host.trim().is_empty() || from.trim().is_empty() {
        return None;
    }
    Some(SmtpSettings {
        host,
        port: port.parse().unwrap_or(25),
        from,
        user: std::env::var("DMS_SMTP_USER")
            .ok()
            .filter(|v| !v.is_empty()),
        pass: std::env::var("DMS_SMTP_PASS")
            .ok()
            .filter(|v| !v.is_empty()),
        allow_insecure: std::env::var("DMS_SMTP_ALLOW_INSECURE")
            .map(|v| v == "1")
            .unwrap_or(false),
    })
}

/// 通过 lettre 投递（进程内 SMTP 实现，不依赖系统 curl）。
///
/// 与旧 curl 路径的行为对齐：
/// - 默认强制 TLS（STARTTLS 或 SMTPS），仅当用户显式勾选 allow_insecure 才允许明文；
/// - 头部由 lettre 做编码与注入防护（比手写 CRLF 拼接更严格）；
/// - 错误信息保留「主机/端口/阶段」，但绝不回显密码。
fn try_smtp_with(
    settings: &SmtpSettings,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{Message, SmtpTransport, Transport};

    let host = settings.host.trim();
    let from = settings.from.trim();
    if host.is_empty() || from.is_empty() {
        return Err("SMTP 主机或发件人为空".to_string());
    }
    if !is_valid_email(from) {
        return Err("发件人邮箱格式不正确".to_string());
    }
    if !is_valid_email(to) {
        return Err("收件人邮箱格式不正确".to_string());
    }

    let from_mbox = from
        .parse::<lettre::message::Mailbox>()
        .map_err(|e| format!("发件人地址解析失败: {}", e))?;
    let to_mbox = to
        .trim()
        .parse::<lettre::message::Mailbox>()
        .map_err(|e| format!("收件人地址解析失败: {}", e))?;
    let email = Message::builder()
        .from(from_mbox)
        .to(to_mbox)
        .subject(sanitize_header(subject))
        .body(body.to_string())
        .map_err(|e| format!("组装邮件失败: {}", e))?;

    let mut builder = if settings.port == 465 {
        SmtpTransport::relay(host).map_err(|e| format!("初始化 SMTP 失败: {}", e))?
    } else if settings.allow_insecure {
        SmtpTransport::builder_dangerous(host)
    } else {
        SmtpTransport::starttls_relay(host).map_err(|e| format!("初始化 STARTTLS 失败: {}", e))?
    };
    builder = builder
        .port(settings.port)
        .timeout(Some(std::time::Duration::from_secs(20)));

    if let (Some(user), Some(pass)) = (settings.user.as_deref(), settings.pass.as_deref()) {
        if !user.trim().is_empty() && !pass.is_empty() {
            builder = builder.credentials(Credentials::new(user.to_string(), pass.to_string()));
        }
    }

    let mailer = builder.build();
    mailer
        .send(&email)
        .map(|_| ())
        .map_err(|e| format!("邮件投递失败: {}", e))
}

/// 邮件头净化：剥离 CR/LF，阻断 SMTP 头注入（头部拆分可注入 Bcc 等恶意头）
fn sanitize_header(value: &str) -> String {
    value
        .chars()
        .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
        .collect()
}

/// 收件人合法性：仅允许常规 ASCII 邮箱形态。非法值（如未填邮箱时的手机号回退）
/// 不允许进入 SMTP 会话，调用方直接写入本地发件箱
pub(crate) fn is_valid_email_pub(addr: &str) -> bool {
    is_valid_email(addr)
}

fn is_valid_email(addr: &str) -> bool {
    let s = addr.trim();
    if s.is_empty() || s.len() > 254 || s.matches('@').count() != 1 {
        return false;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '-' | '+'))
    {
        return false;
    }
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    if local.is_empty() || local.starts_with('.') || local.ends_with('.') {
        return false;
    }
    if domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return false;
    }
    true
}

/// 发送一封测试邮件：验证「用户填的 SMTP 真的能送达」。
///
/// 与正式通知走同一个 `try_smtp_with`，只换主题和正文——
/// 若这里成功而到期通知失败，说明问题在收件人而非配置。
pub fn send_test_message(settings: &SmtpSettings, recipient: &str) -> Result<(), String> {
    try_smtp_with(
        settings,
        recipient,
        "【归零】SMTP 测试邮件",
        "这是一封来自「归零 DigiLegacy」的测试邮件。\n\n\
如果你收到了它，说明 SMTP 配置可用，到期提醒会通过这条通道发出。\n\
本邮件不含任何账号信息，可安全删除。\n\n\
—— 归零 DigiLegacy",
    )
}

fn dispatch(
    app_handle: &AppHandle,
    stage: i64,
    recipient: &str,
    subject: &str,
    body: &str,
) -> Result<DmsNotification, String> {
    let path = outbox_path(app_handle)?;
    if already_notified(&path, stage, recipient) {
        return Ok(DmsNotification {
            stage,
            recipient: recipient.to_string(),
            subject: subject.to_string(),
            body: body.to_string(),
            sent_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            channel: "dedup".to_string(),
        });
    }

    let channel = if let Some(settings) = resolve_smtp_settings() {
        if !is_valid_email(recipient) {
            // 收件人不是合法邮箱（如未填邮箱时的手机号回退）：
            // 不允许进入 SMTP 会话，直接写入本地发件箱供人工查看
            let _ = append_outbox(
                &path,
                &DmsNotification {
                    stage,
                    recipient: recipient.to_string(),
                    subject: subject.to_string(),
                    body: body.to_string(),
                    sent_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    channel: "local_outbox".to_string(),
                },
            );
            "local_outbox".to_string()
        } else {
            match try_smtp_with(&settings, recipient, subject, body) {
                Ok(()) => "smtp".to_string(),
                Err(_) => {
                    let _ = append_outbox(
                        &path,
                        &DmsNotification {
                            stage,
                            recipient: recipient.to_string(),
                            subject: subject.to_string(),
                            body: body.to_string(),
                            sent_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                            channel: "local_outbox".to_string(),
                        },
                    );
                    "local_outbox".to_string()
                }
            }
        }
    } else {
        let _ = append_outbox(
            &path,
            &DmsNotification {
                stage,
                recipient: recipient.to_string(),
                subject: subject.to_string(),
                body: body.to_string(),
                sent_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                channel: "local_outbox".to_string(),
            },
        );
        "local_outbox".to_string()
    };

    Ok(DmsNotification {
        stage,
        recipient: recipient.to_string(),
        subject: subject.to_string(),
        body: body.to_string(),
        sent_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        channel,
    })
}

/// 执行一轮通知投递并返回本轮结果。**只有后台循环应调用它**：
/// 它有副作用（写发件箱、发 SMTP）。界面读取状态请用 `get_dms_status`。
#[tauri::command]
pub fn evaluate_dms(app_handle: AppHandle) -> Result<DmsEvaluation, String> {
    let cfg = read_config()?;

    if !cfg.enabled {
        return Ok(DmsEvaluation {
            enabled: false,
            overdue: false,
            current_stage: 0,
            days_overdue: 0,
            notifications: vec![],
        });
    }

    let (overdue_days, current_stage) = stage_snapshot(&cfg);

    let overdue = current_stage > 0;
    let user = db::get_user_profile_db()?.unwrap_or_default();
    let mut notifications = Vec::new();

    if overdue {
        let name = if user.full_name.is_empty() {
            "用户".to_string()
        } else {
            user.full_name.clone()
        };
        // 二/三级收件人优先使用邮箱（SMTP 通道只认邮件地址）；未填邮箱时回退手机号，
        // 该值也会原样写入本地发件箱供人工查看，仅 SMTP 投递会失败并降级。
        let stage2_recipient = if user.emergency_contact_email.is_empty() {
            user.emergency_contact_phone.clone()
        } else {
            user.emergency_contact_email.clone()
        };
        // 第三级优先发给「执行人」：多继承人时只有执行人负责统筹，
        // 发给某一位继承人不合适（其他人会以为与自己无关）。
        // 名单为空时回退旧的 heir_primary_*，保证既有数据继续可用。
        let heirs = db::get_heirs_db().unwrap_or_default();
        let executor = heirs.iter().find(|h| h.is_executor);
        let (stage3_recipient, stage3_name) = match executor {
            Some(h) => (
                if h.email.trim().is_empty() {
                    h.phone.clone()
                } else {
                    h.email.clone()
                },
                h.name.clone(),
            ),
            None => (
                if user.heir_primary_email.is_empty() {
                    user.heir_primary_phone.clone()
                } else {
                    user.heir_primary_email.clone()
                },
                user.heir_primary_name.clone(),
            ),
        };

        if current_stage >= 1 {
            if let Ok(n) = dispatch(
                &app_handle,
                1,
                &user.email,
                &format!("【归零】{}，请确认你的安全状态", name),
                &format!(
                    "你好 {}，\n\n你已超过 {} 天未确认安全状态。请打开「归零」应用点击「我还安好」。\n若 {} 天后仍未确认，系统将通知你的紧急联系人。\n\n—— 归零 DigiLegacy",
                    name, overdue_days, cfg.stage1_wait_days
                ),
            ) {
                notifications.push(n);
            }
        }

        if current_stage >= 2 {
            if let Ok(n) = dispatch(
                &app_handle,
                2,
                &stage2_recipient,
                &format!("【归零】请确认 {} 的安全状态", name),
                &format!(
                    "你好，\n\n{} 已超过 {} 天未确认安全状态。请尝试联系本人确认是否安好。\n若确认失联，请联系遗产执行人。\n\n—— 归零 DigiLegacy",
                    name,
                    overdue_days
                ),
            ) {
                notifications.push(n);
            }
        }

        if current_stage >= 3 {
            if let Ok(n) = dispatch(
                &app_handle,
                3,
                &stage3_recipient,
                &format!("【归零】{} 的数字遗产继承指引", name),
                &format!(
                    "你好 {}，\n\n{} 已长时间未确认安全状态。\n请备齐死亡证明与继承公证材料（或法院判决书），\n在「归零」应用的继承人向导页面按平台逐一办理继承手续。\n本邮件不含任何账号凭据；账号信息请通过继承人本机的加密备份取得。\n\n—— 归零 DigiLegacy",
                    stage3_name, name
                ),
            ) {
                notifications.push(n);
            }
        }
    }

    Ok(DmsEvaluation {
        enabled: true,
        overdue,
        current_stage,
        days_overdue: overdue_days,
        notifications,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `_s3` 保留在签名里：既有测试按三级窗口传参，
    /// 但分级只用前两级（三级之后不再有下一级）。
    fn test_config(s1: i64, s2: i64, _s3: i64) -> DmsConfig {
        DmsConfig {
            enabled: true,
            interval_days: 30,
            stage1_wait_days: s1,
            stage2_wait_days: s2,
            paused_until: None,
            last_check_in: None,
        }
    }

    #[test]
    fn stage_0_when_not_overdue() {
        let cfg = test_config(3, 7, 14);
        assert_eq!(compute_stage(0, &cfg), 0);
        assert_eq!(compute_stage(-5, &cfg), 0);
    }

    #[test]
    fn stage_1_within_first_window() {
        let cfg = test_config(3, 7, 14);
        assert_eq!(compute_stage(1, &cfg), 1);
        assert_eq!(compute_stage(3, &cfg), 1);
    }

    #[test]
    fn stage_2_within_second_window() {
        let cfg = test_config(3, 7, 14);
        assert_eq!(compute_stage(4, &cfg), 2);
        assert_eq!(compute_stage(10, &cfg), 2);
    }

    #[test]
    fn stage_3_beyond_all_windows() {
        let cfg = test_config(3, 7, 14);
        assert_eq!(compute_stage(11, &cfg), 3);
        assert_eq!(compute_stage(100, &cfg), 3);
    }

    #[test]
    fn days_since_last_check_in_parses_correctly() {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        assert_eq!(days_since_last_check_in(&now), Some(0));

        let past = (Local::now() - chrono::Duration::days(10))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        assert_eq!(days_since_last_check_in(&past), Some(10));
    }

    #[test]
    fn days_since_rejects_invalid_format() {
        assert_eq!(days_since_last_check_in("not-a-date"), None);
        assert_eq!(days_since_last_check_in(""), None);
    }

    /// 收件人校验：合法邮箱放行，手机号/注入形态/残缺地址一律拒绝
    #[test]
    fn smtp_recipient_validator() {
        assert!(is_valid_email("a@b.com"));
        assert!(is_valid_email("user.name+tag@sub.domain.cn"));
        assert!(is_valid_email("a_b-c@d.io"));

        assert!(!is_valid_email(""), "空值必须拒绝");
        assert!(!is_valid_email("13800000000"), "手机号回退必须拒绝");
        assert!(!is_valid_email("a@b"), "缺域名点号必须拒绝");
        assert!(!is_valid_email("a@b."), "域名以点结尾必须拒绝");
        assert!(!is_valid_email("a@b@c.com"), "双 @ 必须拒绝");
        assert!(!is_valid_email("a b@x.com"), "含空格必须拒绝");
        assert!(
            !is_valid_email("a@x.com\r\nBcc: e@x.com"),
            "含 CRLF 必须拒绝"
        );
        assert!(!is_valid_email(".a@x.com"), "本地部以点开头必须拒绝");
    }

    /// 安全默认：未显式允许明文时，服务器不提供 STARTTLS 必须拒绝投递。
    ///
    /// 这是 TLS 降级攻击的防线：如果实现「连不上就退明文」，
    /// 主动降级的中间人就能收到明文邮件。这里起一个不广告 STARTTLS 的
    /// 最小 SMTP mock，断言投递失败。
    #[test]
    fn try_smtp_refuses_without_starttls() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 本地端口失败");
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(
                    b"220 mock ESMTP

",
                );
                let _ = stream.flush();
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf); // EHLO
                let _ = stream.write_all(
                    b"250-mock

250 SIZE 10485760

",
                );
                let _ = stream.flush();
                // 不广告 STARTTLS：客户端必须中止
            }
        });

        let settings = SmtpSettings {
            host: "127.0.0.1".to_string(),
            port,
            from: "from@test.local".to_string(),
            user: None,
            pass: None,
            allow_insecure: false,
        };
        let result = try_smtp_with(&settings, "to@test.local", "s", "b");
        assert!(result.is_err(), "无 STARTTLS 能力时必须拒绝投递");
        let _ = handle.join();
    }

    /// 去重逻辑：同 stage + 同收件人只投递一次。
    ///
    /// 这条没有测试时，把 `stage` 改成 `stage + 1` 全部 65 个测试仍绿——
    /// 真实后果：去重永远查下一级记录，同一 stage 每 30 分钟重复投递，
    /// 收件人被反复轰炸。
    #[test]
    fn already_notified_dedups_same_stage_and_recipient() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("digilegacy_outbox_{}.jsonl", std::process::id()));
        let _ = fs::remove_file(&path);

        let entry = DmsNotification {
            stage: 2,
            recipient: "heir@example.com".to_string(),
            subject: "s".to_string(),
            body: "b".to_string(),
            sent_at: "2026-01-01 00:00:00".to_string(),
            channel: "local_outbox".to_string(),
        };
        append_outbox(&path, &entry).unwrap();

        assert!(
            already_notified(&path, 2, "heir@example.com"),
            "同 stage 同收件人必须判为已通知"
        );
        assert!(
            !already_notified(&path, 1, "heir@example.com"),
            "不同 stage 不得误判"
        );
        assert!(
            !already_notified(&path, 3, "heir@example.com"),
            "更高 stage 不得误判"
        );
        assert!(
            !already_notified(&path, 2, "other@example.com"),
            "不同收件人不得误判"
        );

        // 不存在的文件：视为未通知，允许首次投递
        let missing = dir.join("digilegacy_outbox_missing.jsonl");
        let _ = fs::remove_file(&missing);
        assert!(!already_notified(&missing, 2, "heir@example.com"));

        let _ = fs::remove_file(&path);
    }

    /// DMS 启用后必须有「起始锚点」：从未签到过时以启用时刻为准计算逾期，
    /// 而不是永远 waiting。旧实现 last_check_in 为 NULL 时 stage 恒为 0，
    /// 用户以为开了保险，实际永远不会触发任何提醒。
    #[test]
    fn enabled_dms_anchors_on_first_enable_instead_of_never_firing() {
        let _g = crate::vault::test_lock();
        let path =
            std::env::temp_dir().join(format!("digilegacy_dms_anchor_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        crate::db::init_database_at(path.clone()).unwrap();

        crate::db::set_dms_config_db(true, 7).unwrap();
        let status = status_snapshot().unwrap();
        assert!(status.enabled);
        assert_eq!(status.current_stage, 0, "刚启用不应立刻逾期");

        // 启用时刻必须被记录为锚点（否则永远不会逾期）
        let anchor: Option<String> = {
            let guard = crate::db::get_conn().unwrap();
            guard
                .as_ref()
                .unwrap()
                .query_row("SELECT last_check_in FROM dms_config WHERE id=1", [], |r| {
                    r.get(0)
                })
                .ok()
        };
        assert!(
            anchor.is_some(),
            "启用 DMS 时必须写入起始锚点，否则永远不逾期"
        );

        // 把锚点推到 8 天前（间隔 7 天），应进入 stage 1
        let past = (chrono::Local::now() - chrono::Duration::days(8))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        {
            let guard = crate::db::get_conn().unwrap();
            guard
                .as_ref()
                .unwrap()
                .execute(
                    "UPDATE dms_config SET last_check_in=?1 WHERE id=1",
                    rusqlite::params![past],
                )
                .unwrap();
        }
        let overdue = status_snapshot().unwrap();
        assert!(overdue.overdue, "超过间隔必须逾期");
        assert_eq!(overdue.current_stage, 1);
        assert_eq!(overdue.days_overdue, 1);

        let _ = std::fs::remove_file(&path);
    }

    /// 外出暂停：即使逾期很久，暂停期内分级必须归零——
    /// 出差回来不该面对已经发给家人的「告别邮件」。
    #[test]
    fn paused_config_suppresses_stage() {
        let future = (Local::now().date_naive() + chrono::Duration::days(7))
            .format("%Y-%m-%d")
            .to_string();
        let mut cfg = test_config(3, 7, 14);
        cfg.paused_until = Some(future);
        cfg.last_check_in = Some(
            (Local::now() - chrono::Duration::days(60))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        );
        let (days, stage) = stage_snapshot(&cfg);
        assert_eq!((days, stage), (0, 0), "暂停期内不得判逾期");

        // 暂停到期后自动恢复：过期日期不挂起评估。
        let past = (Local::now().date_naive() - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        cfg.paused_until = Some(past);
        let (days, stage) = stage_snapshot(&cfg);
        assert!(days > 0 && stage >= 1, "过期暂停必须自动失效");
    }

    /// 只读状态快照必须与投递共用同一套分级，但绝不产生副作用。
    #[test]
    fn stage_snapshot_matches_compute_stage() {
        let cfg = DmsConfig {
            enabled: true,
            interval_days: 30,
            stage1_wait_days: 3,
            stage2_wait_days: 7,
            paused_until: None,
            last_check_in: Some(
                (Local::now() - chrono::Duration::days(60))
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            ),
        };
        let (overdue, stage) = stage_snapshot(&cfg);
        assert_eq!(overdue, 30);
        assert_eq!(stage, 3);
        assert_eq!(stage, compute_stage(overdue, &cfg));
    }

    /// 发件箱历史：按日期倒序聚合，坏行跳过而不是整体失败。
    #[test]
    fn outbox_history_reads_newest_first_and_skips_bad_lines() {
        let dir = std::env::temp_dir().join(format!("digilegacy_hist_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let old = dir.join("2026-01-01.jsonl");
        let new = dir.join("2026-01-02.jsonl");
        append_outbox(
            &old,
            &DmsNotification {
                stage: 1,
                recipient: "a@x.com".to_string(),
                subject: "old".to_string(),
                body: "b".to_string(),
                sent_at: "2026-01-01 00:00:00".to_string(),
                channel: "local_outbox".to_string(),
            },
        )
        .unwrap();
        append_outbox(
            &new,
            &DmsNotification {
                stage: 3,
                recipient: "b@x.com".to_string(),
                subject: "new".to_string(),
                body: "b".to_string(),
                sent_at: "2026-01-02 00:00:00".to_string(),
                channel: "smtp".to_string(),
            },
        )
        .unwrap();
        fs::write(new.with_file_name("2026-01-03.jsonl"), "{not json}\n").unwrap();

        let mut out = Vec::new();
        for file in [&new, &old] {
            append_history_from(file, 10, &mut out);
        }
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].stage, 3, "最新的文件先读");

        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod stage_boundary_tests {
    use super::*;

    fn cfg() -> DmsConfig {
        DmsConfig {
            enabled: true,
            interval_days: 30,
            stage1_wait_days: 3,
            stage2_wait_days: 7,
            paused_until: None,
            last_check_in: None,
        }
    }

    /// 分级只看前两级窗口：stage3 没有「下一级」，超过 s1+s2 即保持三级。
    /// 这条测试锁住「stage3_wait_days 不是必需配置」这一事实，防止有人再加回死字段。
    #[test]
    fn stage_boundaries_use_only_first_two_windows() {
        let c = cfg();
        assert_eq!(compute_stage(0, &c), 0, "未逾期必须是 0");
        assert_eq!(compute_stage(-5, &c), 0, "提前不算逾期");
        assert_eq!(compute_stage(1, &c), 1);
        assert_eq!(compute_stage(3, &c), 1, "第 3 天仍是第 1 级");
        assert_eq!(compute_stage(4, &c), 2, "超过 s1 进入第 2 级");
        assert_eq!(compute_stage(10, &c), 2, "第 2 级上界 = s1+s2 = 10");
        assert_eq!(compute_stage(11, &c), 3);
        // 关键：再久也停在 3，不会出现第 4 级
        assert_eq!(compute_stage(9999, &c), 3, "三级是终态");
    }
}
