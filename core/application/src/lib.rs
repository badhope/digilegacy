//! Application layer: use cases shared by HTTP, CLI and Agent tools.
//!
//! Entry points own transport permissions; this layer owns command semantics.
//! It reads/writes one Store and never duplicates SQL in an app crate.

#![forbid(unsafe_code)]

use digilegacy_domains::{
    approvals, assets, audit, discovery, notify, overview, plan, policy, runs, usage, DomainError,
};
use digilegacy_integrations::{send_smtp, SmtpTarget};
use serde_json::{json, Value};

pub mod builtin_mcp;
pub mod connections_health;
pub mod deliver;
pub mod harness;
pub mod jobs;
pub mod policy_sync;
pub mod runtime;
pub mod skills_admin;
use digilegacy_foundation::PlatformError;
use digilegacy_security::derive_domain_key;
use digilegacy_security::keyring::FileKeyring;
use digilegacy_store::{checkpoint, Store};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

const CONNECTION_CONFIG_DOMAIN: &[u8] = b"digilegacy/connection-config/v1";
const UNLOCK_MAX_FAILURES: u32 = 5;
const UNLOCK_WINDOW: Duration = Duration::from_secs(60);
const UNLOCK_BLOCK: Duration = Duration::from_secs(30);

pub const STATUS: &str = "m2-read-write-core";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NotificationSendResult {
    configured: bool,
    sent: i64,
    failed: i64,
    pending: i64,
    errors: Vec<String>,
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Clone)]
enum NotificationTarget {
    Http {
        channel: String,
        url: String,
        token: Option<String>,
    },
    Smtp(SmtpTarget),
}

fn http_notification_target(channel: &str) -> Option<NotificationTarget> {
    let url = match channel {
        "webhook" => env_value("DIGILEGACY_WEBHOOK_URL"),
        "feishu" => env_value("DIGILEGACY_FEISHU_WEBHOOK_URL"),
        "wecom" => env_value("DIGILEGACY_WECOM_WEBHOOK_URL"),
        _ => None,
    }?;
    let token =
        env_value("DIGILEGACY_WEBHOOK_TOKEN").or_else(|| env_value("DIGILEGACY_NOTIFY_TOKEN"));
    Some(NotificationTarget::Http {
        channel: channel.to_string(),
        url,
        token,
    })
}

fn notification_payload(channel: &str, item: &notify::Notification) -> Value {
    let text = format!("【{}】\n{}", item.subject, item.body);
    match channel {
        "feishu" => json!({
            "msg_type": "text",
            "content": { "text": text },
        }),
        "wecom" => json!({
            "msgtype": "text",
            "text": { "content": text },
        }),
        _ => json!({
            "event": "digilegacy.trigger.final",
            "id": item.id,
            "kind": item.kind,
            "recipient": item.recipient,
            "recipientRole": item.recipient_role,
            "subject": item.subject,
            "body": item.body,
            "createdAt": item.created_at,
            "triggerEventId": item.trigger_event_id,
        }),
    }
}

#[derive(Debug)]
struct UnlockGuard {
    failures: u32,
    window_start: Instant,
    blocked_until: Option<Instant>,
}

impl Default for UnlockGuard {
    fn default() -> Self {
        Self {
            failures: 0,
            window_start: Instant::now(),
            blocked_until: None,
        }
    }
}

impl UnlockGuard {
    fn check(&mut self) -> Result<(), PlatformError> {
        if let Some(until) = self.blocked_until {
            if until > Instant::now() {
                let seconds = until.duration_since(Instant::now()).as_secs().max(1);
                return Err(PlatformError::new(
                    "RATE_LIMITED",
                    format!("尝试次数过多，请在 {seconds} 秒后重试"),
                ));
            }
            self.reset();
        }
        if self.window_start.elapsed() > UNLOCK_WINDOW {
            self.failures = 0;
            self.window_start = Instant::now();
        }
        Ok(())
    }

    fn fail(&mut self) -> bool {
        if self.window_start.elapsed() > UNLOCK_WINDOW {
            self.failures = 0;
            self.window_start = Instant::now();
        }
        self.failures += 1;
        if self.failures >= UNLOCK_MAX_FAILURES {
            self.blocked_until = Some(Instant::now() + UNLOCK_BLOCK);
            return true;
        }
        false
    }

    fn reset(&mut self) {
        self.failures = 0;
        self.window_start = Instant::now();
        self.blocked_until = None;
    }
}

#[derive(Clone)]
pub struct Application {
    store: Arc<Store>,
    config_key: Option<Zeroizing<[u8; 32]>>,
    db_key: Option<Zeroizing<[u8; 32]>>,
    unlock_guard: Arc<Mutex<UnlockGuard>>,
}

impl Application {
    pub fn new(store: Arc<Store>) -> Self {
        Self {
            store,
            config_key: None,
            db_key: None,
            unlock_guard: Arc::new(Mutex::new(UnlockGuard::default())),
        }
    }

    /// Test/support constructor for writing encrypted connection config.
    pub fn with_config_key(store: Arc<Store>, key: [u8; 32]) -> Self {
        Self {
            store,
            config_key: Some(Zeroizing::new(key)),
            db_key: None,
            unlock_guard: Arc::new(Mutex::new(UnlockGuard::default())),
        }
    }

