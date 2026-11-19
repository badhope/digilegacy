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

pub mod connections_health;
pub mod deliver;
pub mod jobs;
pub mod runtime;
use digilegacy_foundation::PlatformError;
use digilegacy_security::derive_domain_key;
use digilegacy_security::keyring::FileKeyring;
use digilegacy_store::Store;
use std::sync::Arc;
use zeroize::Zeroizing;

const CONNECTION_CONFIG_DOMAIN: &[u8] = b"digilegacy/connection-config/v1";

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

#[derive(Clone)]
pub struct Application {
    store: Arc<Store>,
    config_key: Option<Zeroizing<[u8; 32]>>,
}

impl Application {
    pub fn new(store: Arc<Store>) -> Self {
        Self {
            store,
            config_key: None,
        }
    }

    /// Test/support constructor for writing encrypted connection config.
    pub fn with_config_key(store: Arc<Store>, key: [u8; 32]) -> Self {
        Self {
            store,
            config_key: Some(Zeroizing::new(key)),
        }
    }

    pub fn config_key(&self) -> Option<&[u8; 32]> {
        self.config_key.as_deref()
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
        match name {
            "app.overview" => self.overview(),
            "audit.list" => self.audit_list(args),
            "assets.list" => self.assets_list(args),
            "assets.get" => self.assets_get(args),
            "assets.create" => self.assets_create(args),
            "assets.update" => self.assets_update(args),
            "assets.discover" => self.assets_discover(args),
            "assets.bulkCreate" => self.assets_bulk_create(args),
            "plan.get" => self.plan(),
            "policies.list" => self.policies_list(args),
            "policies.get" => self.policies_get(args),
            "policies.provenance.set" => self.policies_set_provenance(args),
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
            "security.status" => self.security_status(),
            "agent.runs.list" => self.runs_list(args),
            "agent.usage.summary" => self.usage_summary(args),
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
