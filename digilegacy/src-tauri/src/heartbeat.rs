//! 云端心跳：仅上报"我还安好"令牌，不携带任何用户数据（零内容设计）
//!
//! 服务器只需实现一个接口：
//!   POST {base}/heartbeat   body: {"token": "..."}   → 200
//! 服务器职责：记录该 token 最后心跳时间；到期未收到时由服务端定时任务
//! 按"token → 紧急联系人"的预注册映射发通知（映射也只含联系方式，无资产数据）。
//! 本地失联容忍：请求失败只记录状态，绝不阻塞或 panic。

use serde::Serialize;

const TIMEOUT_SECS: u64 = 8;

#[derive(Debug, Clone, Serialize)]
pub struct HeartbeatStatus {
    pub enabled: bool,
    pub endpoint: String,
    pub last_attempt: Option<String>,
    pub last_result: String, // "ok" | "failed" | "never"
}

#[derive(Default)]
pub(crate) struct HeartbeatConfig {
    enabled: bool,
    token: String,
    endpoint: String,
}

pub(crate) fn validate_heartbeat_endpoint(endpoint: &str) -> Result<String, String> {
    let ep = endpoint.trim();
    if ep.is_empty() || ep.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("心跳地址为空或包含非法字符".to_string());
    }
    let lower = ep.to_ascii_lowercase();
    let (https, rest) = if let Some(rest) = lower.strip_prefix("https://") {
        (true, rest)
    } else {
        return Err("心跳地址必须使用 https://".to_string());
    };
    if !https || rest.is_empty() {
        return Err("心跳地址缺少主机名".to_string());
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return Err("心跳地址主机名无效".to_string());
    }
    let host = if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or_else(|| "心跳地址 IPv6 主机名无效".to_string())?;
        if end + 1 < authority.len() && !authority[end + 1..].starts_with(':') {
            return Err("心跳地址端口无效".to_string());
        }
        &authority[1..end]
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    if host.is_empty() || authority.matches(':').count() > 1 {
        return Err("心跳地址主机名无效".to_string());
    }
    if let Some(port) = authority
        .rsplit_once(':')
        .and_then(|(_, p)| (!p.is_empty()).then_some(p))
    {
        if authority.starts_with('[') && !authority.contains("]:") {
            return Err("心跳地址端口无效".to_string());
        }
        if port.parse::<u16>().is_err() || port == "0" {
            return Err("心跳地址端口无效".to_string());
        }
    }
    let host_lower = host.trim_end_matches('.').to_ascii_lowercase();
    let restricted_name = host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower == "metadata"
        || host_lower == "metadata.google.internal"
        || host_lower == "instance-data";
    let restricted_ip = host_lower
        .parse::<std::net::IpAddr>()
        .map(|ip| match ip {
            std::net::IpAddr::V4(v4) => {
                let o = v4.octets();
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.is_broadcast()
                    || (o[0] == 100 && o[1] == 64)
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_unique_local()
                    || v6.is_unicast_link_local()
                    || v6.to_ipv4().is_some_and(|v4| {
                        v4.is_loopback()
                            || v4.is_private()
                            || v4.is_link_local()
                            || v4.is_unspecified()
                    })
            }
        })
        .unwrap_or(false);
    if restricted_name || restricted_ip {
        return Err("心跳地址必须是公网主机，不得指向本机、内网或云元数据地址".to_string());
    }
    if !host_lower.contains('.') {
        return Err("心跳地址必须使用公网主机名".to_string());
    }
    Ok(ep.to_string())
}

/// 读取心跳配置：本地库为权威来源（用户在界面里可配），
/// 环境变量仅作后备（便于自托管部署与自动化场景）。
/// 旧实现只认环境变量，普通用户装完应用后无法开启该功能。
fn read_config() -> HeartbeatConfig {
    let from_db =
        crate::db::get_heartbeat_config_db().unwrap_or((false, String::new(), String::new()));
    read_config_with_db(from_db)
}

