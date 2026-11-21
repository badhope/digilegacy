//! Integration ports and real health probes (INT-01..07, M5-B).
//!
//! The platform stores only metadata and health. This crate performs bounded
//! network probes; it never persists credentials or returns them.

#![forbid(unsafe_code)]

use digilegacy_security::{resolve_external_url, resolve_tcp_endpoint, resolve_tcp_host};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use rustls::pki_types::CertificateDer;
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
    Mcp,
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
            "mcp" => Some(Self::Mcp),
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
            Self::Mcp => "mcp",
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

fn client(
    timeout: Duration,
    resolved: &digilegacy_security::ResolvedExternalUrl,
) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .resolve_to_addrs(&resolved.host, &resolved.addrs)
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

fn mcp_payload(text: &str) -> Option<serde_json::Value> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return Some(value);
    }
    let mut data = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim_start());
        }
    }
    serde_json::from_str(&data).ok()
}

fn mcp_probe(endpoint: &str, token: Option<&str>, timeout: Duration) -> ProbeResult {
    let endpoint = endpoint.to_string();
    let token = token.map(str::to_string);
    let worker = std::thread::spawn(move || mcp_probe_inner(&endpoint, token.as_deref(), timeout));
    match worker.join() {
        Ok(result) => result,
        Err(_) => result(
            false,
            "error",
            Instant::now(),
            "MCP 探针线程失败".to_string(),
        ),
    }
}

fn mcp_probe_inner(endpoint: &str, token: Option<&str>, timeout: Duration) -> ProbeResult {
    let started = Instant::now();
    let resolved = match resolve_external_url(endpoint) {
        Ok(resolved) => resolved,
        Err(message) => return result(false, "error", started, message),
    };
    let client = match client(timeout, &resolved) {
        Ok(client) => client,
        Err(message) => return result(false, "error", started, message),
    };

    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "DigiLegacy", "version": env!("CARGO_PKG_VERSION") }
        }
    });
    let mut request = client
        .post(&resolved.url)
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .json(&init);
    if let Some(token) = token.filter(|value| !value.is_empty()) {
        request = request.bearer_auth(token);
    }
    let response = match request.send() {
        Ok(response) => response,
        Err(error) => return result(false, "error", started, format!("MCP 初始化失败: {error}")),
    };
    let code = response.status();
    let session = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response.text().unwrap_or_default();
    if !code.is_success() {
        let status = if code.is_client_error() {
            "degraded"
        } else {
            "error"
        };
        return result(
            false,
            status,
            started,
            format!("MCP 初始化 HTTP {}", code.as_u16()),
        );
    }
    let Some(payload) = mcp_payload(&body) else {
        return result(
            false,
            "degraded",
            started,
            "MCP 初始化响应不是 JSON-RPC".to_string(),
        );
    };
    if payload.get("error").is_some() {
        return result(
            false,
            "error",
            started,
            "MCP 初始化返回 JSON-RPC 错误".to_string(),
        );
    }

    let mut notify = client
        .post(&resolved.url)
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    if let Some(token) = token.filter(|value| !value.is_empty()) {
        notify = notify.bearer_auth(token);
    }
    if let Some(session) = session.as_deref() {
        notify = notify.header("mcp-session-id", session);
    }
    let _ = notify.send();

    let mut list = client
        .post(&resolved.url)
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .json(
            &serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
        );
    if let Some(token) = token.filter(|value| !value.is_empty()) {
        list = list.bearer_auth(token);
    }
    if let Some(session) = session.as_deref() {
        list = list.header("mcp-session-id", session);
    }
    let tools = match list.send() {
        Ok(response) if response.status().is_success() => response
            .text()
            .ok()
            .and_then(|body| mcp_payload(&body))
            .and_then(|payload| payload.pointer("/result/tools").cloned())
            .and_then(|tools| tools.as_array().map(|items| items.len()))
            .unwrap_or(0),
        _ => 0,
    };
    result(true, "ok", started, format!("MCP 已连接 · {tools} 个工具"))
}

