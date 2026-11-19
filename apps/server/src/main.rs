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
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Router,
};
use digilegacy_application::Application;
use digilegacy_domains::{assets, audit, policy};
use digilegacy_foundation::{Envelope, Permission, PlatformError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
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
        .route("/api/v1/assets/{id}", get(asset_get))
        .route("/api/v1/plan", get(plan_get))
        .route("/api/v1/policies", get(policies_list))
        .route("/api/v1/policies/{id}", get(policy_get))
        .route(
            "/api/v1/policies/{id}/provenance",
            patch(policy_provenance_set),
        )
        .route("/api/v1/deliverables/export", post(deliver_export))
        .route("/api/v1/exports/{job_id}", get(download_export))
        .route("/api/v1/security", get(security_get))
        .route("/api/v1/agent/runs", get(agent_runs))
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
    dispatch(&state, &request.name, request.args, Some(&headers))
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

async fn plan_get(State(state): State<AppState>) -> impl IntoResponse {
    dispatch(&state, "plan.get", json!({}), None)
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

async fn deliver_export(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ExportRequest>,
) -> impl IntoResponse {
    dispatch(
        &state,
        "deliver.export",
        json!({ "kind": input.kind, "locale": input.locale }),
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

async fn agent_runs(
    State(state): State<AppState>,
    Query(query): Query<Value>,
) -> impl IntoResponse {
    let (status, envelope) = dispatch(&state, "agent.runs.list", query, None);
    (status, envelope)
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
        let app = Application::new(Arc::new(
            digilegacy_store::Store::open_in_memory(None).unwrap(),
        ));
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
    fn key_and_network_commands_are_forbidden_over_http() {
        let app = test_app();
        assert_eq!(
            dispatch(&app, "connections.test", json!({}), None).0,
            StatusCode::FORBIDDEN
        );
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
