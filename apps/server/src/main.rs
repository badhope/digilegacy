//! DigiLegacy Platform API — M2 read surface.
//!
//! M1's encrypted Store is now wired in; overview/assets/plan/security/runs
//! are served from real rows. Writes stay behind the HTTP token gate until the
//! auth flow lands. The Web console can switch to live reads without a second
//! backend.

mod generated;
mod local_models;

use axum::{
    body::Body,
    extract::{Json, Path, Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{delete, get, patch, post, put},
    Router,
};
use digilegacy_application::{jobs, Application};
use digilegacy_domains::{approvals, assets, audit, discovery, plan, policy};
use digilegacy_foundation::{Envelope, Permission, PlatformError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub app: Arc<Application>,
    pub write_token: Option<String>,
    pub metrics: Arc<Metrics>,
}

const LATENCY_BOUNDS_MS: [u64; 12] = [5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000, 30000];

/// Process-local HTTP counters. Deliberately structural only: no paths, asset
/// names, connection labels or credentials are stored here.
#[derive(Debug)]
pub struct Metrics {
    started_at: Instant,
    requests_total: AtomicU64,
    inflight: AtomicU64,
    status_2xx: AtomicU64,
    status_3xx: AtomicU64,
    status_4xx: AtomicU64,
    status_5xx: AtomicU64,
    latency_sum_ms: AtomicU64,
    latency_max_ms: AtomicU64,
    latency_buckets: [AtomicU64; 12],
    latency_overflow: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            requests_total: AtomicU64::new(0),
            inflight: AtomicU64::new(0),
            status_2xx: AtomicU64::new(0),
            status_3xx: AtomicU64::new(0),
            status_4xx: AtomicU64::new(0),
            status_5xx: AtomicU64::new(0),
            latency_sum_ms: AtomicU64::new(0),
            latency_max_ms: AtomicU64::new(0),
            latency_buckets: std::array::from_fn(|_| AtomicU64::new(0)),
            latency_overflow: AtomicU64::new(0),
        }
    }

    fn record(&self, status: u16, elapsed: Duration) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        match status {
            200..=299 => self.status_2xx.fetch_add(1, Ordering::Relaxed),
            300..=399 => self.status_3xx.fetch_add(1, Ordering::Relaxed),
            400..=499 => self.status_4xx.fetch_add(1, Ordering::Relaxed),
            _ => self.status_5xx.fetch_add(1, Ordering::Relaxed),
        };
        let ms = elapsed.as_millis() as u64;
        self.latency_sum_ms.fetch_add(ms, Ordering::Relaxed);
        self.latency_max_ms.fetch_max(ms, Ordering::Relaxed);
        if let Some(index) = LATENCY_BOUNDS_MS.iter().position(|bound| ms <= *bound) {
            self.latency_buckets[index].fetch_add(1, Ordering::Relaxed);
        } else {
            self.latency_overflow.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn percentile(&self, percent: u64) -> u64 {
        let total = self.requests_total.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }
        let target = total.saturating_mul(percent).div_ceil(100);
        let mut seen = 0;
        for (index, bucket) in self.latency_buckets.iter().enumerate() {
            seen += bucket.load(Ordering::Relaxed);
            if seen >= target {
                return LATENCY_BOUNDS_MS[index];
            }
        }
        30000
    }

    fn snapshot(&self, app: &Application) -> Value {
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "uptimeSeconds": self.started_at.elapsed().as_secs(),
            "requests": {
                "total": self.requests_total.load(Ordering::Relaxed),
                "inflight": self.inflight.load(Ordering::Relaxed),
                "status2xx": self.status_2xx.load(Ordering::Relaxed),
                "status3xx": self.status_3xx.load(Ordering::Relaxed),
                "status4xx": self.status_4xx.load(Ordering::Relaxed),
                "status5xx": self.status_5xx.load(Ordering::Relaxed),
            },
            "latencyMs": {
                "p50": self.percentile(50),
                "p95": self.percentile(95),
                "p99": self.percentile(99),
                "max": self.latency_max_ms.load(Ordering::Relaxed),
                "sum": self.latency_sum_ms.load(Ordering::Relaxed),
            },
            "store": {
                "schemaVersion": app.store().schema_version().unwrap_or(-1),
                "tables": app.store().table_count().unwrap_or(-1),
            }
        })
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct InvokeRequest {
    name: String,
    #[serde(default)]
    args: Value,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let application = Arc::new(Application::from_env().expect("platform application"));
    let _ = local_models::Manager::global();
    info!(
        path = %application.store().path().display(),
        schema = application.store().schema_version().unwrap_or(-1),
        tables = application.store().table_count().unwrap_or(-1),
        "store ready"
    );
    jobs::spawn_worker((*application).clone());
    application.spawn_trigger_worker();
    application.spawn_notification_worker();
    let write_token = std::env::var("DIGILEGACY_API_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty());
    if write_token.is_some() {
        info!("Platform API write token enabled");
    } else {
        info!("Platform API write token not configured; write routes stay closed");
    }
    let app = router(AppState {
        app: application,
        write_token,
        metrics: Arc::new(Metrics::new()),
    });
    let addr: SocketAddr = std::env::var("DIGILEGACY_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_string())
        .parse()
        .expect("DIGILEGACY_BIND must be host:port");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind platform api");
    info!("DigiLegacy Platform API listening on {addr}");
    axum::serve(listener, app).await.expect("platform api");
}

pub fn router(state: AppState) -> Router {
    let metrics = state.metrics.clone();
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/metrics", get(metrics_snapshot))
        .route("/api/v1/commands", get(commands))
        .route("/api/v1/overview", get(overview))
        .route("/api/v1/audit", get(audit_list))
        .route("/api/v1/assets", get(assets_list).post(assets_create))
        .route("/api/v1/assets/discover", get(assets_discover))
        .route("/api/v1/assets/bulk", post(assets_bulk_create))
        .route("/api/v1/assets/{id}", get(asset_get).patch(asset_update))
        .route("/api/v1/plan", get(plan_get))
        .route("/api/v1/plan/profile", patch(plan_profile_set))
        .route("/api/v1/plan/heirs", put(plan_heirs_set))
        .route("/api/v1/trigger/checkin", post(trigger_checkin))
        .route("/api/v1/trigger", patch(trigger_configure))
        .route("/api/v1/trigger/evaluate", post(trigger_evaluate))
        .route("/api/v1/trigger/events", get(trigger_events))
        .route(
            "/api/v1/connections",
            get(connections_list).post(connections_upsert),
        )
        .route("/api/v1/connections/test", post(connections_test))
        .route("/api/v1/connections/{id}", delete(connections_delete))
        .route("/api/v1/local-models", get(local_models_list))
        .route(
            "/api/v1/local-models/{id}/download",
            post(local_model_download),
        )
        .route(
            "/api/v1/local-models/{id}/activate",
            post(local_model_activate),
        )
        .route("/api/v1/local-models/stop", post(local_model_stop))
        .route("/api/v1/notifications", get(notifications_list))
        .route("/api/v1/notifications/send", post(notifications_send))
        .route("/api/v1/policies", get(policies_list))
        .route("/api/v1/policies/sync", post(policy_sync))
        .route("/api/v1/policies/candidates", get(policy_candidates))
        .route(
            "/api/v1/policies/candidates/{id}/decide",
            post(policy_candidate_decide),
        )
        .route("/api/v1/policies/{id}", get(policy_get))
        .route(
            "/api/v1/policies/{id}/provenance",
            patch(policy_provenance_set),
        )
        .route("/api/v1/deliverables/export", post(deliver_export))
        .route("/api/v1/exports/{job_id}", get(download_export))
        .route("/api/v1/jobs/{id}", get(job_detail))
        .route("/api/v1/jobs/{id}/cancel", post(job_cancel))
        .route("/api/v1/events", get(events))
        .route("/api/v1/security", get(security_get))
        .route("/api/v1/security/unlock", post(security_unlock))
        .route("/api/v1/security/recover", post(security_recover))
        .route("/api/v1/security/lock", post(security_lock))
        .route("/api/v1/agent/runs", get(agent_runs).post(agent_runs_start))
        .route("/api/v1/agent/usage", get(agent_usage))
        .route("/api/v1/agent/runs/{id}", get(agent_run_detail))
        .route("/api/v1/agent/runs/{id}/cancel", post(agent_run_cancel))
        .route("/api/v1/agent/runs/{id}/retry", post(agent_run_retry))
        .route("/api/v1/agent/plan", post(agent_plan))
        .route("/api/v1/agent/approvals", get(agent_approvals))
        .route(
            "/api/v1/agent/approvals/{id}/decide",
            post(agent_approval_decide),
        )
        .route("/api/v1/invoke", post(invoke))
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(metrics, track_metrics))
        .with_state(state)
}

async fn track_metrics(
    State(metrics): State<Arc<Metrics>>,
    request: Request,
    next: Next,
) -> Response {
    metrics.inflight.fetch_add(1, Ordering::Relaxed);
    let started = Instant::now();
    let response = next.run(request).await;
    metrics.record(response.status().as_u16(), started.elapsed());
    metrics.inflight.fetch_sub(1, Ordering::Relaxed);
    response
}

async fn health(State(state): State<AppState>) -> Json<Envelope<Value>> {
    Json(Envelope::ok(state.app.health()))
}

async fn metrics_snapshot(State(state): State<AppState>) -> Json<Envelope<Value>> {
    Json(Envelope::ok(state.metrics.snapshot(&state.app)))
}

async fn commands() -> Json<Envelope<Value>> {
    let items: Vec<Value> = generated::COMMANDS
        .iter()
        .map(|command| {
            json!({
                "name": command.name,
                "domain": command.domain,
                "permission": command.permission,
                "http": command.http,
                "requiresUnlock": command.requires_unlock,
                "audit": command.audit,
            })
        })
        .collect();
    Json(Envelope::ok(json!(items)))
}

async fn overview(State(state): State<AppState>) -> impl IntoResponse {
    dispatch(&state, "app.overview", json!({}), None)
}

async fn audit_list(
    State(state): State<AppState>,
    Query(input): Query<audit::ListInput>,
) -> impl IntoResponse {
    dispatch(&state, "audit.list", json!(input), None)
}

async fn invoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InvokeRequest>,
) -> impl IntoResponse {
    let mut args = request.args;
    if request.name == "agent.approvals.decide" {
        args["background"] = json!(true);
    }
    if request.name == "deliver.export" {
        args["wait"] = json!(false);
    }
    dispatch(&state, &request.name, args, Some(&headers))
}