fn http_probe_inner(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
    timeout: Duration,
) -> ProbeResult {
    let started = Instant::now();
    let resolved = match resolve_external_url(url) {
        Ok(resolved) => resolved,
        Err(message) => return result(false, "error", started, message),
    };
    let client = match client(timeout, &resolved) {
        Ok(client) => client,
        Err(message) => return result(false, "error", started, message),
    };
    let request = match method {
        "POST" => client.post(&resolved.url),
        _ => client.get(&resolved.url),
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
    let addrs = match resolve_tcp_endpoint(address) {
        Ok(addrs) => addrs,
        Err(error) => return result(false, "error", started, error),
    };
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
fn llm_models_url(endpoint: &str) -> String {
    let endpoint = endpoint.trim();
    if endpoint.ends_with("/v1") {
        format!("{endpoint}/models")
    } else if endpoint.ends_with("/models") {
        endpoint.to_string()
    } else {
        format!("{}/models", endpoint.trim_end_matches('/'))
    }
}

/// List model ids exposed by an OpenAI-compatible endpoint. This is a
/// read-only discovery call; it returns an empty list on any probe failure so
/// callers can keep the connection usable with a manually typed model name.
pub fn list_llm_models(endpoint: &str, token: Option<&str>) -> Vec<String> {
    let url = llm_models_url(endpoint);
    let token = token.map(str::to_string);
    let worker = std::thread::spawn(move || {
        let resolved = match resolve_external_url(&url) {
            Ok(resolved) => resolved,
            Err(_) => return Vec::new(),
        };
        let client = match client(Duration::from_secs(8), &resolved) {
            Ok(client) => client,
            Err(_) => return Vec::new(),
        };
        let mut request = client.get(&resolved.url);
        if let Some(token) = token.as_deref().filter(|value| !value.is_empty()) {
            request = request.bearer_auth(token);
        }
        let response = match request.send() {
            Ok(response) if response.status().is_success() => response,
            _ => return Vec::new(),
        };
        let body: serde_json::Value = match response.json() {
            Ok(value) => value,
            Err(_) => return Vec::new(),
        };
        let mut ids = body
            .get("data")
            .and_then(|value| value.as_array())
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row.get("id").and_then(|id| id.as_str()))
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        ids.sort();
        ids.dedup();
        ids
    });
    worker.join().unwrap_or_default()
}

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
        ConnectionKind::Llm => http_probe("GET", &llm_models_url(endpoint), token, None, timeout),
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
        ConnectionKind::Mcp => mcp_probe(endpoint, token, Duration::from_secs(8)),
    }
}

/// SMTP delivery target. Credentials are only used in-memory for one send.
#[derive(Debug, Clone)]
pub struct SmtpTarget {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub starttls: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from: String,
}

impl SmtpTarget {
    pub fn parse(
        endpoint: &str,
        security: Option<&str>,
        username: Option<String>,
        password: Option<String>,
        from: Option<String>,
    ) -> Result<Self, String> {
        let raw = endpoint.trim();
        if raw.is_empty()
            || raw.chars().any(char::is_control)
            || raw.chars().any(char::is_whitespace)
        {
            return Err("SMTP 地址不合法".to_string());
        }
        let (scheme, rest) = match raw.find("://") {
            Some(index) => (raw[..index].to_ascii_lowercase(), &raw[index + 3..]),
            None => ("".to_string(), raw),
        };
        if !matches!(scheme.as_str(), "" | "smtp" | "smtps") {
            return Err("SMTP 仅支持 smtp:// 或 smtps://".to_string());
        }
        let (host, port) = split_host_port(
            rest,
            if scheme == "smtps" {
                465
            } else if scheme == "smtp" {
                587
            } else {
                25
            },
        )?;
        if host.is_empty() {
            return Err("SMTP 地址缺少主机".to_string());
        }
        let security = security
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if scheme == "smtps" || port == 465 {
                    "tls".to_string()
                } else if port == 25 {
                    "none".to_string()
                } else {
                    "starttls".to_string()
                }
            });
        let (tls, starttls) = match security.as_str() {
            "tls" => (true, false),
            "starttls" => (false, true),
            "none" => (false, false),
            _ => return Err("security 必须是 tls/starttls/none".to_string()),
        };
        let from = from
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                username
                    .as_deref()
                    .filter(|value| value.contains('@'))
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "digilegacy@localhost".to_string());
        if from.chars().any(char::is_control) {
            return Err("发件人地址不合法".to_string());
        }
        Ok(Self {
            host,
            port,
            tls,
            starttls,
            username,
            password,
            from,
        })
    }
}

