//! Application layer: use cases shared by HTTP, CLI and Agent tools.
//!
//! Entry points own transport permissions; this layer owns command semantics.
//! It reads/writes one Store and never duplicates SQL in an app crate.

#![forbid(unsafe_code)]

use digilegacy_domains::{approvals, assets, audit, overview, plan, policy, runs, DomainError};
use serde_json::{json, Value};

pub mod deliver;
pub mod jobs;
pub mod runtime;
use digilegacy_foundation::PlatformError;
use digilegacy_security::keyring::FileKeyring;
use digilegacy_store::Store;
use std::sync::Arc;

pub const STATUS: &str = "m2-read-write-core";

#[derive(Clone)]
pub struct Application {
    store: Arc<Store>,
}

impl Application {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    /// Open the local database from `DIGILEGACY_DB` / `DIGILEGACY_DB_KEY_FILE`.
    /// Both server and CLI use this so a platform process and a CLI process
    /// address the same file/key pair.
    pub fn from_env() -> Result<Self, String> {
        let db_path =
            std::env::var("DIGILEGACY_DB").unwrap_or_else(|_| "data/digilegacy.db".to_string());
        if let Some(parent) = std::path::Path::new(&db_path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建数据目录失败: {e}"))?;
        }
        let store = match std::env::var("DIGILEGACY_DB_KEY_FILE") {
            Ok(key_path) => {
                let key = FileKeyring::new(key_path).load_or_create()?;
                let key_ref: &[u8; 32] = &key;
                Store::open(&db_path, Some(key_ref))
            }
            Err(_) => Store::open(&db_path, None),
        }
        .map_err(|e| format!("打开数据库失败: {e}"))?;
        let app = Self::new(Arc::new(store));
        policy::seed_if_empty(app.store()).map_err(|e| format!("政策库初始化失败: {e}"))?;
        Ok(app)
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn health(&self) -> Value {
        json!({
            "status": "ok",
            "mode": "m2",
            "version": env!("CARGO_PKG_VERSION"),
            "contracts": digilegacy_contracts::COMMANDS.len(),
            "schemaVersion": self.store.schema_version().unwrap_or(-1),
            "tables": self.store.table_count().unwrap_or(-1),
            "store": format!("{:?}", self.store.state()),
        })
    }

    pub fn dispatch(&self, name: &str, args: Value) -> Result<Value, PlatformError> {
        match name {
            "app.overview" => self.overview(),
            "audit.list" => self.audit_list(args),
            "assets.list" => self.assets_list(args),
            "assets.get" => self.assets_get(args),
            "assets.create" => self.assets_create(args),
            "assets.update" => self.assets_update(args),
            "plan.get" => self.plan(),
            "policies.list" => self.policies_list(args),
            "policies.get" => self.policies_get(args),
            "policies.provenance.set" => self.policies_set_provenance(args),
            "trigger.checkin" => self.checkin(),
            "deliver.export" => self.deliver_export(args),
            "security.status" => self.security_status(),
            "agent.runs.list" => self.runs_list(args),
            "agent.runs.get" => self.runs_get(args),
            "agent.runs.start" => self.runs_start(args),
            "agent.runs.plan" => runtime::plan(self, args),
            "agent.runs.cancel" => runtime::cancel(self, args),
            "agent.runs.retry" => runtime::retry(self, args),
            "agent.approvals.list" => self.approvals_list(args),
            "agent.approvals.decide" => self.approvals_decide(args),
            "jobs.get" => self.jobs_get(args),
            "jobs.cancel" => self.jobs_cancel(args),
            other => Err(PlatformError::unsupported(other)),
        }
    }

    pub fn overview(&self) -> Result<Value, PlatformError> {
        overview::get(&self.store)
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn assets_list(&self, args: Value) -> Result<Value, PlatformError> {
        let input: assets::ListInput = parse(args)?;
        assets::list(&self.store, &input)
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn assets_get(&self, args: Value) -> Result<Value, PlatformError> {
        let id = args
            .get("id")
            .and_then(|v| {
                v.as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| Some(v.to_string()))
            })
            .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
        assets::get(&self.store, id.trim_matches('"'))
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn assets_create(&self, args: Value) -> Result<Value, PlatformError> {
        let input: assets::Mutation = parse(args)?;
        let asset = assets::create(&self.store, &input).map_err(map_domain)?;
        self.audit("asset.create", &asset.id)?;
        Ok(json!(asset))
    }

    pub fn assets_update(&self, args: Value) -> Result<Value, PlatformError> {
        let input: assets::Mutation = parse(args)?;
        let asset = assets::update(&self.store, &input).map_err(map_domain)?;
        self.audit("asset.update", &asset.id)?;
        Ok(json!(asset))
    }

    pub fn plan(&self) -> Result<Value, PlatformError> {
        plan::get(&self.store)
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn policies_list(&self, args: Value) -> Result<Value, PlatformError> {
        let input: policy::ListInput = parse(args)?;
        policy::list(&self.store, &input)
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn policies_get(&self, args: Value) -> Result<Value, PlatformError> {
        let id = args
            .get("id")
            .and_then(|v| {
                v.as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| Some(v.to_string()))
            })
            .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
        policy::get(&self.store, id.trim_matches('"'))
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn approvals_list(&self, args: Value) -> Result<Value, PlatformError> {
        let input: approvals::ListInput = parse(args)?;
        let page = approvals::list(&self.store, &input).map_err(map_domain)?;
        Ok(json!(page))
    }

    pub fn approvals_decide(&self, args: Value) -> Result<Value, PlatformError> {
        let input: approvals::DecideInput = parse(args)?;
        let approval = approvals::decide(&self.store, &input).map_err(map_domain)?;
        self.audit("agent.approval.decide", &approval.id)?;
        if input.background {
            let job_id = jobs::enqueue(
                &self.store,
                "agent.resume",
                json!({ "approvalId": approval.id }),
            )?;
            self.audit("agent.job.enqueue", &job_id)?;
            let mut value = json!(approval);
            value["jobId"] = json!(job_id);
            return Ok(value);
        }
        runtime::resume(self, &approval)?;
        Ok(json!(approval))
    }

    pub fn jobs_cancel(&self, args: Value) -> Result<Value, PlatformError> {
        let id = args
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
        let detail = jobs::cancel(&self.store, id)?;
        self.audit("job.cancel", id)?;
        Ok(json!(detail))
    }

    pub fn jobs_get(&self, args: Value) -> Result<Value, PlatformError> {
        let id = args
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
        jobs::get(&self.store, id).map(|detail| json!(detail))
    }

    pub fn audit_list(&self, args: Value) -> Result<Value, PlatformError> {
        let input: audit::ListInput = parse(args)?;
        let page = audit::page(&self.store, &input).map_err(map_domain)?;
        Ok(json!(page))
    }

    pub fn policies_set_provenance(&self, args: Value) -> Result<Value, PlatformError> {
        let input: policy::ProvenanceInput = parse(args)?;
        let updated = policy::set_provenance(&self.store, &input).map_err(map_domain)?;
        self.audit("policy.provenance.set", &updated.id)?;
        Ok(json!(updated))
    }

    pub fn checkin(&self) -> Result<Value, PlatformError> {
        let now = chrono::Utc::now();
        let interval = self
            .store
            .read(|conn| {
                conn.query_row(
                    "SELECT check_in_interval_days FROM dms_config WHERE id=1",
                    [],
                    |r| r.get::<_, i64>(0),
                )
            })
            .unwrap_or(30);
        let checked = now.to_rfc3339();
        let next = (now + chrono::Duration::days(interval)).to_rfc3339();
        self.store
            .write(|conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "INSERT INTO check_in_log (checked_at, status, note) VALUES (?1,'ok','')",
                    [&checked],
                )?;
                tx.execute(
                    "UPDATE dms_config SET last_check_in=?1 WHERE id=1",
                    [&checked],
                )?;
                tx.commit()?;
                Ok(())
            })
            .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))?;
        self.audit("trigger.checkin", "")?;
        Ok(json!({
            "checkedInAt": checked,
            "nextCheckAt": next,
            "status": "ok",
        }))
    }

    pub fn deliver_export(&self, args: Value) -> Result<Value, PlatformError> {
        let kind = args
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| PlatformError::invalid_args("缺少 kind"))?;
        let locale = args
            .get("locale")
            .and_then(Value::as_str)
            .unwrap_or("zh-CN");
        let wait = args.get("wait").and_then(Value::as_bool).unwrap_or(true);
        if wait {
            let result = deliver::export(&self.store, kind, locale)?;
            self.audit("deliver.export", result["jobId"].as_str().unwrap_or(""))?;
            return Ok(result);
        }
        let job_id = deliver::enqueue(&self.store, kind, locale)?;
        self.audit("deliver.export", &job_id)?;
        Ok(json!({
            "jobId": job_id,
            "status": "queued",
            "progress": 0.0,
            "downloadUrl": format!("/api/v1/exports/{job_id}"),
        }))
    }