async fn assets_list(
    State(state): State<AppState>,
    Query(input): Query<assets::ListInput>,
) -> impl IntoResponse {
    let (status, envelope) = dispatch(&state, "assets.list", json!(input), None);
    (status, envelope)
}

async fn asset_get(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let (status, envelope) = dispatch(&state, "assets.get", json!({ "id": id }), None);
    (status, envelope)
}

async fn assets_discover(
    State(state): State<AppState>,
    Query(input): Query<discovery::DiscoverInput>,
) -> impl IntoResponse {
    let (status, envelope) = dispatch(&state, "assets.discover", json!(input), None);
    (status, envelope)
}

async fn assets_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(args): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "assets.create", args, Some(&headers))
}

async fn asset_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut args): Json<Value>,
) -> impl IntoResponse {
    args["id"] = json!(id);
    dispatch(&state, "assets.update", args, Some(&headers))
}

async fn assets_bulk_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(args): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "assets.bulkCreate", args, Some(&headers))
}

async fn plan_get(State(state): State<AppState>) -> impl IntoResponse {
    dispatch(&state, "plan.get", json!({}), None)
}

async fn plan_profile_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(args): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "plan.profile.set", args, Some(&headers))
}

async fn plan_heirs_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(args): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "plan.heirs.set", args, Some(&headers))
}