fn split_host_port(value: &str, default_port: u16) -> Result<(String, u16), String> {
    if let Some(rest) = value.strip_prefix('[') {
        let (host, tail) = rest
            .split_once(']')
            .ok_or_else(|| "SMTP IPv6 地址缺少 ]".to_string())?;
        let port = tail
            .strip_prefix(':')
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(default_port);
        return Ok((host.to_string(), port));
    }
    if let Some((host, port)) = value.rsplit_once(':') {
        if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return Ok((
                host.to_string(),
                port.parse::<u16>()
                    .map_err(|_| "SMTP 端口不合法".to_string())?,
            ));
        }
    }
    Ok((value.to_string(), default_port))
}

fn build_tls_config(
    roots: rustls::RootCertStore,
) -> Result<std::sync::Arc<rustls::ClientConfig>, String> {
    let provider = rustls::crypto::ring::default_provider();
    let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|error| format!("初始化 TLS 失败: {error}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(std::sync::Arc::new(config))
}

fn tls_client_config(
    extra_roots: &[CertificateDer<'static>],
) -> Result<std::sync::Arc<rustls::ClientConfig>, String> {
    if !extra_roots.is_empty() {
        let mut roots = rustls::RootCertStore::empty();
        roots.add_parsable_certificates(extra_roots.iter().cloned());
        return build_tls_config(roots);
    }
    static CONFIG: std::sync::OnceLock<std::sync::Arc<rustls::ClientConfig>> =
        std::sync::OnceLock::new();
    if let Some(config) = CONFIG.get() {
        return Ok(config.clone());
    }
    let loaded = rustls_native_certs::load_native_certs();
    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(loaded.certs);
    if roots.is_empty() {
        return Err("没有可用的系统 CA 证书".to_string());
    }
    let config = build_tls_config(roots)?;
    let _ = CONFIG.set(config.clone());
    Ok(config)
}

fn tls_stream(
    tcp: std::net::TcpStream,
    host: &str,
    extra_roots: &[CertificateDer<'static>],
) -> Result<rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream>, String> {
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| "SMTP 主机名不合法".to_string())?;
    let connection = rustls::ClientConnection::new(tls_client_config(extra_roots)?, server_name)
        .map_err(|error| format!("建立 TLS 连接失败: {error}"))?;
    Ok(rustls::StreamOwned::new(connection, tcp))
}

trait ReadWrite: std::io::Read + std::io::Write {}
impl<T: std::io::Read + std::io::Write> ReadWrite for T {}

fn read_line(stream: &mut dyn ReadWrite) -> Result<String, String> {
    let mut bytes = Vec::new();
    for _ in 0..8192 {
        let mut byte = [0u8; 1];
        match stream.read(&mut byte) {
            Ok(0) => return Err("SMTP 连接提前关闭".to_string()),
            Ok(_) if byte[0] == b'\n' => {
                if bytes.last() == Some(&b'\r') {
                    bytes.pop();
                }
                return Ok(String::from_utf8_lossy(&bytes).into_owned());
            }
            Ok(_) => bytes.push(byte[0]),
            Err(error) => return Err(format!("读取 SMTP 响应失败: {error}")),
        }
    }
    Err("SMTP 响应过长".to_string())
}

fn read_response(stream: &mut dyn ReadWrite) -> Result<(u16, String), String> {
    let mut text = String::new();
    for _ in 0..100 {
        let line = read_line(stream)?;
        if line.len() < 3 {
            return Err("SMTP 响应格式错误".to_string());
        }
        let code = line[..3]
            .parse::<u16>()
            .map_err(|_| "SMTP 响应码不合法".to_string())?;
        if !text.is_empty() {
            text.push('\n');
        }
        if line.len() > 3 {
            text.push_str(line[3..].trim_start_matches([' ', '-']));
        }
        if line.as_bytes().get(3) != Some(&b'-') {
            return Ok((code, text));
        }
    }
    Err("SMTP 响应行过多".to_string())
}

fn expect(stream: &mut dyn ReadWrite, codes: &[u16]) -> Result<String, String> {
    let (code, text) = read_response(stream)?;
    if codes.contains(&code) {
        Ok(text)
    } else {
        Err(format!(
            "SMTP {code}: {}",
            text.chars().take(300).collect::<String>()
        ))
    }
}

fn smtp_command(
    stream: &mut dyn ReadWrite,
    command: &str,
    codes: &[u16],
) -> Result<String, String> {
    stream
        .write_all(command.as_bytes())
        .and_then(|_| stream.write_all(b"\r\n"))
        .and_then(|_| stream.flush())
        .map_err(|error| format!("写入 SMTP 命令失败: {error}"))?;
    expect(stream, codes)
}

fn greet(stream: &mut dyn ReadWrite) -> Result<(), String> {
    expect(stream, &[220])?;
    Ok(())
}

fn auth_and_send(
    stream: &mut dyn ReadWrite,
    target: &SmtpTarget,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    if to.contains('\n')
        || to.contains('\r')
        || !to.contains('@')
        || to.chars().any(char::is_whitespace)
    {
        return Err("收件人不是有效邮箱地址".to_string());
    }
    // EHLO again after STARTTLS, per RFC 3207.
    smtp_command(stream, &format!("EHLO {}", target.host), &[250])?;
    if let (Some(username), Some(password)) =
        (target.username.as_deref(), target.password.as_deref())
    {
        let payload = format!("\0{}\0{}", username, password);
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            payload.as_bytes(),
        );
        let (code, text) = {
            stream
                .write_all(format!("AUTH PLAIN {encoded}\r\n").as_bytes())
                .and_then(|_| stream.flush())
                .map_err(|error| format!("写入 SMTP AUTH 失败: {error}"))?;
            read_response(stream)?
        };
        if !matches!(code, 235 | 503) {
            return Err(format!(
                "SMTP 认证失败 {code}: {}",
                text.chars().take(200).collect::<String>()
            ));
        }
    }
    let from = if target.from.contains('@') {
        target.from.clone()
    } else {
        format!("digilegacy@{}", target.host)
    };
    smtp_command(stream, &format!("MAIL FROM:<{from}>"), &[250])?;
    smtp_command(stream, &format!("RCPT TO:<{to}>"), &[250, 251])?;
    smtp_command(stream, "DATA", &[354])?;

    let encoded_subject = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        subject.as_bytes(),
    );
    let message = format!(
        "From: {from}\r\nTo: {to}\r\nSubject: =?UTF-8?B?{encoded_subject}?=\r\nDate: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n",
        chrono::Utc::now().to_rfc2822()
    );
    stream
        .write_all(message.as_bytes())
        .map_err(|error| format!("写入邮件头失败: {error}"))?;
    for line in body.replace("\r\n", "\n").split('\n') {
        let line = if line.starts_with('.') {
            format!(".{line}")
        } else {
            line.to_string()
        };
        stream
            .write_all(line.as_bytes())
            .and_then(|_| stream.write_all(b"\r\n"))
            .map_err(|error| format!("写入邮件正文失败: {error}"))?;
    }
    stream
        .write_all(b".\r\n")
        .and_then(|_| stream.flush())
        .map_err(|error| format!("提交邮件正文失败: {error}"))?;
    expect(stream, &[250])?;
    let _ = smtp_command(stream, "QUIT", &[221]);
    Ok(())
}