fn read_config_with_db(
    (db_enabled, db_token, db_endpoint): (bool, String, String),
) -> HeartbeatConfig {
    let enabled = if db_enabled {
        true
    } else {
        std::env::var("DMS_HEARTBEAT_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    };
    let raw_endpoint = if !db_endpoint.is_empty() {
        db_endpoint
    } else {
        std::env::var("DMS_HEARTBEAT_ENDPOINT").unwrap_or_default()
    };
    let endpoint = validate_heartbeat_endpoint(&raw_endpoint).unwrap_or_default();
    HeartbeatConfig {
        enabled,
        token: db_token,
        endpoint,
    }
}

/// 发送心跳请求。
///
/// 改用进程内 HTTP 客户端（ureq + rustls）而不是调用系统 curl：
/// - 不依赖用户机器上是否装有 curl（Windows 10 1803 之前的镜像没有）；
/// - 不把令牌写进子进程环境/标准输入，减少暴露面；
/// - 超时、TLS 校验、重定向次数都由代码控制，不依赖外部命令版本差异。
fn post_heartbeat(endpoint: &str, token: &str) -> Result<(), String> {
    if token.is_empty() {
        return Err("心跳令牌为空".to_string());
    }
    if endpoint.trim().is_empty() {
        return Err("未配置心跳服务器地址".to_string());
    }
    let ep = validate_heartbeat_endpoint(endpoint)?;
    let url = format!("{}/heartbeat", ep.trim_end_matches('/'));
    let payload = serde_json::json!({ "token": token });

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(TIMEOUT_SECS)))
        // 不信 3xx 跳转：心跳端点是用户自配的，跳转到任意主机等于把令牌外送
        .max_redirects(0)
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();

    let response = agent
        .post(&url)
        .header("Content-Type", "application/json")
        .send_json(&payload)
        .map_err(|e| format!("心跳请求失败: {}", e))?;

    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("心跳服务器返回 HTTP {}", status));
    }
    Ok(())
}

static LAST_STATUS: once_cell::sync::Lazy<std::sync::Mutex<HeartbeatStatus>> =
    once_cell::sync::Lazy::new(|| {
        std::sync::Mutex::new(HeartbeatStatus {
            enabled: false,
            endpoint: String::new(),
            last_attempt: None,
            last_result: "never".to_string(),
        })
    });

