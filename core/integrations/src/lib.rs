//! Integration ports and real health probes (INT-01..07, M5-B).
//!
//! The platform stores only metadata and health. This crate performs bounded
//! network probes; it never persists credentials or returns them.

#![forbid(unsafe_code)]

use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use serde::Serialize;

pub const MODULE_FAMILY: &str = "INT";
pub const STATUS: &str = "m5-health";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKind {
    Llm,
    Policy,
    Smtp,
    Imap,
    Heartbeat,
    Webhook,
    Feishu,
    Wecom,
}

impl ConnectionKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "llm" => Some(Self::Llm),
            "policy" => Some(Self::Policy),
            "smtp" => Some(Self::Smtp),
            "imap" => Some(Self::Imap),
            "heartbeat" => Some(Self::Heartbeat),
            "webhook" => Some(Self::Webhook),
            "feishu" => Some(Self::Feishu),
            "wecom" => Some(Self::Wecom),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Policy => "policy",
            Self::Smtp => "smtp",
            Self::Imap => "imap",
            Self::Heartbeat => "heartbeat",
            Self::Webhook => "webhook",
            Self::Feishu => "feishu",
            Self::Wecom => "wecom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Unconfigured,
    Ok,
    Degraded,
    Error,
    Disabled,
}

impl ConnectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Ok => "ok",
            Self::Degraded => "degraded",
            Self::Error => "error",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub ok: bool,
    pub status: String,
    pub latency_ms: u64,
    pub message: String,
}

fn result(ok: bool, status: &str, started: Instant, message: String) -> ProbeResult {
    ProbeResult {
        ok,
        status: status.to_string(),
        latency_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        message,
    }
}

fn client(timeout: Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("DigiLegacy/0.9 connection-probe")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

/// Run the blocking reqwest client on a plain OS thread. Dropping its internal
/// runtime from a Tokio blocking worker can panic during shutdown, so keep the
/// client lifecycle entirely outside the async runtime.
fn http_probe(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
    timeout: Duration,
) -> ProbeResult {
    let method = method.to_string();
    let url = url.to_string();
    let token = token.map(str::to_string);
    let worker = std::thread::spawn(move || {
        http_probe_inner(&method, &url, token.as_deref(), body, timeout)
    });
    match worker.join() {
        Ok(result) => result,
        Err(_) => result(
            false,
            "error",
            Instant::now(),
            "HTTP 探针线程失败".to_string(),
        ),
    }
}

fn http_probe_inner(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
    timeout: Duration,
) -> ProbeResult {
    let started = Instant::now();
    let client = match client(timeout) {
        Ok(client) => client,
        Err(message) => return result(false, "error", started, message),
    };
    let request = match method {
        "POST" => client.post(url),
        _ => client.get(url),
    };
    let request = if let Some(token) = token.filter(|value| !value.is_empty()) {
        request.bearer_auth(token)
    } else {
        request
    };
    let request = if let Some(body) = body {
        request.json(&body)
    } else {
        request
    };
    match request.send() {
        Ok(response) => {
            let code = response.status();
            let message = format!("HTTP {}", code.as_u16());
            if code.is_success() {
                result(true, "ok", started, message)
            } else if code.is_client_error() {
                result(false, "degraded", started, message)
            } else {
                result(false, "error", started, message)
            }
        }
        Err(error) if error.is_timeout() => result(false, "error", started, "连接超时".to_string()),
        Err(error) if error.is_connect() => {
            result(false, "error", started, "无法连接目标地址".to_string())
        }
        // Never surface the raw URL: it may contain a webhook token.
        Err(_) => result(false, "error", started, "请求失败".to_string()),
    }
}

fn tcp_endpoint(value: &str) -> &str {
    let trimmed = value.trim();
    for prefix in ["tcp://", "smtp://", "smtps://", "imap://", "imaps://"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.trim_end_matches('/');
        }
    }
    trimmed
}

fn tcp_probe(endpoint: &str, timeout: Duration) -> ProbeResult {
    let started = Instant::now();
    let address = tcp_endpoint(endpoint);
    let addrs = match address.to_socket_addrs() {
        Ok(addrs) => addrs.collect::<Vec<_>>(),
        Err(error) => return result(false, "error", started, format!("地址解析失败: {error}")),
    };
    if addrs.is_empty() {
        return result(false, "error", started, "地址为空".to_string());
    }
    let mut last_error = "连接失败".to_string();
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(_) => return result(true, "ok", started, format!("TCP 已连接 {addr}")),
            Err(error) => last_error = error.to_string(),
        }
    }
    result(false, "error", started, last_error)
}

/// Probe one connection with a bounded timeout. `endpoint` must already be
/// resolved by the caller; no secrets are accepted or returned.
pub fn probe(kind: &str, endpoint: &str, token: Option<&str>) -> ProbeResult {
    let started = Instant::now();
    let Some(kind) = ConnectionKind::parse(kind) else {
        return result(false, "error", started, format!("未知连接类型: {kind}"));
    };
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return result(false, "unconfigured", started, "未配置连接地址".to_string());
    }
    let timeout = Duration::from_secs(5);
    match kind {
        ConnectionKind::Smtp | ConnectionKind::Imap => tcp_probe(endpoint, timeout),
        ConnectionKind::Llm => {
            let url = if endpoint.ends_with("/v1") {
                format!("{endpoint}/models")
            } else if endpoint.ends_with("/models") {
                endpoint.to_string()
            } else {
                format!("{}/models", endpoint.trim_end_matches('/'))
            };
            http_probe("GET", &url, token, None, timeout)
        }
        ConnectionKind::Policy | ConnectionKind::Heartbeat => {
            http_probe("GET", endpoint, token, None, timeout)
        }
        ConnectionKind::Webhook => http_probe(
            "POST",
            endpoint,
            token,
            Some(serde_json::json!({ "event": "digilegacy.connection.test" })),
            timeout,
        ),
        ConnectionKind::Feishu => http_probe(
            "POST",
            endpoint,
            token,
            Some(serde_json::json!({
                "msg_type": "text",
                "content": { "text": "DigiLegacy 连接测试" }
            })),
            timeout,
        ),
        ConnectionKind::Wecom => http_probe(
            "POST",
            endpoint,
            token,
            Some(serde_json::json!({
                "msgtype": "text",
                "text": { "content": "DigiLegacy 连接测试" }
            })),
            timeout,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn unknown_kind_is_a_structured_error() {
        let result = probe("nope", "http://127.0.0.1", None);
        assert!(!result.ok);
        assert_eq!(result.status, "error");
        assert!(result.message.contains("未知连接类型"));
    }

    #[test]
    fn tcp_probe_reaches_a_local_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let _ = listener.accept();
        });
        let result = probe("smtp", &addr.to_string(), None);
        assert!(result.ok);
        assert_eq!(result.status, "ok");
        worker.join().unwrap();
    }

    #[test]
    fn empty_endpoint_is_unconfigured() {
        let result = probe("webhook", " ", None);
        assert!(!result.ok);
        assert_eq!(result.status, "unconfigured");
    }
}