/// Send one UTF-8 text email through SMTP. Supports implicit TLS, STARTTLS,
/// plaintext, and AUTH PLAIN. Never logs or returns the password.
pub fn send_smtp(target: &SmtpTarget, to: &str, subject: &str, body: &str) -> Result<(), String> {
    send_smtp_inner(target, to, subject, body, &[])
}

fn send_smtp_inner(
    target: &SmtpTarget,
    to: &str,
    subject: &str,
    body: &str,
    extra_roots: &[CertificateDer<'static>],
) -> Result<(), String> {
    let addrs = resolve_tcp_host(&target.host, target.port)?;
    let mut last_error = String::from("连接失败");
    let mut connected = None;
    for addr in addrs {
        match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(10)) {
            Ok(stream) => {
                connected = Some(stream);
                break;
            }
            Err(error) => last_error = error.to_string(),
        }
    }
    let mut tcp = connected.ok_or_else(|| format!("连接 SMTP 失败: {last_error}"))?;
    tcp.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|error| format!("设置 SMTP 读超时失败: {error}"))?;
    tcp.set_write_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|error| format!("设置 SMTP 写超时失败: {error}"))?;
    if target.tls {
        let mut stream = tls_stream(tcp, &target.host, extra_roots)?;
        greet(&mut stream)?;
        return auth_and_send(&mut stream, target, to, subject, body);
    }
    if target.starttls {
        greet(&mut tcp)?;
        smtp_command(&mut tcp, &format!("EHLO {}", target.host), &[250])?;
        smtp_command(&mut tcp, "STARTTLS", &[220])?;
        let mut stream = tls_stream(tcp, &target.host, extra_roots)?;
        return auth_and_send(&mut stream, target, to, subject, body);
    }
    greet(&mut tcp)?;
    auth_and_send(&mut tcp, target, to, subject, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;

    fn test_cert_and_key() -> (
        rustls::pki_types::CertificateDer<'static>,
        PrivateKeyDer<'static>,
    ) {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        (
            cert.der().clone(),
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der())),
        )
    }

    fn test_server_config(
        cert: rustls::pki_types::CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
    ) -> Arc<rustls::ServerConfig> {
        let provider = rustls::crypto::ring::default_provider();
        Arc::new(
            rustls::ServerConfig::builder_with_provider(Arc::new(provider))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(vec![cert], key)
                .unwrap(),
        )
    }

    fn reply(stream: &mut dyn ReadWrite, text: &str) {
        stream.write_all(text.as_bytes()).unwrap();
        stream.flush().unwrap();
    }

    fn run_smtp_session(
        stream: &mut dyn ReadWrite,
        send_greeting: bool,
        stop_on_starttls: bool,
    ) -> String {
        if send_greeting {
            reply(stream, "220 local\r\n");
        }
        let mut data_mode = false;
        let mut body = String::new();
        while let Ok(line) = read_line(stream) {
            if data_mode {
                if line.trim_end() == "." {
                    reply(stream, "250 queued\r\n");
                    data_mode = false;
                } else {
                    body.push_str(&line);
                    body.push('\n');
                }
                continue;
            }
            let command = line.trim_end().to_ascii_uppercase();
            if command.starts_with("EHLO") {
                reply(stream, "250-localhost\r\n250-8BITMIME\r\n250 OK\r\n");
            } else if command.starts_with("MAIL") || command.starts_with("RCPT") {
                reply(stream, "250 OK\r\n");
            } else if command == "DATA" {
                reply(stream, "354 go ahead\r\n");
                data_mode = true;
            } else if command.starts_with("AUTH") {
                reply(stream, "235 authenticated\r\n");
            } else if command == "QUIT" {
                reply(stream, "221 bye\r\n");
                break;
            } else if command == "STARTTLS" && stop_on_starttls {
                reply(stream, "220 ready\r\n");
                return body;
            } else {
                reply(stream, "250 OK\r\n");
            }
        }
        body
    }

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
    fn smtp_delivery_works_against_a_local_plain_server() {
        use std::io::{BufRead, BufReader, Write};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            socket.write_all(b"220 local\r\n").unwrap();
            let mut data_mode = false;
            let mut body = String::new();
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 {
                    break;
                }
                if data_mode {
                    if line.trim_end() == "." {
                        socket.write_all(b"250 queued\r\n").unwrap();
                        data_mode = false;
                    } else {
                        body.push_str(&line);
                    }
                    continue;
                }
                let command = line.trim_end().to_ascii_uppercase();
                if command.starts_with("EHLO") {
                    socket
                        .write_all(b"250-localhost\r\n250-8BITMIME\r\n250 OK\r\n")
                        .unwrap();
                } else if command.starts_with("MAIL") || command.starts_with("RCPT") {
                    socket.write_all(b"250 OK\r\n").unwrap();
                } else if command == "DATA" {
                    socket.write_all(b"354 go ahead\r\n").unwrap();
                    data_mode = true;
                } else if command == "QUIT" {
                    socket.write_all(b"221 bye\r\n").unwrap();
                    break;
                } else if command.starts_with("STARTTLS") || command.starts_with("AUTH") {
                    socket.write_all(b"454 unavailable\r\n").unwrap();
                } else {
                    socket.write_all(b"250 OK\r\n").unwrap();
                }
            }
            body
        });
        let target = SmtpTarget::parse(
            &addr.to_string(),
            Some("none"),
            None,
            None,
            Some("sender@example.com".to_string()),
        )
        .unwrap();
        send_smtp(
            &target,
            "heir@example.com",
            "遗产通知",
            "请查收这份执行清单。",
        )
        .unwrap();
        let body = worker.join().unwrap();
        assert!(body.contains("heir@example.com"));
        assert!(body.contains("Content-Type: text/plain; charset=UTF-8"));
    }

    #[test]
    fn smtp_delivery_works_against_a_local_starttls_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (cert, key) = test_cert_and_key();
        let server_config = test_server_config(cert.clone(), key);
        let worker = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let mut plain = socket;
            let before_tls = run_smtp_session(&mut plain, true, true);
            assert!(before_tls.is_empty());
            let mut tls = rustls::StreamOwned::new(
                rustls::ServerConnection::new(server_config).unwrap(),
                plain,
            );
            run_smtp_session(&mut tls, false, false)
        });
        let target = SmtpTarget::parse(
            &format!("localhost:{}", addr.port()),
            Some("starttls"),
            Some("user@example.com".to_string()),
            Some("secret".to_string()),
            Some("sender@example.com".to_string()),
        )
        .unwrap();
        send_smtp_inner(
            &target,
            "heir@example.com",
            "遗产通知",
            "请查收这份执行清单。",
            &[cert],
        )
        .unwrap();
        let body = worker.join().unwrap();
        assert!(body.contains("To: heir@example.com"));
        assert!(body.contains("Content-Type: text/plain; charset=UTF-8"));
        assert!(body.contains("请查收这份执行清单。"));
    }

    #[test]
    fn smtp_delivery_works_against_a_local_implicit_tls_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (cert, key) = test_cert_and_key();
        let server_config = test_server_config(cert.clone(), key);
        let worker = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let mut tls = rustls::StreamOwned::new(
                rustls::ServerConnection::new(server_config).unwrap(),
                socket,
            );
            run_smtp_session(&mut tls, true, false)
        });
        let target = SmtpTarget::parse(
            &format!("localhost:{}", addr.port()),
            Some("tls"),
            Some("user@example.com".to_string()),
            Some("secret".to_string()),
            Some("sender@example.com".to_string()),
        )
        .unwrap();
        send_smtp_inner(
            &target,
            "heir@example.com",
            "遗产通知",
            "请查收这份执行清单。",
            &[cert],
        )
        .unwrap();
        let body = worker.join().unwrap();
        assert!(body.contains("To: heir@example.com"));
        assert!(body.contains("Content-Type: text/plain; charset=UTF-8"));
        assert!(body.contains("请查收这份执行清单。"));
    }

    #[test]
    fn empty_endpoint_is_unconfigured() {
        let result = probe("webhook", " ", None);
        assert!(!result.ok);
        assert_eq!(result.status, "unconfigured");
    }
}

