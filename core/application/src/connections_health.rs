//! M5-B connection health use cases: list, probe, persist health.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use digilegacy_domains::connections;
use digilegacy_foundation::PlatformError;
use digilegacy_integrations::{probe, SmtpTarget};
use digilegacy_security::{decrypt_field_any, encrypt_field_v2, validate_external_url};

use crate::{map_domain, parse, Application};

const ENV_KINDS: [&str; 8] = [
    "llm",
    "policy",
    "smtp",
    "imap",
    "heartbeat",
    "webhook",
    "feishu",
    "wecom",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredConfig {
    endpoint: String,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    security: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveInput {
    kind: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    label: Option<String>,
    endpoint: String,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    security: Option<String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

struct NormalizedConfig {
    id_hint: Option<String>,
    enabled: bool,
    stored: StoredConfig,
}

fn normalized_config(kind: &str, input: SaveInput) -> Result<NormalizedConfig, PlatformError> {
    let endpoint = input.endpoint.trim().to_string();
    if endpoint.is_empty() || endpoint.chars().any(char::is_control) {
        return Err(PlatformError::invalid_args(
            "连接地址不能为空且不能包含控制字符",
        ));
    }
    let is_http = matches!(
        kind,
        "llm" | "policy" | "heartbeat" | "webhook" | "feishu" | "wecom"
    );
    let endpoint = if is_http {
        validate_external_url(&endpoint).map_err(PlatformError::invalid_args)?
    } else {
        if endpoint.chars().any(char::is_whitespace) {
            return Err(PlatformError::invalid_args("SMTP/IMAP 地址不能包含空白"));
        }
        endpoint
    };
    let token = input
        .token
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if token.as_ref().is_some_and(|value| value.len() > 4096) {
        return Err(PlatformError::invalid_args("凭据长度超过 4096 字符"));
    }
    let username = input
        .username
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let from = input
        .from
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if from
        .as_deref()
        .is_some_and(|value| value.contains('\n') || value.contains('\r'))
    {
        return Err(PlatformError::invalid_args("发件人不能包含换行"));
    }
    let security = input
        .security
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(value) = security.as_deref() {
        if !matches!(value, "tls" | "starttls" | "none") {
            return Err(PlatformError::invalid_args(
                "security 必须是 tls/starttls/none",
            ));
        }
    }
    let id_hint = input
        .id
        .map(|value| value.trim().trim_matches('"').to_string())
        .filter(|value| !value.is_empty());
    Ok(NormalizedConfig {
        id_hint,
        enabled: input.enabled,
        stored: StoredConfig {
            endpoint,
            token,
            username,
            from,
            security,
        },
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestInput {
    kind: String,
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn host_port(host_env: &str, port_env: &str, default_port: u16) -> Option<String> {
    let host = env_value(host_env)?;
    let port = env_value(port_env)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port);
    Some(if host.contains(':') {
        host
    } else {
        format!("{host}:{port}")
    })
}

/// Resolve the endpoint of an environment-managed channel. Never returns a
/// credential-bearing URL to the client; it is used for probes only.
fn env_endpoint(kind: &str) -> Option<String> {
    match kind {
        "llm" => env_value("DIGILEGACY_LLM_BASE_URL"),
        "policy" => env_value("DIGILEGACY_POLICY_URL"),
        "heartbeat" => env_value("DIGILEGACY_HEARTBEAT_URL"),
        "webhook" => env_value("DIGILEGACY_WEBHOOK_URL"),
        "feishu" => env_value("DIGILEGACY_FEISHU_WEBHOOK_URL"),
        "wecom" => env_value("DIGILEGACY_WECOM_WEBHOOK_URL"),
        "smtp" => host_port("DIGILEGACY_SMTP_HOST", "DIGILEGACY_SMTP_PORT", 587),
        "imap" => host_port("DIGILEGACY_IMAP_HOST", "DIGILEGACY_IMAP_PORT", 993),
        _ => None,
    }
}

fn env_token(kind: &str) -> Option<String> {
    match kind {
        "llm" => env_value("DIGILEGACY_LLM_API_KEY"),
        "webhook" => env_value("DIGILEGACY_WEBHOOK_TOKEN"),
        "policy" => env_value("DIGILEGACY_POLICY_TOKEN"),
        "smtp" => env_value("DIGILEGACY_SMTP_PASSWORD"),
        "imap" => env_value("DIGILEGACY_IMAP_PASSWORD"),
        _ => None,
    }
}

impl Application {
    /// Seed metadata for environment-managed channels at startup. Rows are
    /// metadata-only (`config_enc = 'env'`); no endpoint is persisted.
    pub fn seed_environment_connections(&self) -> Result<(), String> {
        for kind in ENV_KINDS {
            // Only create/enable what this process can actually see. A CLI
            // launched with a partial environment must not disable a server's
            // existing channel just because the variable is absent here.
            if env_endpoint(kind).is_some() {
                connections::ensure_env(self.store(), kind, "env", true)
                    .map_err(|error| format!("初始化连接 {kind} 失败: {error}"))?;
            }
        }
        Ok(())
    }

    pub fn connections_list(&self, _args: Value) -> Result<Value, PlatformError> {
        connections::list(self.store())
            .map(|page| json!(page))
            .map_err(map_domain)
    }

    /// Save an encrypted connection config. Secrets are accepted only from the
    /// human HTTP/CLI surface and never echoed by the read model.
    pub fn connections_upsert(&self, args: Value) -> Result<Value, PlatformError> {
        let input: SaveInput = parse(args)?;
        let kind = input.kind.trim().to_ascii_lowercase();
        if !connections::is_kind(&kind) {
            return Err(PlatformError::invalid_args(format!("未知连接类型: {kind}")));
        }
        let label = input
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("default")
            .to_string();
        let config = normalized_config(&kind, input)?;
        let key = self.config_key().ok_or_else(|| {
            PlatformError::new(
                "CONFIG_KEY_MISSING",
                "连接配置密钥未配置；请设置 DIGILEGACY_CONFIG_KEY_FILE 或 DIGILEGACY_DB_KEY_FILE",
            )
        })?;
        let plain = serde_json::to_string(&config.stored)
            .map_err(|error| PlatformError::new("INTERNAL", error.to_string()))?;
        let config_enc = encrypt_field_v2(&plain, key).map_err(|error| {
            PlatformError::new("INTERNAL", format!("加密连接配置失败: {error}"))
        })?;
        let saved = connections::upsert(
            self.store(),
            config.id_hint.as_deref(),
            &kind,
            &label,
            &config_enc,
            config.enabled,
        )
        .map_err(map_domain)?;
        self.audit(
            "connection.upsert",
            &format!("id={};kind={kind};label={label}", saved.id),
        )?;
        Ok(json!(saved))
    }

    pub fn connections_delete(&self, args: Value) -> Result<Value, PlatformError> {
        let id = args
            .get("id")
            .and_then(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| Some(value.to_string()))
            })
            .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
        let id = id.trim_matches('"');
        let (kind, label) = connections::delete(self.store(), id).map_err(map_domain)?;
        self.audit(
            "connection.delete",
            &format!("id={id};kind={kind};label={label}"),
        )?;
        Ok(json!({ "id": id, "kind": kind, "label": label }))
    }

    /// Resolve the first enabled saved HTTP notification channel.
    pub(crate) fn saved_http_notification_target(
        &self,
        kind: &str,
    ) -> Result<Option<(String, Option<String>)>, PlatformError> {
        let page = connections::list(self.store()).map_err(map_domain)?;
        for item in page.items.iter().filter(|item| {
            item.kind == kind && item.enabled && matches!(kind, "webhook" | "feishu" | "wecom")
        }) {
            let config = self.load_connection_config(item)?.unwrap_or_default();
            if config.endpoint.trim().is_empty() {
                continue;
            }
            return Ok(Some((config.endpoint, config.token)));
        }
        Ok(None)
    }

    /// Resolve an enabled SMTP connection for outbox delivery. Saved rows are
    /// preferred over environment variables; the first enabled row wins.
    pub(crate) fn smtp_notification_target(&self) -> Result<Option<SmtpTarget>, PlatformError> {
        let page = connections::list(self.store()).map_err(map_domain)?;
        for item in page
            .items
            .iter()
            .filter(|item| item.kind == "smtp" && item.enabled)
        {
            let config = self.load_connection_config(item)?.unwrap_or_default();
            if config.endpoint.trim().is_empty() {
                continue;
            }
            let target = SmtpTarget::parse(
                &config.endpoint,
                config.security.as_deref(),
                config.username,
                config.token,
                config.from,
            )
            .map_err(PlatformError::invalid_args)?;
            return Ok(Some(target));
        }
        let Some(endpoint) = env_endpoint("smtp") else {
            return Ok(None);
        };
        SmtpTarget::parse(
            &endpoint,
            env_value("DIGILEGACY_SMTP_SECURITY").as_deref(),
            env_value("DIGILEGACY_SMTP_USERNAME"),
            env_value("DIGILEGACY_SMTP_PASSWORD"),
            env_value("DIGILEGACY_SMTP_FROM"),
        )
        .map(Some)
        .map_err(PlatformError::invalid_args)
    }

    fn load_connection_config(
        &self,
        row: &connections::Connection,
    ) -> Result<Option<StoredConfig>, PlatformError> {
        if row.source == "env" {
            return Ok(Some(StoredConfig {
                endpoint: env_endpoint(&row.kind).unwrap_or_default(),
                token: env_token(&row.kind),
                username: env_value("DIGILEGACY_SMTP_USERNAME"),
                from: env_value("DIGILEGACY_SMTP_FROM"),
                security: env_value("DIGILEGACY_SMTP_SECURITY"),
            }));
        }
        let encrypted = connections::config_enc(self.store(), &row.id).map_err(map_domain)?;
        let key = self
            .config_key()
            .ok_or_else(|| PlatformError::new("CONFIG_KEY_MISSING", "连接配置密钥未配置"))?;
        let plain = decrypt_field_any(&encrypted, key).map_err(|error| {
            PlatformError::new("CORRUPT_DATA", format!("解密连接配置失败: {error}"))
        })?;
        let config = serde_json::from_str(&plain).map_err(|error| {
            PlatformError::new("CORRUPT_DATA", format!("连接配置损坏: {error}"))
        })?;
        Ok(Some(config))
    }

    pub fn connections_test(&self, args: Value) -> Result<Value, PlatformError> {
        let input: TestInput = parse(args)?;
        let page = connections::list(self.store()).map_err(map_domain)?;
        let row = if let Some(id) = input.id {
            Some(
                page.items
                    .iter()
                    .find(|item| item.id == id.to_string())
                    .cloned()
                    .ok_or_else(|| PlatformError::new("NOT_FOUND", format!("连接 {id} 不存在")))?,
            )
        } else {
            page.items
                .iter()
                .find(|item| {
                    item.kind == input.kind
                        && item.enabled
                        && input
                            .label
                            .as_ref()
                            .map(|label| &item.label == label)
                            .unwrap_or(true)
                })
                .or_else(|| {
                    page.items
                        .iter()
                        .find(|item| item.kind == input.kind && item.enabled)
                })
                .cloned()
        };

        if let Some(row) = &row {
            if row.kind != input.kind {
                return Err(PlatformError::invalid_args(format!(
                    "连接 {} 的类型是 {}，不能用 {} 测试",
                    row.id, row.kind, input.kind
                )));
            }
            if !row.enabled {
                return Ok(json!({
                    "ok": false,
                    "status": "disabled",
                    "latencyMs": 0,
                    "message": "连接已停用",
                    "connectionId": row.id,
                    "checkedAt": Utc::now().to_rfc3339(),
                }));
            }
        }

        let connection_id = row.as_ref().map(|item| item.id.clone());
        let stored = row
            .as_ref()
            .map(|item| self.load_connection_config(item))
            .transpose()?
            .flatten();
        let endpoint = input
            .endpoint
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                stored.as_ref().and_then(|config| {
                    (!config.endpoint.trim().is_empty()).then(|| config.endpoint.clone())
                })
            })
            .or_else(|| env_endpoint(&input.kind));
        let token = stored
            .as_ref()
            .and_then(|config| config.token.clone())
            .or_else(|| env_token(&input.kind));
        let mut checked_at = Utc::now().to_rfc3339();
        let probe_result = match endpoint {
            Some(endpoint) => {
                let probe_result = probe(&input.kind, &endpoint, token.as_deref());
                if let Some(id) = connection_id.as_deref() {
                    let checked = connections::record_check(
                        self.store(),
                        id,
                        &probe_result.status,
                        if probe_result.ok {
                            ""
                        } else {
                            &probe_result.message
                        },
                    )
                    .map_err(map_domain)?;
                    checked_at = checked.last_check_at;
                }
                probe_result
            }
            None => {
                let probe_result = digilegacy_integrations::ProbeResult {
                    ok: false,
                    status: "unconfigured".to_string(),
                    latency_ms: 0,
                    message: "未配置连接地址".to_string(),
                };
                if let Some(id) = connection_id.as_deref() {
                    let checked = connections::record_check(
                        self.store(),
                        id,
                        &probe_result.status,
                        &probe_result.message,
                    )
                    .map_err(map_domain)?;
                    checked_at = checked.last_check_at;
                }
                probe_result
            }
        };
        self.audit(
            "connection.test",
            &format!(
                "kind={};status={};connection={}",
                input.kind,
                probe_result.status,
                connection_id.as_deref().unwrap_or("-")
            ),
        )?;
        let mut response = json!({
            "ok": probe_result.ok,
            "status": probe_result.status,
            "latencyMs": probe_result.latency_ms,
            "message": probe_result.message,
            "checkedAt": checked_at,
        });
        if let Some(id) = connection_id {
            response["connectionId"] = json!(id);
        }
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use digilegacy_store::Store;
    use std::sync::Arc;

    fn make_app(with_key: bool) -> Application {
        let store = Arc::new(Store::open_in_memory(None).unwrap());
        if with_key {
            Application::with_config_key(store, [7u8; 32])
        } else {
            Application::new(store)
        }
    }

    #[test]
    fn stored_config_is_encrypted_and_never_listed() {
        let app = make_app(true);
        let saved = app
            .connections_upsert(json!({
                "kind": "webhook",
                "label": "family",
                "endpoint": "http://127.0.0.1:8791/hook",
                "token": "top-secret-token",
                "enabled": true
            }))
            .unwrap();
        assert_eq!(saved["source"], "stored");
        let list = app.connections_list(json!({})).unwrap();
        assert_eq!(list["items"][0]["label"], "family");
        assert!(!serde_json::to_string(&list).unwrap().contains("top-secret"));
        let id = saved["id"].as_str().unwrap();
        let encrypted = connections::config_enc(app.store(), id).unwrap();
        assert!(encrypted.starts_with("enc:v2:"));
        assert!(!encrypted.contains("top-secret-token"));
    }

    #[test]
    fn config_write_requires_a_key_and_env_rows_are_immutable() {
        let app = make_app(false);
        let error = app
            .connections_upsert(json!({
                "kind": "webhook",
                "endpoint": "http://127.0.0.1:8791/hook"
            }))
            .unwrap_err();
        assert_eq!(error.code, "CONFIG_KEY_MISSING");

        let app = make_app(true);
        connections::ensure_env(app.store(), "webhook", "env", true).unwrap();
        let error = app
            .connections_upsert(json!({
                "kind": "webhook",
                "label": "env",
                "endpoint": "http://127.0.0.1:8791/hook"
            }))
            .unwrap_err();
        assert_eq!(error.code, "CONFLICT");
        let env_id = connections::list(app.store()).unwrap().items[0].id.clone();
        assert_eq!(
            app.connections_delete(json!({ "id": env_id }))
                .unwrap_err()
                .code,
            "CONFLICT"
        );
    }

    #[test]
    fn saved_smtp_config_resolves_for_notification_delivery() {
        let app = make_app(true);
        app.connections_upsert(json!({
            "kind": "smtp",
            "label": "mail",
            "endpoint": "127.0.0.1:2525",
            "username": "mailer",
            "token": "password",
            "from": "digilegacy@example.com",
            "security": "none"
        }))
        .unwrap();
        let target = app.smtp_notification_target().unwrap().unwrap();
        assert_eq!(target.host, "127.0.0.1");
        assert_eq!(target.port, 2525);
        assert!(!target.tls);
        assert!(!target.starttls);
        assert_eq!(target.from, "digilegacy@example.com");
    }

    #[test]
    fn stored_connection_can_be_deleted() {
        let app = make_app(true);
        let saved = app
            .connections_upsert(json!({
                "kind": "webhook",
                "label": "temp",
                "endpoint": "http://127.0.0.1:8791/hook",
                "token": "secret"
            }))
            .unwrap();
        let id = saved["id"].as_str().unwrap();
        let deleted = app.connections_delete(json!({ "id": id })).unwrap();
        assert_eq!(deleted["kind"], "webhook");
        assert_eq!(app.connections_list(json!({})).unwrap()["total"], 0);
    }
}