async fn trigger_checkin(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    dispatch(&state, "trigger.checkin", json!({}), Some(&headers))
}

async fn trigger_configure(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "trigger.configure", input, Some(&headers))
}

async fn trigger_evaluate(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    dispatch(&state, "trigger.evaluate", json!({}), Some(&headers))
}

async fn local_models_list() -> Json<Envelope<Value>> {
    Json(Envelope::ok(local_models::Manager::global().status()))
}

async fn local_model_download(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !authorized_write(&state, Some(&headers)) {
        return error_response(PlatformError::new(
            "AUTH_REQUIRED",
            "下载本地模型需要 Authorization: Bearer <DIGILEGACY_API_TOKEN>",
        ));
    }
    match local_models::Manager::global().start_download(&id) {
        Ok(data) => (StatusCode::OK, Json(Envelope::ok(data))),
        Err(error) => error_response(PlatformError::new(error.code, error.message)),
    }
}

async fn local_model_activate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if !authorized_write(&state, Some(&headers)) {
        return error_response(PlatformError::new(
            "AUTH_REQUIRED",
            "启用本地模型需要 Authorization: Bearer <DIGILEGACY_API_TOKEN>",
        ))
        .into_response();
    }
    match tokio::task::spawn_blocking(move || local_models::Manager::global().activate(&id)).await {
        Ok(Ok(data)) => (StatusCode::OK, Json(Envelope::ok(data))).into_response(),
        Ok(Err(error)) => {
            error_response(PlatformError::new(error.code, error.message)).into_response()
        }
        Err(error) => {
            error_response(PlatformError::new("INTERNAL", error.to_string())).into_response()
        }
    }
}