// ---------------------------------------------------------------------------
// M5-D policy sync transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PolicyFetch {
    pub status: u16,
    pub body: String,
    pub etag: String,
    pub signature: String,
}

fn is_local_host(host: Option<&str>) -> bool {
    matches!(host, Some("localhost") | Some("127.0.0.1") | Some("::1"))
}

fn validate_policy_endpoint(endpoint: &str) -> Result<reqwest::Url, String> {
    let url =
        reqwest::Url::parse(endpoint.trim()).map_err(|error| format!("政策端点无效: {error}"))?;
    match url.scheme() {
        "https" if url.host().is_some() => Ok(url),
        "http" if is_local_host(url.host_str()) => Ok(url),
        _ => Err("政策端点必须使用 HTTPS；仅 localhost 允许 HTTP".to_string()),
    }
}

/// Fetch one policy-sync page on a dedicated OS thread. The returned body is
/// intentionally raw so the caller can verify an HMAC before parsing it.
pub fn fetch_policy_sync(
    endpoint: &str,
    token: Option<&str>,
    since: &str,
    etag: &str,
) -> Result<PolicyFetch, String> {
    let endpoint = endpoint.to_string();
    let token = token.map(str::to_string);
    let since = since.to_string();
    let etag = etag.to_string();
    std::thread::spawn(move || fetch_policy_sync_inner(&endpoint, token.as_deref(), &since, &etag))
        .join()
        .map_err(|_| "政策同步线程失败".to_string())?
}

