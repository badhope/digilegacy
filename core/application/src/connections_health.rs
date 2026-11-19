//! M5-B connection health use cases: list, probe, persist health.

use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use digilegacy_domains::connections;
use digilegacy_foundation::PlatformError;
use digilegacy_integrations::probe;

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
        let endpoint = input
            .endpoint
            .filter(|value| !value.trim().is_empty())
            .or_else(|| env_endpoint(&input.kind));
        let mut checked_at = Utc::now().to_rfc3339();
        let probe_result = match endpoint {
            Some(endpoint) => {
                let token = env_token(&input.kind);
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