async fn local_model_stop(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized_write(&state, Some(&headers)) {
        return error_response(PlatformError::new(
            "AUTH_REQUIRED",
            "停止本地模型需要 Authorization: Bearer <DIGILEGACY_API_TOKEN>",
        ))
        .into_response();
    }
    (
        StatusCode::OK,
        Json(Envelope::ok(local_models::Manager::global().stop())),
    )
        .into_response()
}

async fn notifications_list(
    State(state): State<AppState>,
    Query(query): Query<Value>,
) -> impl IntoResponse {
    dispatch(&state, "notifications.list", query, None)
}

async fn connections_list(
    State(state): State<AppState>,
    Query(query): Query<Value>,
) -> impl IntoResponse {
    dispatch(&state, "connections.list", query, None)
}

async fn connections_upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(args): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "connections.upsert", args, Some(&headers))
}

async fn connections_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    dispatch(
        &state,
        "connections.delete",
        json!({ "id": id }),
        Some(&headers),
    )
}

async fn connections_test(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    let input = body.map(|Json(value)| value).unwrap_or_else(|| json!({}));
    // Probes are blocking; run them off the Tokio runtime.
    match tokio::task::spawn_blocking(move || {
        let (status, Json(envelope)) = dispatch(&state, "connections.test", input, Some(&headers));
        (status, envelope)
    })
    .await
    {
        Ok((status, envelope)) => (status, Json(envelope)).into_response(),
        Err(error) => {
            let envelope =
                Envelope::<Value>::err(PlatformError::new("INTERNAL", error.to_string()));
            (StatusCode::INTERNAL_SERVER_ERROR, Json(envelope)).into_response()
        }
    }
}

async fn notifications_send(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    let input = body.map(|Json(value)| value).unwrap_or_else(|| json!({}));
    // HTTP delivery uses a blocking client; never build/drop it on the Tokio runtime.
    match tokio::task::spawn_blocking(move || {
        let (status, Json(envelope)) =
            dispatch(&state, "notifications.send", input, Some(&headers));
        (status, envelope)
    })
    .await
    {
        Ok((status, envelope)) => (status, Json(envelope)).into_response(),
        Err(error) => {
            error_response(PlatformError::new("INTERNAL", error.to_string())).into_response()
        }
    }
}

async fn trigger_events(
    State(state): State<AppState>,
    Query(input): Query<plan::EventsInput>,
) -> impl IntoResponse {
    dispatch(&state, "trigger.events.list", json!(input), None)
}

async fn policies_list(
    State(state): State<AppState>,
    Query(input): Query<policy::ListInput>,
) -> impl IntoResponse {
    dispatch(&state, "policies.list", json!(input), None)
}

async fn policy_get(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    dispatch(&state, "policies.get", json!({ "id": id }), None)
}

async fn policy_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "policies.sync", input, Some(&headers))
}

#[derive(Debug, Deserialize)]
struct PolicyCandidatesQuery {
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    limit: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

async fn policy_candidates(
    State(state): State<AppState>,
    Query(input): Query<PolicyCandidatesQuery>,
) -> impl IntoResponse {
    let limit = input
        .limit
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok());
    dispatch(
        &state,
        "policies.candidates.list",
        json!({ "cursor": input.cursor, "limit": limit, "status": input.status }),
        None,
    )
}

async fn policy_candidate_decide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut input): Json<Value>,
) -> impl IntoResponse {
    input["id"] = json!(id);
    dispatch(&state, "policies.candidate.decide", input, Some(&headers))
}

#[derive(Debug, Deserialize)]
struct ExportRequest {
    kind: String,
    #[serde(default = "default_locale")]
    locale: String,
}

fn default_locale() -> String {
    "zh-CN".to_string()
}

#[derive(Debug, Deserialize)]
struct EventQuery {
    #[serde(rename = "runId")]
    run_id: Option<i64>,
    #[serde(rename = "jobId")]
    job_id: Option<String>,
}

