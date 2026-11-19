//! DigiLegacy Platform API — M2 read surface.
//!
//! M1's encrypted Store is now wired in; overview/assets/plan/security/runs
//! are served from real rows. Writes stay behind the HTTP token gate until the
//! auth flow lands. The Web console can switch to live reads without a second
//! backend.

mod generated;

use axum::{
    body::Body,
    extract::{Json, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{delete, get, patch, post},
    Router,
};
use digilegacy_application::{jobs, Application};
use digilegacy_domains::{approvals, assets, audit, discovery, plan, policy};
use digilegacy_foundation::{Envelope, Permission, PlatformError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub app: Arc<Application>,
    pub write_token: Option<String>,
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
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/commands", get(commands))
        .route("/api/v1/overview", get(overview))
        .route("/api/v1/audit", get(audit_list))
        .route("/api/v1/assets", get(assets_list))
        .route("/api/v1/assets/discover", get(assets_discover))
        .route("/api/v1/assets/bulk", post(assets_bulk_create))
        .route("/api/v1/assets/{id}", get(asset_get))
        .route("/api/v1/plan", get(plan_get))
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
        .route("/api/v1/notifications", get(notifications_list))
        .route("/api/v1/notifications/send", post(notifications_send))
        .route("/api/v1/policies", get(policies_list))
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
        .with_state(state)
        .fallback(not_found)
}

async fn health(State(state): State<AppState>) -> Json<Envelope<Value>> {
    Json(Envelope::ok(state.app.health()))
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
        }
    }

    fn write_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );
        headers
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