fn fetch_policy_sync_inner(
    endpoint: &str,
    token: Option<&str>,
    since: &str,
    etag: &str,
) -> Result<PolicyFetch, String> {
    let mut url = validate_policy_endpoint(endpoint)?;
    if !since.trim().is_empty() {
        url.query_pairs_mut().append_pair("since", since.trim());
    }
    let resolved = resolve_external_url(url.as_str())?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .resolve_to_addrs(&resolved.host, &resolved.addrs)
        .user_agent("DigiLegacy/0.9 policy-sync")
        .build()
        .map_err(|error| format!("创建政策同步客户端失败: {error}"))?;
    let mut request = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(token) = token.filter(|value| !value.trim().is_empty()) {
        request = request.bearer_auth(token);
    }
    if !etag.trim().is_empty() {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag.trim());
    }
    let response = request
        .send()
        .map_err(|error| format!("政策同步请求失败: {error}"))?;
    let status = response.status().as_u16();
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let signature = response
        .headers()
        .get("x-digilegacy-signature")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response
        .text()
        .map_err(|error| format!("读取政策同步响应失败: {error}"))?;
    Ok(PolicyFetch {
        status,
        body,
        etag,
        signature,
    })
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for index in (0..value.len()).step_by(2) {
        bytes.push(u8::from_str_radix(&value[index..index + 2], 16).ok()?);
    }
    Some(bytes)
}