fn record_attempt(endpoint: &str, enabled: bool, ok: bool) {
    let mut guard = LAST_STATUS.lock().unwrap();
    guard.enabled = enabled;
    guard.endpoint = endpoint.to_string();
    guard.last_attempt = Some(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    guard.last_result = if ok {
        "ok".to_string()
    } else {
        "failed".to_string()
    };
}

/// 发送一次心跳（后台线程与手动测试共用）；永不 panic
pub fn send_heartbeat() -> HeartbeatStatus {
    let cfg = read_config();
    send_heartbeat_with(&cfg)
}

/// 可注入配置的心跳发送（便于测试，避免依赖进程级环境变量）
pub fn send_heartbeat_with(cfg: &HeartbeatConfig) -> HeartbeatStatus {
    // 端点为空表示尚未配置服务器：这不是「发送失败」，而是「没有可发往的目标」。
    if !cfg.enabled || cfg.token.is_empty() || cfg.endpoint.trim().is_empty() {
        let status = HeartbeatStatus {
            enabled: cfg.enabled && !cfg.token.is_empty() && !cfg.endpoint.trim().is_empty(),
            endpoint: cfg.endpoint.clone(),
            last_attempt: None,
            last_result: "never".to_string(),
        };
        return status;
    }
    let ok = post_heartbeat(&cfg.endpoint, &cfg.token).is_ok();
    record_attempt(&cfg.endpoint, true, ok);
    let guard = LAST_STATUS.lock().unwrap();
    HeartbeatStatus {
        enabled: guard.enabled,
        endpoint: guard.endpoint.clone(),
        last_attempt: guard.last_attempt.clone(),
        last_result: guard.last_result.clone(),
    }
}

pub fn current_status() -> HeartbeatStatus {
    LAST_STATUS.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试串行锁：本模块的断言依赖全局 LAST_STATUS，避免与其他用例交叉污染。
    ///
    /// 必须复用 `vault::test_lock()` 而不是另起一把锁：`heartbeat_config_roundtrip_in_db`
    /// 会调用 `db::init_database_at` 改写全局 DB_PATH/CONN，而 db 模块的用例都持这把锁。
    /// 两把锁各管一半全局状态时会并行竞争（旧实现的隐患）。
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        crate::vault::test_lock()
    }

    #[test]
    fn disabled_or_empty_token_never_sends() {
        let _g = lock();
        let cfg = HeartbeatConfig {
            enabled: false,
            token: String::new(),
            endpoint: String::new(),
        };
        let status = send_heartbeat_with(&cfg);
        assert_eq!(status.last_result, "never");
        assert!(!status.enabled);
    }

    #[test]
    fn enabled_but_empty_token_never_sends() {
        let _g = lock();
        let cfg = HeartbeatConfig {
            enabled: true,
            token: String::new(),
            endpoint: String::new(),
        };
        assert_eq!(send_heartbeat_with(&cfg).last_result, "never");
    }

    /// 端点未配置时必须报 never 而不是 failed：
    /// 旧实现里默认端点是 example.com 保留域名，任何启用都会「永远失败」，
    /// 用户看到的是坏掉的功能而不是「尚未配置」。
    #[test]
    fn enabled_without_endpoint_reports_never_not_failed() {
        let _g = lock();
        let cfg = HeartbeatConfig {
            enabled: true,
            token: "tok".to_string(),
            endpoint: String::new(),
        };
        let status = send_heartbeat_with(&cfg);
        assert_eq!(status.last_result, "never");
        assert!(!status.enabled, "未配置端点不构成可用的心跳");
    }

    #[test]
    fn env_endpoint_fallback_uses_public_https_policy_without_network() {
        let _g = lock();
        let old_enabled = std::env::var("DMS_HEARTBEAT_ENABLED").ok();
        let old_endpoint = std::env::var("DMS_HEARTBEAT_ENDPOINT").ok();
        std::env::set_var("DMS_HEARTBEAT_ENABLED", "true");
        std::env::set_var("DMS_HEARTBEAT_ENDPOINT", "http://example.com");
        let cfg = read_config_with_db((false, String::new(), String::new()));
        assert!(cfg.enabled);
        assert!(
            cfg.endpoint.is_empty(),
            "env endpoint 必须复用公网 HTTPS policy"
        );
        if let Some(value) = old_enabled {
            std::env::set_var("DMS_HEARTBEAT_ENABLED", value);
        } else {
            std::env::remove_var("DMS_HEARTBEAT_ENABLED");
        }
        if let Some(value) = old_endpoint {
            std::env::set_var("DMS_HEARTBEAT_ENDPOINT", value);
        } else {
            std::env::remove_var("DMS_HEARTBEAT_ENDPOINT");
        }
    }

    #[test]
    fn post_heartbeat_rejects_blank_endpoint() {
        assert!(post_heartbeat("", "tok").is_err());
        assert!(post_heartbeat("   ", "tok").is_err());
    }

    /// 非 http(s) 协议必须在发出请求前拒绝：环境变量可绕过命令层校验。
    #[test]
    fn post_heartbeat_rejects_non_http_schemes() {
        for bad in ["file:///etc/passwd", "ftp://example.com", "gopher://x"] {
            let err = post_heartbeat(bad, "tok").unwrap_err();
            assert!(err.contains("http"), "{} 应被拒绝，实际: {}", bad, err);
        }
    }

    /// 配置持久化：写入后能原样读回，且 upsert 不会产生多行。
    #[test]
    fn endpoint_policy_rejects_non_public_targets_and_remote_http() {
        for bad in [
            "http://example.com",
            "https://127.0.0.1",
            "https://10.0.0.1",
            "https://192.168.1.1",
            "https://169.254.169.254",
            "https://metadata.google.internal",
            "https://localhost",
        ] {
            assert!(
                validate_heartbeat_endpoint(bad).is_err(),
                "must reject {}",
                bad
            );
        }
        assert!(validate_heartbeat_endpoint("https://example.com/heartbeat").is_ok());
    }

    #[test]
    fn vault_heartbeat_token_is_ciphertext_and_backup_never_contains_plaintext() {
        let _g = lock();
        crate::vault::clear_key();
        let path =
            std::env::temp_dir().join(format!("digilegacy_hb_secret_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        crate::db::init_database_at(path.clone()).unwrap();
        let salt = crate::crypto::generate_salt_v3();
        let kek = crate::crypto::derive_from_salt("hb-secret-pw", &salt).unwrap();
        let dek = crate::vault::generate_data_key();
        let wrapped = crate::vault::wrap_data_key(&dek, &kek).unwrap();
        let verifier = crate::vault::make_verifier(&dek).unwrap();
        crate::db::set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
        crate::vault::set_key(dek);

        crate::db::set_heartbeat_config_db(true, "heartbeat-secret", "https://example.com")
            .unwrap();
        let raw: String = {
            let guard = crate::db::get_conn().unwrap();
            guard
                .as_ref()
                .unwrap()
                .query_row("SELECT token FROM heartbeat_config WHERE id=1", [], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        assert!(raw.starts_with(crate::vault::ENC_PREFIX_V2));
        assert!(!raw.contains("heartbeat-secret"));
        assert_eq!(
            crate::db::get_heartbeat_config_db().unwrap().1,
            "heartbeat-secret"
        );
        let backup = crate::db::export_backup_json_db().unwrap();
        assert!(!backup.contains("heartbeat-secret"));

        crate::vault::clear_key();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn heartbeat_config_roundtrip_in_db() {
        let _g = lock();
        let mut path = std::env::temp_dir();
        path.push(format!("digilegacy_hb_cfg_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        crate::db::init_database_at(path.clone()).unwrap();

        assert_eq!(
            crate::db::get_heartbeat_config_db().unwrap(),
            (false, String::new(), String::new())
        );

        let salt = crate::crypto::generate_salt_v3();
        let kek = crate::crypto::derive_from_salt("heartbeat-roundtrip-pw", &salt).unwrap();
        let dek = crate::vault::generate_data_key();
        let wrapped = crate::vault::wrap_data_key(&dek, &kek).unwrap();
        let verifier = crate::vault::make_verifier(&dek).unwrap();
        crate::db::set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
        crate::vault::set_key(dek);

        crate::db::set_heartbeat_config_db(true, "tok-1", "https://hb.example.org").unwrap();
        assert_eq!(
            crate::db::get_heartbeat_config_db().unwrap(),
            (
                true,
                "tok-1".to_string(),
                "https://hb.example.org".to_string()
            )
        );

        crate::db::set_heartbeat_config_db(false, "tok-2", "").unwrap();
        assert_eq!(
            crate::db::get_heartbeat_config_db().unwrap(),
            (false, "tok-2".to_string(), String::new())
        );
        crate::vault::clear_key();
    }

    #[test]
    fn private_endpoint_is_rejected_without_network() {
        let _g = lock();
        let cfg = HeartbeatConfig {
            enabled: true,
            token: "test-token-123".to_string(),
            endpoint: "https://127.0.0.1:9".to_string(),
        };
        let status = send_heartbeat_with(&cfg);
        assert_eq!(
            status.last_result, "failed",
            "私网端点必须在发出请求前被拒绝"
        );
    }

    #[test]
    fn status_defaults() {
        let s = current_status();
        assert!(matches!(s.last_result.as_str(), "never" | "ok" | "failed"));
    }

    #[test]
    fn local_http_endpoint_is_rejected_without_network() {
        let _g = lock();
        let cfg = HeartbeatConfig {
            enabled: true,
            token: "local-test-token".to_string(),
            endpoint: "http://127.0.0.1:1".to_string(),
        };
        let status = send_heartbeat_with(&cfg);
        assert_eq!(status.last_result, "failed");
    }
}