async fn events(
    State(state): State<AppState>,
    Query(query): Query<EventQuery>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let app = state.app;
    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let stream = futures_util::stream::unfold(
        (app, query, String::new(), ticker),
        |(app, query, last, mut ticker)| async move {
            ticker.tick().await;
            let payload = if let Some(run_id) = query.run_id {
                match app.dispatch("agent.runs.get", json!({ "id": run_id })) {
                    Ok(run) => json!({ "run": run, "at": chrono::Utc::now().to_rfc3339() }),
                    Err(error) => json!({ "error": error.code, "message": error.message }),
                }
            } else if let Some(job_id) = &query.job_id {
                match app.dispatch("jobs.get", json!({ "id": job_id })) {
                    Ok(job) => json!({ "job": job, "at": chrono::Utc::now().to_rfc3339() }),
                    Err(error) => json!({ "error": error.code, "message": error.message }),
                }
            } else {
                json!({ "error": "INVALID_ARGS", "message": "需要 runId 或 jobId" })
            };
            let text = payload.to_string();
            let event = if text == last {
                Event::default().comment("keepalive")
            } else {
                Event::default().event("snapshot").data(text.clone())
            };
            Some((Ok(event), (app, query, text, ticker)))
        },
    );
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn deliver_export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ExportRequest>,
) -> impl IntoResponse {
    dispatch(
        &state,
        "deliver.export",
        json!({ "kind": input.kind, "locale": input.locale, "wait": false }),
        Some(&headers),
    )
}

async fn download_export(State(state): State<AppState>, Path(job_id): Path<String>) -> Response {
    match state.app.read_export(&job_id) {
        Ok((file_name, bytes)) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{file_name}\""),
            )
            .body(Body::from(bytes))
            .unwrap_or_else(|error| {
                error_response(PlatformError::new("INTERNAL", error.to_string())).into_response()
            }),
        Err(error) => error_response(error).into_response(),
    }
}

async fn policy_provenance_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut input): Json<Value>,
) -> impl IntoResponse {
    input["id"] = json!(id);
    dispatch(&state, "policies.provenance.set", input, Some(&headers))
}

async fn security_get(State(state): State<AppState>) -> impl IntoResponse {
    dispatch(&state, "security.status", json!({}), None)
}

async fn security_unlock(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "security.unlock", input, None)
}

async fn security_recover(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "security.recover", input, None)
}

async fn security_lock(State(state): State<AppState>) -> impl IntoResponse {
    dispatch(&state, "security.lock", json!({}), None)
}

async fn agent_run_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    dispatch(&state, "agent.runs.get", json!({ "id": id }), None)
}

async fn agent_plan(State(state): State<AppState>, Json(input): Json<Value>) -> impl IntoResponse {
    dispatch(&state, "agent.runs.plan", input, None)
}

async fn agent_run_cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    dispatch(
        &state,
        "agent.runs.cancel",
        json!({ "id": id }),
        Some(&headers),
    )
}

async fn agent_run_retry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    dispatch(
        &state,
        "agent.runs.retry",
        json!({ "id": id }),
        Some(&headers),
    )
}

async fn agent_runs_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> impl IntoResponse {
    dispatch(&state, "agent.runs.start", input, Some(&headers))
}

async fn agent_approvals(
    State(state): State<AppState>,
    Query(input): Query<approvals::ListInput>,
) -> impl IntoResponse {
    dispatch(&state, "agent.approvals.list", json!(input), None)
}

async fn agent_approval_decide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut input): Json<Value>,
) -> impl IntoResponse {
    input["id"] = json!(id);
    input["background"] = json!(true);
    dispatch(&state, "agent.approvals.decide", input, Some(&headers))
}

async fn job_detail(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    dispatch(&state, "jobs.get", json!({ "id": id }), None)
}

async fn job_cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    dispatch(&state, "jobs.cancel", json!({ "id": id }), Some(&headers))
}

async fn agent_runs(
    State(state): State<AppState>,
    Query(query): Query<Value>,
) -> impl IntoResponse {
    let (status, envelope) = dispatch(&state, "agent.runs.list", query, None);
    (status, envelope)
}

#[derive(Debug, Deserialize)]
struct UsageQuery {
    #[serde(default)]
    days: Option<i64>,
}

async fn agent_usage(
    State(state): State<AppState>,
    Query(query): Query<UsageQuery>,
) -> impl IntoResponse {
    dispatch(
        &state,
        "agent.usage.summary",
        json!({ "days": query.days }),
        None,
    )
}

fn error_response(error: PlatformError) -> (StatusCode, Json<Envelope<Value>>) {
    let status =
        StatusCode::from_u16(error.http_status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(Envelope::err(error)))
}

