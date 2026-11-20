//! M5-D policy sync use case: fetch, queue candidates, human decision.

use serde::Deserialize;
use serde_json::{json, Value};

use digilegacy_domains::{
    policy,
    policy_sync::{self as domain_sync, ListInput as CandidateListInput, PendingPolicy},
};
use digilegacy_foundation::PlatformError;
use digilegacy_integrations::{fetch_policy_sync, verify_policy_signature};
use digilegacy_security::{decrypt_field_any, encrypt_field_v2};

use crate::{map_domain, parse, Application};

const MAX_REMOTE_POLICIES: usize = 512;
const POLICY_SIGNING_KEY_ENV: &str = "DIGILEGACY_POLICY_SIGNING_KEY";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncInput {
    #[serde(default)]
    since: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecideInput {
    id: String,
    action: String,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Default)]
struct RemotePage {
    policies: Vec<policy::SeedPolicy>,
    cursor: String,
    etag: String,
}

fn remote_page(body: &str) -> Result<RemotePage, PlatformError> {
    let value: Value = serde_json::from_str(body).map_err(|error| {
        PlatformError::new("PROVIDER_ERROR", format!("政策响应不是 JSON: {error}"))
    })?;
    let policies_value = if value.is_array() {
        value.clone()
    } else {
        value
            .get("policies")
            .or_else(|| value.get("items"))
            .cloned()
            .ok_or_else(|| PlatformError::new("PROVIDER_ERROR", "政策响应缺少 policies/items"))?
    };
    let policies: Vec<policy::SeedPolicy> =
        serde_json::from_value(policies_value).map_err(|error| {
            PlatformError::new("PROVIDER_ERROR", format!("政策列表格式错误: {error}"))
        })?;
    if policies.len() > MAX_REMOTE_POLICIES {
        return Err(PlatformError::new(
            "PROVIDER_ERROR",
            format!("单次政策响应超过 {MAX_REMOTE_POLICIES} 条，已拒绝"),
        ));
    }
    let cursor = value
        .get("nextCursor")
        .or_else(|| value.get("next_cursor"))
        .or_else(|| value.get("cursor"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let etag = value
        .get("etag")
        .or_else(|| value.get("ETag"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(RemotePage {
        policies,
        cursor,
        etag,
    })
}

fn display_source(endpoint: &str) -> String {
    reqwest::Url::parse(endpoint)
        .map(|mut url| {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_query(None);
            url.to_string()
        })
        .unwrap_or_else(|_| endpoint.to_string())
}

fn provider_error(error: String) -> PlatformError {
    if error.contains("timed out") || error.contains("timeout") {
        PlatformError::new("TIMEOUT", error)
    } else {
        PlatformError::new("NETWORK", error)
    }
}

impl Application {
    fn encode_candidate_payload(
        &self,
        policy: &policy::SeedPolicy,
    ) -> Result<String, PlatformError> {
        let plain = serde_json::to_string(policy)
            .map_err(|error| PlatformError::new("INTERNAL", format!("序列化候选失败: {error}")))?;
        match self.config_key()? {
            Some(key) => encrypt_field_v2(&plain, &key).map_err(|error| {
                PlatformError::new("INTERNAL", format!("加密政策候选失败: {error}"))
            }),
            None => Ok(format!("plain:{plain}")),
        }
    }

    fn decode_candidate_payload(&self, payload: &str) -> Result<policy::SeedPolicy, PlatformError> {
        let plain = if let Some(plain) = payload.strip_prefix("plain:") {
            plain.to_string()
        } else if payload.starts_with("enc:") {
            let key = self.config_key()?.ok_or_else(|| {
                PlatformError::new(
                    "CONFIG_KEY_MISSING",
                    "政策候选已加密；请设置 DIGILEGACY_CONFIG_KEY_FILE 或 DIGILEGACY_DB_KEY_FILE",
                )
            })?;
            decrypt_field_any(payload, &key).map_err(|error| {
                PlatformError::new("CORRUPT_DATA", format!("解密政策候选失败: {error}"))
            })?
        } else {
            payload.to_string()
        };
        serde_json::from_str(&plain)
            .map_err(|error| PlatformError::new("CORRUPT_DATA", format!("政策候选损坏: {error}")))
    }

    pub fn policies_sync(&self, args: Value) -> Result<Value, PlatformError> {
        let input: SyncInput = parse(args)?;
        let (endpoint, token) = self.policy_sync_target()?.ok_or_else(|| {
            PlatformError::new(
                "PROVIDER_UNCONFIGURED",
                "未配置政策连接；请在设置中保存 policy 连接或设置 DIGILEGACY_POLICY_URL",
            )
        })?;
        digilegacy_security::validate_external_url(&endpoint)
            .map_err(PlatformError::invalid_args)?;
        let source = display_source(&endpoint);
        let state = domain_sync::state(self.store()).map_err(map_domain)?;
        let since = if input.since.trim().is_empty() {
            state.cursor.clone()
        } else {
            input.since.trim().to_string()
        };
        let fetched = fetch_policy_sync(&endpoint, token.as_deref(), &since, &state.etag)
            .map_err(provider_error)?;

        if fetched.status == 304 {
            let state = domain_sync::record_state(
                self.store(),
                &source,
                &since,
                &state.etag,
                "not_modified",
                "",
            )
            .map_err(map_domain)?;
            self.audit("policy.sync", "status=not_modified")?;
            return Ok(json!({
                "configured": true,
                "source": source,
                "fetched": 0,
                "changed": 0,
                "queued": 0,
                "conflict": 0,
                "skipped": 0,
                "notModified": true,
                "cursor": state.cursor,
                "lastSyncAt": state.last_sync_at,
                "message": "远端未修改（304），没有入队候选",
            }));
        }

        if !(200..300).contains(&fetched.status) {
            let (code, message) = match fetched.status {
                401 | 403 => ("PROVIDER_AUTH", "政策端点鉴权失败"),
                429 => ("RATE_LIMITED", "政策端点限流"),
                500..=599 => ("PROVIDER_ERROR", "政策端点返回 5xx"),
                _ => ("PROVIDER_ERROR", "政策端点返回非 2xx"),
            };
            let error = format!("{message}（HTTP {}）", fetched.status);
            domain_sync::record_state(self.store(), &source, &since, &state.etag, "error", &error)
                .map_err(map_domain)?;
            return Err(PlatformError::new(code, error));
        }

        let signing_key = std::env::var(POLICY_SIGNING_KEY_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty());
        if let Some(key) = signing_key {
            if !verify_policy_signature(&fetched.body, &fetched.signature, &key) {
                let error = "政策响应签名校验失败；候选未入队";
                domain_sync::record_state(
                    self.store(),
                    &source,
                    &since,
                    &state.etag,
                    "error",
                    error,
                )
                .map_err(map_domain)?;
                return Err(PlatformError::new("PROVIDER_ERROR", error));
            }
        }

        let remote = remote_page(&fetched.body)?;
        if remote.policies.is_empty() {
            let cursor = if remote.cursor.is_empty() {
                since.clone()
            } else {
                remote.cursor.clone()
            };
            let etag = if remote.etag.is_empty() {
                fetched.etag.clone()
            } else {
                remote.etag.clone()
            };
            let state = domain_sync::record_state(self.store(), &source, &cursor, &etag, "ok", "")
                .map_err(map_domain)?;
            self.audit("policy.sync", "fetched=0;queued=0")?;
            return Ok(json!({
                "configured": true,
                "source": source,
                "fetched": 0,
                "changed": 0,
                "queued": 0,
                "conflict": 0,
                "skipped": 0,
                "notModified": false,
                "cursor": state.cursor,
                "lastSyncAt": state.last_sync_at,
                "message": "远端没有政策条目",
            }));
        }

        let mut pending = Vec::with_capacity(remote.policies.len());
        for item in &remote.policies {
            pending.push(PendingPolicy {
                policy: item.clone(),
                payload_enc: self.encode_candidate_payload(item)?,
            });
        }
        let stats = domain_sync::enqueue(self.store(), &source, pending).map_err(map_domain)?;
        let cursor = if remote.cursor.is_empty() {
            since.clone()
        } else {
            remote.cursor.clone()
        };
        let etag = if remote.etag.is_empty() {
            fetched.etag.clone()
        } else {
            remote.etag.clone()
        };
        let state = domain_sync::record_state(self.store(), &source, &cursor, &etag, "ok", "")
            .map_err(map_domain)?;
        self.audit(
            "policy.sync",
            &format!(
                "fetched={};queued={};conflict={};skipped={}",
                stats.fetched, stats.queued, stats.conflict, stats.skipped
            ),
        )?;
        let message = if stats.queued == 0 {
            format!("拉取 {} 条，无新候选；政策库未改变", stats.fetched)
        } else {
            format!(
                "拉取 {} 条，入队 {} 个候选（冲突 {}），政策库未改变",
                stats.fetched, stats.queued, stats.conflict
            )
        };
        Ok(json!({
            "configured": true,
            "source": source,
            "fetched": stats.fetched,
            "changed": stats.changed,
            "queued": stats.queued,
            "conflict": stats.conflict,
            "skipped": stats.skipped,
            "notModified": false,
            "cursor": state.cursor,
            "lastSyncAt": state.last_sync_at,
            "message": message,
        }))
    }

    pub fn policies_candidates_list(&self, args: Value) -> Result<Value, PlatformError> {
        let input: CandidateListInput = parse(args)?;
        let page = domain_sync::list(self.store(), &input).map_err(map_domain)?;
        let state = domain_sync::state(self.store()).map_err(map_domain)?;
        Ok(json!({
            "items": page.items,
            "nextCursor": page.next_cursor,
            "total": page.total,
            "lastSyncAt": state.last_sync_at,
            "lastStatus": state.last_status,
        }))
    }

    pub fn policies_candidate_decide(&self, args: Value) -> Result<Value, PlatformError> {
        let input: DecideInput = parse(args)?;
        let action = input.action.trim().to_ascii_lowercase();
        if !matches!(action.as_str(), "approve" | "reject") {
            return Err(PlatformError::invalid_args("action 必须是 approve/reject"));
        }
        let reviewer = "local-user";
        let result = if action == "approve" {
            let (payload, hint) =
                domain_sync::payload_enc(self.store(), &input.id).map_err(map_domain)?;
            if !matches!(hint.status.as_str(), "pending" | "conflict") {
                return Err(PlatformError::new(
                    "CONFLICT",
                    format!("候选 {} 已是 {}，不能重复审核", input.id, hint.status),
                ));
            }
            let remote = self.decode_candidate_payload(&payload)?;
            if remote.id != hint.platform_id {
                return Err(PlatformError::new(
                    "CONFLICT",
                    format!(
                        "候选平台 {} 与载荷平台 {} 不一致",
                        hint.platform_id, remote.id
                    ),
                ));
            }
            domain_sync::approve(self.store(), &input.id, reviewer, &remote).map_err(map_domain)?
        } else {
            domain_sync::reject(self.store(), &input.id, reviewer).map_err(map_domain)?
        };
        self.audit(
            if action == "approve" {
                "policy.candidate.approve"
            } else {
                "policy.candidate.reject"
            },
            &format!(
                "id={};platform={};note={}",
                result.id,
                result.platform_id,
                input.note.trim()
            ),
        )?;
        Ok(json!(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use digilegacy_domains::policy::ProvenanceInput;
    use digilegacy_store::Store;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;

    fn app_with_key() -> Application {
        let store = Arc::new(Store::open_in_memory(None).unwrap());
        policy::seed_if_empty(&store).unwrap();
        Application::with_config_key(store, [9u8; 32])
    }

    fn remote_policy() -> policy::SeedPolicy {
        policy::SeedPolicy {
            id: 1,
            name: "支付宝".to_string(),
            name_en: "Alipay".to_string(),
            category: "支付金融".to_string(),
            subtype: "移动支付".to_string(),
            has_property: true,
            has_personality: false,
            inheritable: true,
            required_documents: "死亡证明".to_string(),
            process_description: "远程流程".to_string(),
            estimated_difficulty: "中等".to_string(),
            official_policy_url: "https://example.com/alipay".to_string(),
            estimated_time_days: "30 天".to_string(),
            notes: "远程同步备注".to_string(),
            data_source: "远程来源".to_string(),
            inheritance_form_url: String::new(),
            monetization_path: "远程变现".to_string(),
            legal_template_ids: "a,b".to_string(),
            risk_warnings: "远程风险".to_string(),
            tags: "需公证".to_string(),
        }
    }

    fn spawn_feed(body: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer);
            let body = body.clone();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        });
        format!("http://{address}/v1/policies")
    }

    #[test]
    fn sync_requires_a_policy_target() {
        let app = app_with_key();
        let error = app.policies_sync(json!({})).unwrap_err();
        assert_eq!(error.code, "PROVIDER_UNCONFIGURED");
    }

    #[test]
    fn sync_queues_candidate_and_approval_updates_policy_only_after_decision() {
        let app = app_with_key();
        let body = json!({
            "policies": [remote_policy()],
            "cursor": "cursor-1",
            "etag": "etag-1"
        })
        .to_string();
        let endpoint = spawn_feed(body);
        app.connections_upsert(json!({
            "kind": "policy",
            "label": "test",
            "endpoint": endpoint,
            "token": "policy-token",
            "enabled": true
        }))
        .unwrap();
        let result = app.policies_sync(json!({})).unwrap();
        assert_eq!(result["queued"], 1);
        assert_eq!(result["cursor"], "cursor-1");
        let before = policy::get(app.store(), "1").unwrap();
        assert_eq!(before.notes, "5万以下走简易继承程序（2025银行新规）");
        let candidates = app.policies_candidates_list(json!({})).unwrap();
        let id = candidates["items"][0]["id"].as_str().unwrap();
        let approved = app
            .policies_candidate_decide(json!({"id": id, "action": "approve"}))
            .unwrap();
        assert_eq!(approved["status"], "approved");
        let after = policy::get(app.store(), "1").unwrap();
        assert_eq!(after.notes, "远程同步备注");
        assert_eq!(after.provenance.status, "unverified");
    }

    #[test]
    fn conflict_candidate_can_be_rejected_without_touching_content() {
        let app = app_with_key();
        policy::set_provenance(
            app.store(),
            &ProvenanceInput {
                id: "1".to_string(),
                status: "verified".to_string(),
                source_url: "https://example.com".to_string(),
                note: "人工核对".to_string(),
            },
        )
        .unwrap();
        let endpoint = spawn_feed(json!({"policies":[remote_policy()]}).to_string());
        app.connections_upsert(json!({
            "kind": "policy",
            "label": "test",
            "endpoint": endpoint,
            "enabled": true
        }))
        .unwrap();
        let result = app.policies_sync(json!({})).unwrap();
        assert_eq!(result["conflict"], 1);
        let candidates = app
            .policies_candidates_list(json!({"status":"conflict"}))
            .unwrap();
        let id = candidates["items"][0]["id"].as_str().unwrap();
        let rejected = app
            .policies_candidate_decide(json!({"id": id, "action": "reject"}))
            .unwrap();
        assert_eq!(rejected["status"], "rejected");
        let current = policy::get(app.store(), "1").unwrap();
        assert_eq!(current.notes, "5万以下走简易继承程序（2025银行新规）");
    }

    #[test]
    fn sync_state_keeps_cursor_and_etag() {
        let app = app_with_key();
        let endpoint = spawn_feed(json!({"policies":[],"nextCursor":"n2"}).to_string());
        app.connections_upsert(json!({
            "kind": "policy",
            "label": "test",
            "endpoint": endpoint,
            "enabled": true
        }))
        .unwrap();
        let result = app.policies_sync(json!({})).unwrap();
        assert_eq!(result["cursor"], "n2");
        assert_eq!(result["queued"], 0);
        let state = domain_sync::state(app.store()).unwrap();
        assert_eq!(state.cursor, "n2");
    }
}