/// Verify `x-digilegacy-signature: sha256=<hex>` (or bare hex) over the raw
/// response body. A missing/invalid header fails closed when a secret is set.
pub fn verify_policy_signature(body: &str, header: &str, secret: &str) -> bool {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;

    let header = header.trim();
    let header = header
        .strip_prefix("sha256=")
        .or_else(|| header.strip_prefix("sha256:"))
        .unwrap_or(header)
        .trim();
    let Some(signature) = decode_hex(header) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body.as_bytes());
    mac.verify_slice(&signature).is_ok()
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const BODY: &str = "M5-D";
    const SECRET: &str = "secret";
    const SIGNATURE: &str = "e0f8acb11d8f958611ede07fac613f39b781977a4e747ef96b3c83f32116999f";

    fn sign(body: &str, secret: &str) -> String {
        use hmac::{Hmac, KeyInit, Mac};
        use sha2::Sha256;

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body.as_bytes());
        format!(
            "sha256={}",
            mac.finalize()
                .into_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    }

    fn serve_once(
        status: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let status = status.to_string();
        let headers: Vec<(String, String)> = headers
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        let body = body.to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n\r\n{}",
                body.len(),
                headers
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}"))
                    .collect::<Vec<_>>()
                    .join("\r\n"),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        });
        (format!("http://127.0.0.1:{port}/feed"), handle)
    }

    #[test]
    fn policy_signature_accepts_prefixed_and_bare_hex() {
        assert!(verify_policy_signature(BODY, SIGNATURE, SECRET));
        assert!(verify_policy_signature(
            BODY,
            &format!("sha256={SIGNATURE}"),
            SECRET
        ));
        assert!(verify_policy_signature(
            BODY,
            &format!("sha256:{SIGNATURE}"),
            SECRET
        ));
    }

    #[test]
    fn signed_feed_over_real_http_verifies_body_and_headers() {
        let body = r#"{"policies":[],"nextSince":"2026-11-20T00:00:00Z"}"#;
        let signature = sign(body, SECRET);
        let (url, handle) = serve_once(
            "200 OK",
            &[
                ("Content-Type", "application/json"),
                ("ETag", "\"feed-1\""),
                ("X-Digilegacy-Signature", &signature),
            ],
            body,
        );
        let fetched = fetch_policy_sync(&url, Some("read-token"), "", "").unwrap();
        handle.join().unwrap();
        assert_eq!(fetched.status, 200);
        assert_eq!(fetched.body, body);
        assert_eq!(fetched.etag, "\"feed-1\"");
        assert!(verify_policy_signature(
            &fetched.body,
            &fetched.signature,
            SECRET
        ));
        assert!(!verify_policy_signature(
            &fetched.body,
            &fetched.signature,
            "wrong-secret"
        ));
        assert!(!verify_policy_signature(
            r#"{"policies":[{"id":"tampered"}]}"#,
            &fetched.signature,
            SECRET
        ));
    }

    #[test]
    fn policy_feed_honours_etag_with_304_over_real_http() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer);
            let request = String::from_utf8_lossy(&buffer).to_ascii_lowercase();
            let status = if request.contains("if-none-match: \"feed-1\"") {
                "304 Not Modified"
            } else {
                "400 Bad Request"
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nETag: \"feed-1\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.write_all(response.as_bytes());
        });
        let fetched = fetch_policy_sync(
            &format!("http://127.0.0.1:{port}/feed"),
            None,
            "",
            "\"feed-1\"",
        )
        .unwrap();
        handle.join().unwrap();
        assert_eq!(fetched.status, 304);
        assert_eq!(fetched.etag, "\"feed-1\"");
    }

    #[test]
    fn policy_signature_fails_closed_on_missing_tampering_or_bad_hex() {
        assert!(!verify_policy_signature(BODY, "", SECRET));
        assert!(!verify_policy_signature("tampered", SIGNATURE, SECRET));
        assert!(!verify_policy_signature(BODY, "abc", SECRET));
        assert!(!verify_policy_signature(
            BODY,
            "zz".repeat(32).as_str(),
            SECRET
        ));
    }

    #[test]
    fn policy_endpoint_allows_https_and_localhost_http_only() {
        assert!(validate_policy_endpoint("https://example.com/v1/policies").is_ok());
        assert!(validate_policy_endpoint("http://127.0.0.1:8792/v1/policies").is_ok());
        assert!(validate_policy_endpoint("http://localhost/v1/policies").is_ok());
        assert!(validate_policy_endpoint("http://example.com/v1/policies").is_err());
        assert!(validate_policy_endpoint("file:///tmp/policies.json").is_err());
    }
}