fn dispatch(
    state: &AppState,
    name: &str,
    args: Value,
    headers: Option<&HeaderMap>,
) -> (StatusCode, Json<Envelope<Value>>) {
    let spec = match generated::find(name) {
        Some(spec) => spec,
        None => return error_response(PlatformError::not_found(&format!("命令 {name}"))),
    };

    let permission = Permission::parse(spec.permission);
    if permission
        .map(|value| !value.allowed_on_http())
        .unwrap_or(true)
    {
        return error_response(PlatformError::new(
            "FORBIDDEN",
            "该命令不允许通过 Platform API 执行；请在本地 CLI 或桌面端完成",
        ));
    }
    if permission
        .map(Permission::requires_http_write_token)
        .unwrap_or(false)
        && !authorized_write(state, headers)
    {
        return error_response(PlatformError::new(
            "AUTH_REQUIRED",
            "写命令需要 Authorization: Bearer <DIGILEGACY_API_TOKEN>",
        ));
    }

    let result = state.app.dispatch(name, args);

    match result {
        Ok(data) => (StatusCode::OK, Json(Envelope::ok(data))),
        Err(error) => error_response(error),
    }
}

fn authorized_write(state: &AppState, headers: Option<&HeaderMap>) -> bool {
    let expected = match state.write_token.as_deref() {
        Some(token) => token.as_bytes(),
        None => return false,
    };
    let provided = headers
        .and_then(|headers| headers.get(header::AUTHORIZATION))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::as_bytes)
        .unwrap_or(&[]);
    if provided.is_empty() || provided.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in provided.iter().zip(expected.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(Envelope::<Value>::err(PlatformError::not_found("路由"))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use digilegacy_contracts as contracts;
    use tower::ServiceExt;

    fn test_app() -> AppState {
        test_app_with_token(None)
    }

    fn test_app_with_token(token: Option<&str>) -> AppState {
        let app = Application::with_config_key(
            Arc::new(digilegacy_store::Store::open_in_memory(None).unwrap()),
            [3u8; 32],
        );
        policy::seed_if_empty(app.store()).unwrap();
        AppState {
            app: Arc::new(app),
            write_token: token.map(str::to_string),
            metrics: Arc::new(Metrics::new()),
        }
    }

    #[tokio::test]
    async fn unknown_route_is_counted_by_middleware() {
        let state = test_app();
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/v1/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let Json(envelope) = metrics_snapshot(State(state)).await;
        assert_eq!(envelope.data.unwrap()["requests"]["total"], 1);
    }

    #[tokio::test]
    async fn metrics_endpoint_exposes_counters_without_secrets() {
        let state = test_app();
        state.metrics.record(200, Duration::from_millis(12));
        let Json(envelope) = metrics_snapshot(State(state)).await;
        assert!(envelope.success);
        let data = envelope.data.as_ref().unwrap();
        assert_eq!(data["requests"]["total"], 1);
        assert_eq!(data["requests"]["status2xx"], 1);
        assert_eq!(data["latencyMs"]["p95"], 25);
        assert_eq!(data["store"]["schemaVersion"], 16);
        let text = data.to_string().to_lowercase();
        assert!(!text.contains("token"));
        assert!(!text.contains("secret"));
        assert!(!text.contains("asset"));
    }

    fn write_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );
        headers
    }

    #[tokio::test]
    async fn plan_write_routes_update_holder_and_heirs() {
        let state = test_app_with_token(Some("test-token"));
        let profile = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/v1/plan/profile")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"name":"林知远"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(profile.status(), StatusCode::OK);

        let heirs = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/plan/heirs")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"heirs":[{"name":"林若岚","role":"配偶","isExecutor":true}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(heirs.status(), StatusCode::OK);

        let plan = router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/plan")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(plan.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(plan.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let plan: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(plan["data"]["holder"]["name"], "林知远");
        assert_eq!(plan["data"]["heirs"][0]["name"], "林若岚");
        assert_eq!(plan["data"]["heirs"][0]["role"], "executor");
    }

    #[tokio::test]
    async fn asset_write_routes_accept_token_and_persist_status() {
        let state = test_app_with_token(Some("test-token"));
        let create = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/assets")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"name":"路由资产","kind":"account"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(create.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let created: Value = serde_json::from_slice(&bytes).unwrap();
        let id = created["data"]["id"].as_str().unwrap().to_string();
        assert_eq!(created["data"]["status"], "active");

        let update = router(state)
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/assets/{id}"))
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"name":"路由资产","kind":"account","status":"archived"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(update.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let updated: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(updated["data"]["status"], "archived");
    }

    #[test]
    fn configured_vault_blocks_reads_until_password_unlocks() {
        let state = test_app();
        let init = state.app.security_init("correct horse battery").unwrap();
        assert_eq!(init["configured"], true);
        let recovery = init["recoveryCode"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(!recovery.is_empty());
        // A fresh server process starts locked even after the CLI bootstraps it.
        digilegacy_security::session::lock();

        let (status, envelope) = dispatch(&state, "assets.list", json!({}), None);
        assert_eq!(status, StatusCode::LOCKED);
        assert_eq!(envelope.code, "VAULT_LOCKED");

        let (status, envelope) = dispatch(
            &state,
            "security.unlock",
            json!({ "password": "wrong password" }),
            None,
        );
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(envelope.code, "INVALID_ARGS");

        let (status, envelope) = dispatch(
            &state,
            "security.unlock",
            json!({ "password": "correct horse battery" }),
            None,
        );
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.data.as_ref().unwrap()["locked"], false);

        let (status, _) = dispatch(&state, "assets.list", json!({}), None);
        assert_eq!(status, StatusCode::OK);

        let (status, envelope) = dispatch(&state, "security.lock", json!({}), None);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.data.as_ref().unwrap()["locked"], true);
        let (status, _) = dispatch(&state, "assets.list", json!({}), None);
        assert_eq!(status, StatusCode::LOCKED);

        let (status, envelope) = dispatch(
            &state,
            "security.recover",
            json!({ "code": recovery }),
            None,
        );
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.data.as_ref().unwrap()["locked"], false);
        let (status, _) = dispatch(&state, "assets.list", json!({}), None);
        assert_eq!(status, StatusCode::OK);
    }

    #[test]
    fn generated_contract_is_visible_to_http() {
        let spec = contracts::find_command("assets.list").expect("assets.list");
        assert_eq!(spec.http, "GET /api/v1/assets");
        assert_eq!(spec.permission, "read");
    }

    #[test]
    fn notifications_list_is_available_over_http() {
        let app = test_app();
        let (status, envelope) = dispatch(&app, "notifications.list", json!({}), None);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.data.as_ref().unwrap()["total"], 0);
    }

    #[test]
    fn unknown_command_maps_to_404() {
        let app = test_app();
        let (status, _) = dispatch(&app, "nope.nope", json!({}), None);
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn write_command_requires_a_token_over_http() {
        let app = test_app();
        let (status, envelope) = dispatch(&app, "assets.create", json!({}), None);
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(envelope.code, "AUTH_REQUIRED");
    }

    #[test]
    fn valid_token_allows_a_scoped_write() {
        let state = test_app_with_token(Some("test-token"));
        let headers = write_headers("test-token");
        let (status, envelope) = dispatch(
            &state,
            "assets.create",
            json!({ "name": "令牌资产", "kind": "account" }),
            Some(&headers),
        );
        assert_eq!(status, StatusCode::OK);
        assert!(envelope.success);
    }

    #[test]
    fn bad_token_is_rejected_for_a_scoped_write() {
        let state = test_app_with_token(Some("test-token"));
        let headers = write_headers("wrong-token");
        let (status, _) = dispatch(
            &state,
            "assets.create",
            json!({ "name": "令牌资产", "kind": "account" }),
            Some(&headers),
        );
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn connection_config_is_writable_only_with_a_token() {
        let state = test_app_with_token(Some("test-token"));
        let headers = write_headers("test-token");
        let (status, envelope) = dispatch(
            &state,
            "connections.upsert",
            json!({
                "kind": "webhook",
                "label": "family",
                "endpoint": "http://127.0.0.1:8791/hook",
                "token": "secret"
            }),
            Some(&headers),
        );
        assert_eq!(status, StatusCode::OK);
        let saved = envelope.data.as_ref().unwrap().clone();
        assert_eq!(saved["source"], "stored");
        assert!(!saved.to_string().contains("secret"));
        let id = saved["id"].as_str().unwrap().to_string();
        let (status, _) = dispatch(
            &state,
            "connections.delete",
            json!({ "id": id }),
            Some(&headers),
        );
        assert_eq!(status, StatusCode::OK);
    }

    #[test]
    fn connections_test_is_writable_but_requires_a_write_token() {
        let app = test_app();
        let (status, _) = dispatch(&app, "connections.test", json!({"kind": "webhook"}), None);
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn read_command_serves_real_data() {
        let app = test_app();
        let (status, envelope) = dispatch(&app, "assets.list", json!({}), None);
        assert_eq!(status, StatusCode::OK);
        assert!(envelope.success);
        let data = envelope.0.data.clone().unwrap();
        assert_eq!(data["total"], 0);
        assert!(data["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn runtime_start_requires_token_and_exposes_run_detail() {
        let state = test_app_with_token(Some("test-token"));
        let (status, _) = dispatch(
            &state,
            "agent.runs.start",
            json!({ "task": "读取概览", "steps": [{ "tool": "app.overview", "args": {} }] }),
            None,
        );
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let headers = write_headers("test-token");
        let (status, envelope) = dispatch(
            &state,
            "agent.runs.start",
            json!({ "task": "读取概览", "steps": [{ "tool": "app.overview", "args": {} }] }),
            Some(&headers),
        );
        assert_eq!(status, StatusCode::OK);
        let data = envelope.0.data.unwrap();
        assert_eq!(data["status"], "succeeded");
        let id = data["id"].as_str().unwrap().to_string();
        let (status, envelope) = dispatch(&state, "agent.runs.get", json!({ "id": id }), None);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.0.data.unwrap()["steps"][0]["status"], "done");
    }

    #[test]
    fn approvals_list_and_decide_transition_once() {
        let state = test_app_with_token(Some("test-token"));
        state
            .app
            .store()
            .write(|conn| {
                conn.execute(
                    "INSERT INTO agent_approvals (id, run_id, tool, object_key, diff_json, status, created_at)
                     VALUES ('appr_1','run_1','assets.update','3','{}','pending','t1')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let (status, envelope) = dispatch(
            &state,
            "agent.approvals.list",
            json!({ "status": "pending" }),
            None,
        );
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.0.data.unwrap()["pending"], 1);

        let headers = write_headers("test-token");
        let (status, envelope) = dispatch(
            &state,
            "agent.approvals.decide",
            json!({ "id": "appr_1", "decision": "approve", "reason": "ok" }),
            Some(&headers),
        );
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.0.data.unwrap()["status"], "approved");

        let (status, _) = dispatch(
            &state,
            "agent.approvals.decide",
            json!({ "id": "appr_1", "decision": "reject" }),
            Some(&headers),
        );
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[test]
    fn audit_list_is_readable_and_paginated() {
        let state = test_app();
        let (status, envelope) = dispatch(&state, "audit.list", json!({ "limit": 2 }), None);
        assert_eq!(status, StatusCode::OK);
        assert!(envelope.success);
        let page = envelope.0.data.unwrap();
        assert!(page["items"].is_array());
        assert!(page["total"].as_i64().unwrap() >= 0);
    }

    #[test]
    fn draft_provenance_write_is_allowed_and_audited() {
        let state = test_app();
        let (status, envelope) = dispatch(
            &state,
            "policies.provenance.set",
            json!({ "id": "1", "status": "verified", "sourceUrl": "https://example.com", "note": "test" }),
            None,
        );
        assert_eq!(status, StatusCode::OK);
        assert!(envelope.success);
        let data = envelope.0.data.clone().unwrap();
        assert_eq!(data["provenance"]["status"], "verified");

        let (_, envelope) = dispatch(&state, "app.overview", json!({}), None);
        let overview = envelope.0.data.unwrap();
        assert!(overview["activity"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["kind"] == "policy.provenance.set"));
    }

    #[test]
    fn policy_sync_requires_a_target_and_decisions_require_a_token() {
        let state = test_app_with_token(Some("test-token"));
        let headers = write_headers("test-token");
        let (status, envelope) = dispatch(&state, "policies.candidates.list", json!({}), None);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.0.data.unwrap()["total"], 0);

        let (status, envelope) = dispatch(&state, "policies.sync", json!({}), Some(&headers));
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(envelope.0.code, "PROVIDER_UNCONFIGURED");

        let (status, _) = dispatch(
            &state,
            "policies.candidate.decide",
            json!({ "id": "1", "action": "approve" }),
            None,
        );
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn overview_and_plan_are_live() {
        let app = test_app();
        let (status, envelope) = dispatch(&app, "app.overview", json!({}), None);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.0.data.as_ref().unwrap()["assetsTotal"], 0);

        let (status, envelope) = dispatch(&app, "plan.get", json!({}), None);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.0.data.as_ref().unwrap()["trigger"]["days"], 30);
    }
}