    /// Field-encryption key. Once a vault exists the key is derived from the
    /// unlocked session DEK, never from the database key file.
    pub fn config_key(&self) -> Result<Option<Zeroizing<[u8; 32]>>, PlatformError> {
        if self.vault_configured()? {
            let dek = digilegacy_security::vault::get_key()
                .ok_or_else(|| PlatformError::new("VAULT_LOCKED", "保险库未解锁"))?;
            return derive_domain_key(&dek, CONNECTION_CONFIG_DOMAIN)
                .map(Some)
                .map_err(|error| PlatformError::new("INTERNAL", error));
        }
        Ok(self.config_key.clone())
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
        let db_key = std::env::var("DIGILEGACY_DB_KEY_FILE")
            .ok()
            .map(|path| FileKeyring::new(path).load_or_create())
            .transpose()?;
        let store = match db_key.as_ref() {
            Some(key) => {
                let key_ref: &[u8; 32] = key;
                Store::open(&db_path, Some(key_ref))
            }
            None => Store::open(&db_path, None),
        }
        .map_err(|e| format!("打开数据库失败: {e}"))?;
        let config_key = match std::env::var("DIGILEGACY_CONFIG_KEY_FILE") {
            Ok(path) => Some(FileKeyring::new(path).load_or_create()?),
            Err(_) => db_key
                .as_ref()
                .map(|key| derive_domain_key(key, CONNECTION_CONFIG_DOMAIN))
                .transpose()
                .map_err(|error| format!("派生连接配置密钥失败: {error}"))?,
        };
        let app = Self {
            store: Arc::new(store),
            config_key,
            db_key,
            unlock_guard: Arc::new(Mutex::new(UnlockGuard::default())),
        };
        policy::seed_if_empty(app.store()).map_err(|e| format!("政策库初始化失败: {e}"))?;
        app.seed_environment_connections()?;
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
        // A configured vault is a real console lock: only the security lifecycle
        // commands stay reachable until the process session is unlocked.
        if self.vault_configured()?
            && matches!(
                digilegacy_security::session::state(),
                digilegacy_security::VaultState::Locked
            )
            && !matches!(
                name,
                "security.status" | "security.unlock" | "security.recover" | "security.lock"
            )
        {
            return Err(PlatformError::new(
                "VAULT_LOCKED",
                "保险库已锁定；请先执行 security.unlock",
            ));
        }
        match name {
            "app.overview" => self.overview(),
            "audit.list" => self.audit_list(args),
            "assets.list" => self.assets_list(args),
            "assets.get" => self.assets_get(args),
            "assets.create" => self.assets_create(args),
            "assets.update" => self.assets_update(args),
            "assets.delete" => self.assets_delete(args),
            "assets.discover" => self.assets_discover(args),
            "assets.bulkCreate" => self.assets_bulk_create(args),
            "plan.get" => self.plan(),
            "plan.profile.set" => self.plan_profile_set(args),
            "plan.heirs.set" => self.plan_heirs_set(args),
            "policies.list" => self.policies_list(args),
            "policies.get" => self.policies_get(args),
            "policies.provenance.set" => self.policies_set_provenance(args),
            "policies.sync" => self.policies_sync(args),
            "policies.candidates.list" => self.policies_candidates_list(args),
            "policies.candidate.decide" => self.policies_candidate_decide(args),
            "trigger.checkin" => self.checkin(),
            "trigger.configure" => self.configure_trigger(args),
            "trigger.evaluate" => self.evaluate_trigger(),
            "trigger.events.list" => self.trigger_events(args),
            "connections.list" => self.connections_list(args),
            "connections.upsert" => self.connections_upsert(args),
            "connections.delete" => self.connections_delete(args),
            "connections.test" => self.connections_test(args),
            "notifications.list" => self.notifications_list(args),
            "notifications.send" => self.notifications_send(args),
            "deliver.export" => self.deliver_export(args),
            "diagnostics.export" => self.diagnostics_export(),
            "security.status" => self.security_status(),
            "security.unlock" => self.security_unlock(args),
            "security.recover" => self.security_recover(args),
            "security.lock" => self.security_lock(),
            "agent.runs.list" => self.runs_list(args),
            "agent.usage.summary" => self.usage_summary(args),
            "agent.runs.get" => self.runs_get(args),
            "agent.runs.start" => self.runs_start(args),
            "agent.runs.plan" => runtime::plan(self, args),
            "agent.runs.cancel" => runtime::cancel(self, args),
            "agent.runs.retry" => runtime::retry(self, args),
            "agent.approvals.list" => self.approvals_list(args),
            "agent.approvals.decide" => self.approvals_decide(args),
            "agent.harness.status" => harness::status(self).map(|data| json!(data)),
            "skills.list" => self.skills_list(args),
            "skills.upsert" => self.skills_upsert(args),
            "skills.delete" => self.skills_delete(args),
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

    pub fn plan_profile_set(&self, args: Value) -> Result<Value, PlatformError> {
        let input: plan::ProfileInput = parse(args)?;
        plan::set_profile(&self.store, &input)
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn plan_heirs_set(&self, args: Value) -> Result<Value, PlatformError> {
        let input: plan::HeirsInput = parse(args)?;
        plan::set_heirs(&self.store, &input)
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

    pub fn assets_delete(&self, args: Value) -> Result<Value, PlatformError> {
        let input: assets::DeleteInput = parse(args)?;
        if !input.confirm {
            return Err(PlatformError::new(
                "INVALID_ARGS",
                "物理删除不可撤销，必须传 confirm: true",
            ));
        }
        let result = assets::delete(&self.store, &input.id).map_err(map_domain)?;
        self.audit("asset.delete", &result.id)?;
        Ok(json!(result))
    }

    pub fn assets_discover(&self, args: Value) -> Result<Value, PlatformError> {
        let input: discovery::DiscoverInput = parse(args)?;
        discovery::discover(&self.store, &input)
            .map(|data| json!(data))
            .map_err(map_domain)
    }

    pub fn assets_bulk_create(&self, args: Value) -> Result<Value, PlatformError> {
        let input: assets::BulkCreateInput = parse(args)?;
        let result = assets::bulk_create(&self.store, &input).map_err(map_domain)?;
        self.audit(
            "assets.bulk_create",
            &format!("created={};skipped={}", result.created, result.skipped),
        )?;
        Ok(json!(result))
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
                    "UPDATE dms_config SET last_check_in=?1, triggered_at='', trigger_stage=0 WHERE id=1",
                    [&checked],
                )?;
                tx.execute(
                    "INSERT INTO trigger_events (created_at, kind, stage, detail) VALUES (?1,'checkin',0,'签到，重置触发计时')",
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

    pub fn configure_trigger(&self, args: Value) -> Result<Value, PlatformError> {
        let input: plan::ConfigureInput = parse(args)?;
        let trigger = plan::configure(&self.store, &input).map_err(map_domain)?;
        self.audit(
            "trigger.configure",
            &format!(
                "enabled={};days={};status={}",
                input
                    .enabled
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                input
                    .days
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                trigger.status
            ),
        )?;
        Ok(json!(trigger))
    }

    pub fn evaluate_trigger(&self) -> Result<Value, PlatformError> {
        let trigger = plan::evaluate(&self.store).map_err(map_domain)?;
        self.audit(
            "trigger.evaluate",
            &format!("status={};stage={}", trigger.status, trigger.stage),
        )?;
        Ok(json!(trigger))
    }

    /// Scheduler tick: evaluate silently, audit only when state actually changes.
    pub fn trigger_tick(&self) {
        let before = plan::get(&self.store).map(|plan| plan.trigger);
        let after = plan::evaluate(&self.store);
        if let (Ok(before), Ok(after)) = (before, after) {
            if before.status != after.status || before.stage != after.stage {
                let _ = self.audit(
                    "trigger.evaluate.auto",
                    &format!("status={};stage={}", after.status, after.stage),
                );
            }
        }
    }

    /// Start the 30-minute DMS scheduler; the server owns one process-wide loop.
    pub fn spawn_trigger_worker(&self) {
        let app = self.clone();
        std::thread::spawn(move || loop {
            app.trigger_tick();
            std::thread::sleep(std::time::Duration::from_secs(1800));
        });
    }

    pub fn trigger_events(&self, args: Value) -> Result<Value, PlatformError> {
        let input: plan::EventsInput = parse(args)?;
        plan::events(&self.store, &input)
            .map(|page| json!(page))
            .map_err(map_domain)
    }

    pub fn notifications_list(&self, args: Value) -> Result<Value, PlatformError> {
        let input: notify::ListInput = parse(args)?;
        notify::list(&self.store, &input)
            .map(|page| json!(page))
            .map_err(map_domain)
    }

    fn pending_notification_count(&self) -> i64 {
        notify::list(
            &self.store,
            &notify::ListInput {
                status: Some("pending".to_string()),
                cursor: None,
                limit: Some(1),
            },
        )
        .map(|page| page.total)
        .unwrap_or(0)
    }

    /// Flush pending notifications through the configured channel.
    /// The outbox is always authoritative; a failed send stays visible.
    pub fn notifications_send(&self, args: Value) -> Result<Value, PlatformError> {
        let retried = if args
            .get("retryFailed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            notify::retry_failed(&self.store).map_err(map_domain)?
        } else {
            0
        };
        let result = self.deliver_notifications();
        if result.configured {
            self.audit(
                "notification.send",
                &format!(
                    "sent={};failed={};retried={retried}",
                    result.sent, result.failed
                ),
            )?;
        }
        Ok(json!(result))
    }

    fn notification_target(&self) -> Result<Option<NotificationTarget>, PlatformError> {
        let explicit =
            env_value("DIGILEGACY_NOTIFY_CHANNEL").map(|value| value.to_ascii_lowercase());
        if let Some(channel) = explicit.as_deref() {
            return match channel {
                "smtp" => Ok(self
                    .smtp_notification_target()?
                    .map(NotificationTarget::Smtp)),
                "webhook" | "feishu" | "wecom" => {
                    if let Some((url, token)) = self.saved_http_notification_target(channel)? {
                        return Ok(Some(NotificationTarget::Http {
                            channel: channel.to_string(),
                            url,
                            token,
                        }));
                    }
                    Ok(http_notification_target(channel))
                }
                _ => Ok(None),
            };
        }
        for channel in ["feishu", "wecom", "webhook"] {
            if let Some((url, token)) = self.saved_http_notification_target(channel)? {
                return Ok(Some(NotificationTarget::Http {
                    channel: channel.to_string(),
                    url,
                    token,
                }));
            }
            if let Some(target) = http_notification_target(channel) {
                return Ok(Some(target));
            }
        }
        if let Some(smtp) = self.smtp_notification_target()? {
            return Ok(Some(NotificationTarget::Smtp(smtp)));
        }
        Ok(None)
    }

    fn deliver_notifications(&self) -> NotificationSendResult {
        let pending = match notify::pending(&self.store) {
            Ok(items) => items,
            Err(error) => {
                return NotificationSendResult {
                    configured: false,
                    sent: 0,
                    failed: 0,
                    pending: 0,
                    errors: vec![error.to_string()],
                }
            }
        };
        let target = match self.notification_target() {
            Ok(Some(target)) => target,
            Ok(None) => {
                return NotificationSendResult {
                    configured: false,
                    sent: 0,
                    failed: 0,
                    pending: pending.len() as i64,
                    errors: vec![],
                }
            }
            Err(error) => {
                return NotificationSendResult {
                    configured: true,
                    sent: 0,
                    failed: 0,
                    pending: pending.len() as i64,
                    errors: vec![format!("{}: {}", error.code, error.message)],
                }
            }
        };
        let mut sent = 0;
        let mut failed = 0;
        let mut errors = Vec::new();
        match target {
            NotificationTarget::Http {
                channel,
                url,
                token,
            } => {
                let client = match reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                {
                    Ok(client) => client,
                    Err(error) => {
                        return NotificationSendResult {
                            configured: true,
                            sent: 0,
                            failed: 0,
                            pending: pending.len() as i64,
                            errors: vec![error.to_string()],
                        }
                    }
                };
                for item in pending {
                    let payload = notification_payload(&channel, &item);
                    let mut request = client.post(&url).json(&payload);
                    if let Some(token) = token.as_deref().filter(|value| !value.is_empty()) {
                        request = request.bearer_auth(token);
                    }
                    match request.send() {
                        Ok(response) if response.status().is_success() => {
                            let _ = notify::mark_sent(&self.store, &item.id);
                            sent += 1;
                        }
                        Ok(response) => {
                            let message = format!("HTTP {}", response.status());
                            let _ = notify::mark_failed(&self.store, &item.id, &message);
                            failed += 1;
                            errors.push(format!("{}: {}", item.id, message));
                        }
                        Err(error) => {
                            let _ = notify::mark_failed(&self.store, &item.id, &error.to_string());
                            failed += 1;
                            errors.push(format!("{}: {}", item.id, error));
                        }
                    }
                }
            }
            NotificationTarget::Smtp(smtp) => {
                for item in pending {
                    match send_smtp(&smtp, &item.recipient, &item.subject, &item.body) {
                        Ok(()) => {
                            let _ = notify::mark_sent(&self.store, &item.id);
                            sent += 1;
                        }
                        Err(error) => {
                            let message = error.chars().take(300).collect::<String>();
                            let _ = notify::mark_failed(&self.store, &item.id, &message);
                            failed += 1;
                            errors.push(format!("{}: {}", item.id, message));
                        }
                    }
                }
            }
        }
        let remaining = self.pending_notification_count();
        NotificationSendResult {
            configured: true,
            sent,
            failed,
            pending: remaining,
            errors,
        }
    }

    /// Background tick for the outbox. It does nothing until a channel is set.
    pub fn notification_tick(&self) {
        let result = self.deliver_notifications();
        if result.configured && (result.sent > 0 || result.failed > 0) {
            let _ = self.audit(
                "notification.send.auto",
                &format!("sent={};failed={}", result.sent, result.failed),
            );
        }
    }

    pub fn spawn_notification_worker(&self) {
        let app = self.clone();
        std::thread::spawn(move || loop {
            app.notification_tick();
            std::thread::sleep(std::time::Duration::from_secs(60));
        });
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

    pub fn usage_summary(&self, args: Value) -> Result<Value, PlatformError> {
        let input: usage::SummaryInput = parse(args)?;
        usage::summary(&self.store, input.days.unwrap_or(30))
            .map(|data| json!(data))
            .map_err(map_domain)
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

    /// Create an authenticated checkpoint next to the live database.
    ///
    /// This is a local privileged command: the CLI passes the database key so
    /// the manifest gets an HMAC, and the audit log records `backup.create`.
    pub fn security_checkpoint(&self, label: &str) -> Result<Value, PlatformError> {
        let meta = checkpoint::create_checkpoint(
            &self.store,
            self.checkpoint_root(),
            label,
            false,
            self.db_key.as_deref(),
        )
        .map_err(|error| PlatformError::new("INTERNAL", format!("创建检查点失败: {error}")))?;
        self.audit("backup.create", &meta.id)?;
        Ok(json!(meta))
    }

    pub fn security_checkpoints(&self) -> Result<Value, PlatformError> {
        Ok(json!(checkpoint::list_checkpoints(self.checkpoint_root())))
    }

    pub fn security_verify_checkpoint(&self, id: &str) -> Result<Value, PlatformError> {
        let meta = checkpoint::list_checkpoints(self.checkpoint_root())
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| PlatformError::invalid_args("检查点不存在"))?;
        checkpoint::verify_checkpoint(&meta, self.db_key.as_deref())
            .map_err(|error| PlatformError::new("INTERNAL", format!("校验检查点失败: {error}")))?;
        Ok(json!({
            "verified": true,
            "id": meta.id,
            "schemaVersion": meta.schema_version,
            "createdAt": meta.created_at,
        }))
    }

    fn checkpoint_root(&self) -> &Path {
        self.store
            .path()
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    }

    /// Produce the sanitized diagnostics document used by the audit SLO.
    /// Structural metadata only; no row values or credentials are included.
    pub fn diagnostics_export(&self) -> Result<Value, PlatformError> {
        let text = self
            .store
            .diagnostics_json()
            .map_err(|error| PlatformError::new("INTERNAL", format!("诊断导出失败: {error}")))?;
        let data: Value = serde_json::from_str(&text)
            .map_err(|error| PlatformError::new("INTERNAL", format!("诊断解析失败: {error}")))?;
        self.audit("diagnostics.export", "snapshot")?;
        Ok(data)
    }

    fn vault_config(
        &self,
    ) -> Result<Option<digilegacy_security::session::VaultConfig>, PlatformError> {
        self.store
            .read(|conn| {
                let exists: i64 = conn.query_row(
                    "SELECT count(*) FROM security_config WHERE id=1",
                    [],
                    |r| r.get(0),
                )?;
                if exists == 0 {
                    return Ok(None);
                }
                let (kdf_salt, verifier, wrapped_dek): (String, String, String) = conn.query_row(
                    "SELECT kdf_salt, verifier, coalesce(wrapped_dek,'') FROM security_config WHERE id=1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?;
                Ok(Some(digilegacy_security::session::VaultConfig {
                    kdf_salt,
                    verifier,
                    wrapped_dek,
                }))
            })
            .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))
    }

    fn vault_configured(&self) -> Result<bool, PlatformError> {
        Ok(self.vault_config()?.is_some())
    }

    fn unlock_guard(&self) -> Result<std::sync::MutexGuard<'_, UnlockGuard>, PlatformError> {
        self.unlock_guard
            .lock()
            .map_err(|_| PlatformError::new("INTERNAL", "解锁限速器不可用"))
    }

    /// Bootstrap the vault from the local CLI. This is deliberately absent from
    /// the HTTP/Agent surfaces: only the machine owner can set the first password.
    pub fn security_init(&self, password: &str) -> Result<Value, PlatformError> {
        if password.chars().count() < 8 {
            return Err(PlatformError::new(
                "INVALID_ARGS",
                "主密码至少需要 8 个字符",
            ));
        }
        if self.vault_configured()? {
            return Err(PlatformError::new("CONFLICT", "保险库已初始化"));
        }
        let vault = digilegacy_security::session::create_vault(password)
            .map_err(|e| PlatformError::new("INTERNAL", e))?;
        digilegacy_security::session::unlock(password, &vault.config)
            .map_err(|e| PlatformError::new("INTERNAL", e))?;
        let old_key = self.config_key()?;
        let dek = digilegacy_security::vault::get_key()
            .ok_or_else(|| PlatformError::new("INTERNAL", "保险库未解锁"))?;
        let new_key = derive_domain_key(&dek, CONNECTION_CONFIG_DOMAIN)
            .map_err(|e| PlatformError::new("INTERNAL", e))?;
        let migrated = self.store.write(|conn| {
            let tx = conn.transaction()?;
            migrate_encrypted_rows(&tx, old_key.as_deref(), &new_key)?;
            tx.execute(
                "INSERT INTO security_config (id, kdf_salt, verifier, recovery_seal, created_at, wrapped_dek)
                 VALUES (1,?1,?2,?3,?4,?5)",
                rusqlite::params![
                    vault.config.kdf_salt,
                    vault.config.verifier,
                    vault.recovery_seal,
                    chrono::Utc::now().to_rfc3339(),
                    vault.config.wrapped_dek,
                ],
            )?;
            tx.commit()?;
            Ok(())
        });
        if let Err(error) = migrated {
            digilegacy_security::session::lock();
            return Err(PlatformError::new("INTERNAL", error.to_string()));
        }
        self.audit("security.init", "vault")?;
        Ok(json!({
            "configured": true,
            "recoveryCode": vault.recovery_code,
            "recoveryConfigured": true,
            "keySource": "passphrase",
            "lastBackupAt": "",
        }))
    }

    pub fn security_unlock(&self, args: Value) -> Result<Value, PlatformError> {
        let password = args
            .get("password")
            .and_then(Value::as_str)
            .ok_or_else(|| PlatformError::new("INVALID_ARGS", "缺少主密码"))?;
        let config = self.vault_config()?.ok_or_else(|| {
            PlatformError::new(
                "CONFLICT",
                "保险库尚未初始化；请先在本地 CLI 执行 security init",
            )
        })?;
        let mut guard = self.unlock_guard()?;
        guard.check()?;
        if let Err(error) = digilegacy_security::session::unlock(password, &config) {
            let blocked = guard.fail();
            drop(guard);
            if blocked {
                return Err(PlatformError::new(
                    "RATE_LIMITED",
                    "尝试次数过多，请稍后再试",
                ));
            }
            return Err(PlatformError::new("INVALID_ARGS", error));
        }
        guard.reset();
        drop(guard);
        self.audit("security.unlock", "vault")?;
        self.security_status()
    }

    pub fn security_recover(&self, args: Value) -> Result<Value, PlatformError> {
        let code = args
            .get("code")
            .and_then(Value::as_str)
            .ok_or_else(|| PlatformError::new("INVALID_ARGS", "缺少恢复码"))?;
        let config = self.vault_config()?.ok_or_else(|| {
            PlatformError::new(
                "CONFLICT",
                "保险库尚未初始化；请先在本地 CLI 执行 security init",
            )
        })?;
        let seal: String = self
            .store
            .read(|conn| {
                conn.query_row(
                    "SELECT coalesce(recovery_seal,'') FROM security_config WHERE id=1",
                    [],
                    |row| row.get(0),
                )
            })
            .map_err(|error| PlatformError::new("INTERNAL", error.to_string()))?;
        if seal.trim().is_empty() {
            return Err(PlatformError::new("CONFLICT", "保险库未配置恢复码"));
        }
        let mut guard = self.unlock_guard()?;
        guard.check()?;
        if let Err(error) = digilegacy_security::session::unlock_with_recovery(code, &seal, &config)
        {
            let blocked = guard.fail();
            drop(guard);
            if blocked {
                return Err(PlatformError::new(
                    "RATE_LIMITED",
                    "尝试次数过多，请稍后再试",
                ));
            }
            return Err(PlatformError::new("INVALID_ARGS", error));
        }
        guard.reset();
        drop(guard);
        self.audit("security.recover", "vault")?;
        self.security_status()
    }

    pub fn security_lock(&self) -> Result<Value, PlatformError> {
        digilegacy_security::session::lock();
        self.audit("security.lock", "vault")?;
        self.security_status()
    }

    pub fn security_status(&self) -> Result<Value, PlatformError> {
        let details = self
            .store
            .read(|conn| {
                let configured: i64 =
                    conn.query_row("SELECT count(*) FROM security_config WHERE id=1", [], |r| {
                        r.get(0)
                    })?;
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
                Ok((configured, recovery, last_backup))
            })
            .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))?;
        let configured = details.0 > 0;
        let locked = configured
            && matches!(
                digilegacy_security::session::state(),
                digilegacy_security::VaultState::Locked
            );
        Ok(json!({
            "configured": configured,
            "locked": locked,
            "recoveryConfigured": details.1 != 0,
            "keySource": if configured { "passphrase" } else { "unknown" },
            "lastBackupAt": details.2,
        }))
    }

    pub(crate) fn audit(&self, kind: &str, subject: &str) -> Result<(), PlatformError> {
        self.store
            .append_audit(kind, subject, "ok", "")
            .map(|_| ())
            .map_err(|e| PlatformError::new("INTERNAL", format!("审计写入失败: {e}")))
    }
}

fn migrate_encrypted_rows(
    tx: &rusqlite::Transaction<'_>,
    old_key: Option<&[u8; 32]>,
    new_key: &[u8; 32],
) -> Result<(), digilegacy_store::StoreError> {
    let encrypted_connections: Vec<(i64, String)> = {
        let mut stmt = tx
            .prepare("SELECT id, config_enc FROM connections WHERE config_enc LIKE 'enc:%'")
            .map_err(|e| digilegacy_store::StoreError::State(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| digilegacy_store::StoreError::State(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| digilegacy_store::StoreError::State(e.to_string()))?
    };
    for (id, payload) in encrypted_connections {
        let old = old_key.ok_or_else(|| {
            digilegacy_store::StoreError::State("连接配置已加密但旧密钥不可用".to_string())
        })?;
        let plain = digilegacy_security::decrypt_field_any(&payload, old)
            .map_err(digilegacy_store::StoreError::State)?;
        let encrypted = digilegacy_security::encrypt_field_v2(&plain, new_key)
            .map_err(digilegacy_store::StoreError::State)?;
        tx.execute(
            "UPDATE connections SET config_enc=?1 WHERE id=?2",
            rusqlite::params![encrypted, id],
        )
        .map_err(|e| digilegacy_store::StoreError::State(e.to_string()))?;
    }
    let encrypted_candidates: Vec<(i64, String)> = {
        let mut stmt = tx
            .prepare("SELECT id, payload_enc FROM policy_candidates WHERE payload_enc LIKE 'enc:%'")
            .map_err(|e| digilegacy_store::StoreError::State(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| digilegacy_store::StoreError::State(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| digilegacy_store::StoreError::State(e.to_string()))?
    };
    for (id, payload) in encrypted_candidates {
        let old = old_key.ok_or_else(|| {
            digilegacy_store::StoreError::State("政策候选已加密但旧密钥不可用".to_string())
        })?;
        let plain = digilegacy_security::decrypt_field_any(&payload, old)
            .map_err(digilegacy_store::StoreError::State)?;
        let encrypted = digilegacy_security::encrypt_field_v2(&plain, new_key)
            .map_err(digilegacy_store::StoreError::State)?;
        tx.execute(
            "UPDATE policy_candidates SET payload_enc=?1 WHERE id=?2",
            rusqlite::params![encrypted, id],
        )
        .map_err(|e| digilegacy_store::StoreError::State(e.to_string()))?;
    }
    Ok(())
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
    fn skills_registry_and_harness_status_are_consistent() {
        let app = app();
        let saved = app
            .dispatch(
                "skills.upsert",
                json!({
                    "name": "Research",
                    "description": "Cite sources",
                    "instructions": "Prefer primary sources."
                }),
            )
            .unwrap();
        assert_eq!(saved["name"], "Research");
        assert_eq!(saved["enabled"], true);
        let list = app.dispatch("skills.list", json!({})).unwrap();
        assert_eq!(list["total"], 9, "8 built-in skills plus the user skill");
        let status = app.dispatch("agent.harness.status", json!({})).unwrap();
        assert_eq!(status["skills"]["total"], 9);
        assert_eq!(
            status["skills"]["enabled"], 4,
            "3 core built-ins plus the user skill"
        );
        assert_eq!(status["skills"]["builtin"], 8);
        assert_eq!(status["builtinMcp"].as_array().unwrap().len(), 6);
        assert_eq!(status["tools"]["total"], 23);
        let deleted = app
            .dispatch("skills.delete", json!({ "id": saved["id"] }))
            .unwrap();
        assert_eq!(deleted["name"], "Research");
        assert_eq!(app.dispatch("skills.list", json!({})).unwrap()["total"], 8);
    }

    #[test]
    fn unlock_guard_blocks_after_five_failures() {
        let mut guard = UnlockGuard::default();
        for _ in 0..4 {
            assert!(!guard.fail());
        }
        assert!(guard.fail());
        assert_eq!(guard.check().unwrap_err().code, "RATE_LIMITED");
        guard.reset();
        assert!(guard.check().is_ok());
    }

    #[test]
    fn security_recovery_unlocks_with_one_time_code() {
        let app = Application::new(Arc::new(Store::open_in_memory(None).unwrap()));
        let init = app.security_init("correct horse battery").unwrap();
        let code = init["recoveryCode"].as_str().unwrap().to_string();
        digilegacy_security::session::lock();
        let status = app.security_recover(json!({ "code": code })).unwrap();
        assert_eq!(status["configured"], true);
        assert_eq!(status["locked"], false);
        digilegacy_security::session::lock();
    }

    #[test]
    fn security_init_rekeys_existing_connection_secrets() {
        let app =
            Application::with_config_key(Arc::new(Store::open_in_memory(None).unwrap()), [3u8; 32]);
        let saved = app
            .connections_upsert(json!({
                "kind": "webhook",
                "label": "family",
                "endpoint": "http://127.0.0.1:8791/hook",
                "token": "top-secret-token",
                "enabled": true
            }))
            .unwrap();
        let id = saved["id"].as_str().unwrap().to_string();
        let init = app.security_init("correct horse battery").unwrap();
        assert_eq!(init["configured"], true);
        assert!(!init["recoveryCode"].as_str().unwrap_or_default().is_empty());

        let encrypted = digilegacy_domains::connections::config_enc(app.store(), &id).unwrap();
        let new_key = app.config_key().unwrap().unwrap();
        let plain = digilegacy_security::decrypt_field_any(&encrypted, &new_key).unwrap();
        assert!(plain.contains("top-secret-token"));
        assert!(digilegacy_security::decrypt_field_any(&encrypted, &[3u8; 32]).is_err());

        app.dispatch("security.lock", json!({})).unwrap();
        assert!(app.dispatch("assets.list", json!({})).is_err());
    }

    #[test]
    fn checkpoint_commands_create_list_and_verify() {
        let dir = std::env::temp_dir().join(format!(
            "dgl_ckpt_app_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let key = [7u8; 32];
        let store = Store::open(dir.join("live.db"), Some(&key)).unwrap();
        let app = Application {
            store: Arc::new(store),
            config_key: None,
            db_key: Some(Zeroizing::new(key)),
            unlock_guard: Arc::new(Mutex::new(UnlockGuard::default())),
        };

        let meta = app.security_checkpoint("before-upgrade").unwrap();
        let id = meta["id"].as_str().unwrap().to_string();
        assert_eq!(meta["schema_version"], 19);
        assert_eq!(
            app.security_checkpoints()
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let verified = app.security_verify_checkpoint(&id).unwrap();
        assert_eq!(verified["verified"], true);
        assert!(app.security_verify_checkpoint("missing").is_err());
        let status = app.security_status().unwrap();
        assert!(!status["lastBackupAt"].as_str().unwrap().is_empty());

        drop(app);
        std::fs::remove_dir_all(&dir).unwrap();
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
        let data = app.dispatch("notifications.list", json!({})).unwrap();
        assert_eq!(data["total"], 0);
        assert!(data["items"].as_array().unwrap().is_empty());
        let data = app.dispatch("connections.list", json!({})).unwrap();
        assert_eq!(data["total"], 0);
        assert!(data["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn notification_payloads_match_channel_envelopes() {
        let item = notify::Notification {
            id: "7".to_string(),
            created_at: "2026-11-19T10:00:00+08:00".to_string(),
            trigger_event_id: 3,
            kind: "final".to_string(),
            recipient: "heir@example.com".to_string(),
            recipient_role: "heir".to_string(),
            subject: "遗产交接".to_string(),
            body: "请查收".to_string(),
            status: "pending".to_string(),
            attempts: 0,
            last_error: String::new(),
            sent_at: String::new(),
        };
        let generic = notification_payload("webhook", &item);
        assert_eq!(generic["event"], "digilegacy.trigger.final");
        let feishu = notification_payload("feishu", &item);
        assert_eq!(feishu["msg_type"], "text");
        assert!(feishu["content"]["text"]
            .as_str()
            .unwrap()
            .contains("遗产交接"));
        let wecom = notification_payload("wecom", &item);
        assert_eq!(wecom["msgtype"], "text");
        assert!(wecom["text"]["content"]
            .as_str()
            .unwrap()
            .contains("遗产交接"));
    }

    #[test]
    fn discovery_and_bulk_create_are_idempotent() {
        let app = app();
        let result = app
            .dispatch("assets.discover", json!({ "category": "finance" }))
            .unwrap();
        assert_eq!(result["total"], 6);
        let bulk = app
            .dispatch(
                "assets.bulkCreate",
                json!({ "items": [
                    { "name": "微信零钱", "kind": "account", "platform": "微信" },
                    { "name": "支付宝余额", "kind": "account", "platform": "支付宝" }
                ] }),
            )
            .unwrap();
        assert_eq!(bulk["created"], 2);
        let rerun = app
            .dispatch(
                "assets.bulkCreate",
                json!({ "items": [
                    { "name": "微信零钱", "kind": "account", "platform": "微信" }
                ] }),
            )
            .unwrap();
        assert_eq!(rerun["created"], 0);
        assert_eq!(rerun["skipped"], 1);
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
    fn deep_asset_fields_kind_filter_and_hard_delete() {
        let app = app();
        let created = app
            .dispatch(
                "assets.create",
                json!({
                    "name": "鼓浪屿老宅", "kind": "real_estate",
                    "platform": "不动产登记中心",
                    "valueStatus": "unknown", "note": "待评估",
                    "identifier": "闽(2026)第0001号", "ownershipShare": "50%",
                    "beneficiary": "林小雨", "beneficiaryContact": "lin@example.com",
                    "credentialLocation": "保险柜 A-03", "expiresAt": "2056-12-31",
                    "quantity": "1", "unit": "套"
                }),
            )
            .unwrap();
        let id = created["id"].as_str().unwrap().to_string();
        assert_eq!(created["identifier"], "闽(2026)第0001号");
        assert_eq!(created["ownershipShare"], "50%");
        assert_eq!(created["beneficiary"], "林小雨");
        assert_eq!(created["credentialLocation"], "保险柜 A-03");

        let real_estate = app
            .dispatch(
                "assets.list",
                json!({ "status": "all", "kind": "real_estate" }),
            )
            .unwrap();
        assert_eq!(real_estate["total"], 1);
        let vehicles = app
            .dispatch("assets.list", json!({ "status": "all", "kind": "vehicle" }))
            .unwrap();
        assert_eq!(vehicles["total"], 0);

        let active_delete = app.dispatch("assets.delete", json!({ "id": id, "confirm": true }));
        assert!(active_delete.is_err());

        app.dispatch(
            "assets.update",
            json!({
                "id": id, "name": "鼓浪屿老宅", "kind": "real_estate",
                "platform": "不动产登记中心", "status": "archived"
            }),
        )
        .unwrap();
        let fetched = app.dispatch("assets.get", json!({ "id": id })).unwrap();
        assert_eq!(fetched["status"], "archived");
        assert_eq!(fetched["beneficiary"], "林小雨");
        assert_eq!(fetched["credentialLocation"], "保险柜 A-03");

        let no_confirm = app.dispatch("assets.delete", json!({ "id": id }));
        assert!(no_confirm.is_err());
        let deleted = app
            .dispatch("assets.delete", json!({ "id": id, "confirm": true }))
            .unwrap();
        assert_eq!(deleted["deleted"], true);
        assert!(app.dispatch("assets.get", json!({ "id": id })).is_err());
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
        let mismatch = app
            .dispatch(
                "agent.runs.start",
                json!({
                    "task": "另一个任务",
                    "planId": "plan_test",
                    "steps": [{ "tool": "overview.get", "args": {} }]
                }),
            )
            .unwrap_err();
        assert_eq!(mismatch.code, "INVALID_ARGS");
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
    fn usage_summary_rolls_up_plans_without_double_counting_runs() {
        let app = app();
        let plan_id = "plan_usage_summary";
        let usage = runs::Usage {
            tokens_in: 100,
            tokens_out: 20,
            cost_micros: 140,
            priced: true,
        };
        runs::record_plan(app.store(), plan_id, "用量测试", "mock-model", &usage, "[]").unwrap();
        let run = app
            .dispatch(
                "agent.runs.start",
                json!({
                    "task": "用量测试",
                    "planId": plan_id,
                    "steps": [{ "tool": "overview.get", "args": {} }]
                }),
            )
            .unwrap();
        assert_eq!(run["tokens"], 120);
        let summary = app.dispatch("agent.usage.summary", json!({})).unwrap();
        assert_eq!(summary["totals"]["plans"], 1);
        assert_eq!(summary["totals"]["runs"], 1);
        assert_eq!(summary["totals"]["tokens"], 120);
        assert_eq!(summary["totals"]["costMicros"], 140);
        assert_eq!(summary["byModel"][0]["model"], "mock-model");
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
    fn runtime_cancel_marks_running_as_cancelling() {
        let app = app();
        let steps = json!([
            { "tool": "overview.get", "args": {}, "status": "queued", "result": null, "error": "", "at": "" }
        ]);
        let steps_json = serde_json::to_string(&steps).unwrap();
        let run_id = app
            .store()
            .write(|conn| {
                conn.execute(
                    "INSERT INTO agent_run_log (started_at, task, outcome, steps_json)
                     VALUES (?1, '运行中取消', 'running', ?2)",
                    rusqlite::params![chrono::Utc::now().to_rfc3339(), steps_json],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .unwrap();
        let cancelled = app
            .dispatch("agent.runs.cancel", json!({ "id": run_id.to_string() }))
            .unwrap();
        assert_eq!(cancelled["status"], "cancelling");
        let again = app
            .dispatch("agent.runs.cancel", json!({ "id": run_id.to_string() }))
            .unwrap();
        assert_eq!(again["status"], "cancelling");
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
    fn saved_smtp_connection_delivers_the_outbox() {
        use std::io::{BufRead, BufReader, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
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
                } else {
                    socket.write_all(b"250 OK\r\n").unwrap();
                }
            }
            body
        });
        let app =
            Application::with_config_key(Arc::new(Store::open_in_memory(None).unwrap()), [9u8; 32]);
        app.dispatch(
            "connections.upsert",
            json!({
                "kind": "smtp",
                "label": "outbox",
                "endpoint": addr.to_string(),
                "from": "sender@example.com",
                "security": "none",
                "enabled": true
            }),
        )
        .unwrap();
        app.store()
            .write(|conn| {
                conn.execute(
                    "INSERT INTO notification_outbox (created_at, recipient, subject, body) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![
                        "2026-11-20T00:00:00+00:00",
                        "heir@example.com",
                        "遗产交接",
                        "请查收这份执行清单。"
                    ],
                )?;
                Ok(())
            })
            .unwrap();
        let result = app.notifications_send(json!({})).unwrap();
        assert_eq!(result["configured"], true);
        assert_eq!(result["sent"], 1);
        let body = worker.join().unwrap();
        assert!(body.contains("heir@example.com"));
        assert!(body.contains("Content-Type: text/plain; charset=UTF-8"));
    }

    #[test]
    fn diagnostics_export_reports_valid_chain_without_row_values() {
        let app = app();
        let data = app.dispatch("diagnostics.export", json!({})).unwrap();
        assert_eq!(data["audit"]["valid"], true);
        assert_eq!(data["store"]["schema_version"], 19);
        assert_eq!(data["store"]["table_count"], 32);
        assert!(data["rows"].get("audit_log").is_some());
        let text = data.to_string();
        assert!(!text.contains("secret_value"));
    }

    #[test]
    fn checkin_updates_trigger() {
        let app = app();
        let data = app.dispatch("trigger.checkin", json!({})).unwrap();
        assert_eq!(data["status"], "ok");
        assert!(data["nextCheckAt"].as_str().unwrap().len() > 10);
    }

    #[test]
    fn trigger_lifecycle_configures_evaluates_and_records_events() {
        let app = app();
        let configured = app
            .dispatch(
                "trigger.configure",
                json!({ "enabled": true, "days": 1, "pauseDays": 0 }),
            )
            .unwrap();
        assert_eq!(configured["status"], "armed");

        app.store()
            .write(|conn| {
                conn.execute(
                    "UPDATE dms_config SET last_check_in='2020-01-01T00:00:00+00:00' WHERE id=1",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let evaluated = app.dispatch("trigger.evaluate", json!({})).unwrap();
        assert_eq!(evaluated["status"], "triggered");
        assert_eq!(evaluated["stage"], 1);

        let events = app
            .dispatch("trigger.events.list", json!({ "limit": 20 }))
            .unwrap();
        assert!(events["total"].as_i64().unwrap() >= 2);
        assert!(events["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "stage1"));

        app.dispatch("trigger.checkin", json!({})).unwrap();
        let plan = app.dispatch("plan.get", json!({})).unwrap();
        assert_eq!(plan["trigger"]["status"], "armed");
        assert_eq!(plan["trigger"]["triggeredAt"], "");
    }
}