    pub fn read_export(&self, job_id: &str) -> Result<(String, Vec<u8>), PlatformError> {
        deliver::read_export(&self.store, job_id)
    }

    pub fn runs_get(&self, args: Value) -> Result<Value, PlatformError> {
        let id = args
            .get("id")
            .and_then(|v| {
                v.as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| Some(v.to_string()))
            })
            .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
        let id = id.trim_matches('"');
        runs::get(&self.store, id)
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn runs_start(&self, args: Value) -> Result<Value, PlatformError> {
        runtime::start(self, args)
    }

    pub fn runs_plan(&self, args: Value) -> Result<Value, PlatformError> {
        runtime::plan(self, args)
    }

    pub fn runs_list(&self, args: Value) -> Result<Value, PlatformError> {
        let status = args
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("all")
            .to_string();
        let limit = args.get("limit").and_then(Value::as_i64).unwrap_or(20);
        runs::list(&self.store, &status, limit)
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn security_status(&self) -> Result<Value, PlatformError> {
        let locked = matches!(
            digilegacy_security::session::state(),
            digilegacy_security::VaultState::Locked
        );
        let details = self
            .store
            .read(|conn| {
                let recovery: i64 = conn
                    .query_row(
                        "SELECT CASE WHEN length(coalesce(recovery_seal,'')) > 0 THEN 1 ELSE 0 END
                         FROM security_config WHERE id=1",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                let last_backup: String = conn
                    .query_row(
                        "SELECT coalesce(max(at),'') FROM audit_log WHERE kind LIKE 'backup%'",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or_default();
                Ok((recovery, last_backup))
            })
            .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))?;
        Ok(json!({
            "locked": locked,
            "recoveryConfigured": details.0 != 0,
            "keySource": "passphrase",
            "lastBackupAt": details.1,
        }))
    }

    pub(crate) fn audit(&self, kind: &str, subject: &str) -> Result<(), PlatformError> {
        self.store
            .append_audit(kind, subject, "ok", "")
            .map(|_| ())
            .map_err(|e| PlatformError::new("INTERNAL", format!("审计写入失败: {e}")))
    }
}

fn parse<T: serde::de::DeserializeOwned>(args: Value) -> Result<T, PlatformError> {
    serde_json::from_value(args).map_err(|e| PlatformError::invalid_args(e.to_string()))
}

fn map_domain(error: DomainError) -> PlatformError {
    match error {
        DomainError::NotFound(message) => PlatformError::new("NOT_FOUND", message),
        DomainError::Invalid(message) => PlatformError::new("INVALID_ARGS", message),
        DomainError::Conflict(message) => PlatformError::new("CONFLICT", message),
        DomainError::Store(digilegacy_store::StoreError::State(message)) => {
            PlatformError::new("STORE_NOT_READY", message)
        }
        DomainError::Store(other) => PlatformError::new("INTERNAL", other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> Application {
        Application::new(Arc::new(Store::open_in_memory(None).unwrap()))
    }

    #[test]
    fn read_commands_are_live() {
        let app = app();
        let data = app.dispatch("app.overview", json!({})).unwrap();
        assert_eq!(data["assetsTotal"], 0);
        let data = app.dispatch("plan.get", json!({})).unwrap();
        assert_eq!(data["trigger"]["days"], 30);
        let data = app.dispatch("assets.list", json!({})).unwrap();
        assert_eq!(data["total"], 0);
    }

    #[test]
    fn create_update_get_roundtrip_and_audit() {
        let app = app();
        let created = app
            .dispatch(
                "assets.create",
                json!({
                    "name": "主邮箱", "kind": "account",
                    "platform": "示例邮箱", "value": "120", "note": "家人"
                }),
            )
            .unwrap();
        assert_eq!(created["name"], "主邮箱");
        let id = created["id"].as_str().unwrap().to_string();

        let page = app
            .dispatch("assets.list", json!({ "status": "all", "q": "邮箱" }))
            .unwrap();
        assert_eq!(page["total"], 1);

        let updated = app
            .dispatch(
                "assets.update",
                json!({
                    "id": id, "name": "主邮箱2", "kind": "account",
                    "platform": "示例邮箱", "value": "150", "note": "新备注"
                }),
            )
            .unwrap();
        assert_eq!(updated["value"], "150");

        let fetched = app.dispatch("assets.get", json!({ "id": id })).unwrap();
        assert_eq!(fetched["name"], "主邮箱2");

        let overview = app.dispatch("app.overview", json!({})).unwrap();
        assert_eq!(overview["assetsTotal"], 1);
        assert!(overview["activity"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn policy_list_and_get_are_live() {
        let app = app();
        policy::seed_if_empty(app.store()).unwrap();
        let page = app
            .dispatch("policies.list", json!({ "q": "支付宝" }))
            .unwrap();
        assert!(page["total"].as_i64().unwrap() >= 1);
        let id = page["items"][0]["id"].as_str().unwrap().to_string();
        let one = app.dispatch("policies.get", json!({ "id": id })).unwrap();
        assert!(!one["name"].as_str().unwrap().is_empty());
        assert_eq!(one["provenance"]["status"], "unverified");

        let updated = app
            .dispatch(
                "policies.provenance.set",
                json!({ "id": id, "status": "verified", "sourceUrl": "https://example.com", "note": "测试核对" }),
            )
            .unwrap();
        assert_eq!(updated["provenance"]["status"], "verified");
        assert!(!updated["provenance"]["verifiedAt"]
            .as_str()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn runtime_pauses_for_write_and_resumes_after_approval() {
        let app = app();
        let created = app
            .dispatch(
                "assets.create",
                json!({
                    "name": "主邮箱", "kind": "account",
                    "platform": "示例邮箱", "value": "120", "note": "家人"
                }),
            )
            .unwrap();
        let asset_id = created["id"].as_str().unwrap().to_string();

        let run = app
            .dispatch(
                "agent.runs.start",
                json!({
                    "task": "更新资产估值",
                    "steps": [
                        { "tool": "overview.get", "args": {} },
                        { "tool": "assets.update", "args": {
                            "id": asset_id, "name": "主邮箱", "kind": "account",
                            "platform": "示例邮箱", "value": "180", "note": "家人"
                        }}
                    ]
                }),
            )
            .unwrap();
        assert_eq!(run["status"], "waiting_approval");
        let run_id = run["id"].as_str().unwrap().to_string();
        let approvals = app
            .dispatch("agent.approvals.list", json!({ "status": "pending" }))
            .unwrap();
        assert_eq!(approvals["pending"], 1);
        let approval_id = approvals["items"][0]["id"].as_str().unwrap().to_string();

        let decided = app
            .dispatch(
                "agent.approvals.decide",
                json!({ "id": approval_id, "decision": "approve", "reason": "核对无误" }),
            )
            .unwrap();
        assert_eq!(decided["status"], "approved");

        let detail = app
            .dispatch("agent.runs.get", json!({ "id": run_id }))
            .unwrap();
        assert_eq!(detail["status"], "succeeded");
        assert_eq!(detail["steps"][1]["status"], "done");
        let asset = app
            .dispatch("assets.get", json!({ "id": asset_id }))
            .unwrap();
        assert_eq!(asset["value"], "180");
    }

    #[test]
    fn run_inherits_plan_usage() {
        let app = app();
        let usage = runs::Usage {
            tokens_in: 120,
            tokens_out: 45,
            cost_micros: 330,
            priced: true,
        };
        runs::record_plan(
            app.store(),
            "plan_test",
            "读取概览",
            "mock-model",
            &usage,
            "[]",
        )
        .unwrap();
        let run = app
            .dispatch(
                "agent.runs.start",
                json!({
                    "task": "读取概览",
                    "planId": "plan_test",
                    "steps": [{ "tool": "overview.get", "args": {} }]
                }),
            )
            .unwrap();
        assert_eq!(run["status"], "succeeded");
        assert_eq!(run["tokens"], 165);
        assert_eq!(run["tokensIn"], 120);
        assert_eq!(run["costMicros"], 330);
        assert_eq!(run["usagePriced"], true);
        assert_eq!(run["planId"], "plan_test");
        let list = app.dispatch("agent.runs.list", json!({})).unwrap();
        assert_eq!(list["items"][0]["tokens"], 165);
    }

    #[test]
    fn runtime_rejection_stops_run() {
        let app = app();
        let run = app
            .dispatch(
                "agent.runs.start",
                json!({
                    "task": "创建资产",
                    "steps": [{ "tool": "assets.create", "args": {
                        "name": "新资产", "kind": "other", "platform": "测试", "value": "1", "note": ""
                    }}]
                }),
            )
            .unwrap();
        assert_eq!(run["status"], "waiting_approval");
        let run_id = run["id"].as_str().unwrap().to_string();
        let approvals = app
            .dispatch("agent.approvals.list", json!({ "status": "pending" }))
            .unwrap();
        let approval_id = approvals["items"][0]["id"].as_str().unwrap().to_string();
        app.dispatch(
            "agent.approvals.decide",
            json!({ "id": approval_id, "decision": "reject", "reason": "不需要" }),
        )
        .unwrap();
        let detail = app
            .dispatch("agent.runs.get", json!({ "id": run_id }))
            .unwrap();
        assert_eq!(detail["status"], "rejected");
        assert_eq!(detail["steps"][0]["status"], "rejected");
    }

    #[test]
    fn runtime_budget_rejects_too_many_writes() {
        let app = app();
        let error = app
            .dispatch(
                "agent.runs.start",
                json!({
                    "task": "批量创建",
                    "maxWriteSteps": 1,
                    "steps": [
                        { "tool": "assets.create", "args": { "name": "A", "kind": "other" } },
                        { "tool": "assets.create", "args": { "name": "B", "kind": "other" } }
                    ]
                }),
            )
            .unwrap_err();
        assert_eq!(error.code, "INVALID_ARGS");
        assert!(error.message.contains("预算"));
    }

    #[test]
    fn runtime_cancel_expires_pending_approval() {
        let app = app();
        let run = app
            .dispatch(
                "agent.runs.start",
                json!({
                    "task": "取消写运行",
                    "steps": [{ "tool": "assets.create", "args": {
                        "name": "待取消", "kind": "other", "platform": "", "value": "", "note": ""
                    }}]
                }),
            )
            .unwrap();
        let run_id = run["id"].as_str().unwrap().to_string();
        let cancelled = app
            .dispatch("agent.runs.cancel", json!({ "id": run_id }))
            .unwrap();
        assert_eq!(cancelled["status"], "cancelled");
        assert_eq!(cancelled["steps"][0]["status"], "cancelled");

        let approvals = app
            .dispatch("agent.approvals.list", json!({ "status": "all" }))
            .unwrap();
        let item = approvals["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["runId"] == run_id)
            .unwrap();
        assert_eq!(item["status"], "expired");
        let error = app
            .dispatch(
                "agent.approvals.decide",
                json!({ "id": item["id"], "decision": "approve", "reason": "" }),
            )
            .unwrap_err();
        assert_eq!(error.code, "CONFLICT");
    }

    #[test]
    fn runtime_retry_creates_new_run_and_preserves_old() {
        let app = app();
        let run = app
            .dispatch(
                "agent.runs.start",
                json!({
                    "task": "重试写运行",
                    "steps": [{ "tool": "assets.create", "args": {
                        "name": "重试资产", "kind": "other", "platform": "", "value": "", "note": ""
                    }}]
                }),
            )
            .unwrap();
        let old_id = run["id"].as_str().unwrap().to_string();
        let approvals = app
            .dispatch("agent.approvals.list", json!({ "status": "pending" }))
            .unwrap();
        let approval_id = approvals["items"][0]["id"].as_str().unwrap().to_string();
        app.dispatch(
            "agent.approvals.decide",
            json!({ "id": approval_id, "decision": "reject", "reason": "先不建" }),
        )
        .unwrap();
        let old = app
            .dispatch("agent.runs.get", json!({ "id": old_id }))
            .unwrap();
        assert_eq!(old["status"], "rejected");

        let retried = app
            .dispatch("agent.runs.retry", json!({ "id": old_id }))
            .unwrap();
        assert_ne!(retried["id"], old_id);
        assert_eq!(retried["status"], "waiting_approval");
        let new_id = retried["id"].as_str().unwrap().to_string();
        let approvals = app
            .dispatch("agent.approvals.list", json!({ "status": "pending" }))
            .unwrap();
        let approval_id = approvals["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["runId"] == new_id)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        app.dispatch(
            "agent.approvals.decide",
            json!({ "id": approval_id, "decision": "approve", "reason": "同意重试" }),
        )
        .unwrap();
        let new = app
            .dispatch("agent.runs.get", json!({ "id": new_id }))
            .unwrap();
        assert_eq!(new["status"], "succeeded");
        let old = app
            .dispatch("agent.runs.get", json!({ "id": old_id }))
            .unwrap();
        assert_eq!(old["status"], "rejected");
    }

    #[test]
    fn checkin_updates_trigger() {
        let app = app();
        let data = app.dispatch("trigger.checkin", json!({})).unwrap();
        assert_eq!(data["status"], "ok");
        assert!(data["nextCheckAt"].as_str().unwrap().len() > 10);
    }
}
