//! DigiLegacy Platform API — M2 read surface.
//!
//! M1's encrypted Store is now wired in; overview/assets/plan/security/runs
//! are served from real rows. Writes stay behind the HTTP token gate until the
//! auth flow lands. The Web console can switch to live reads without a second
//! backend.

mod generated;

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use digilegacy_application::Application;
use digilegacy_domains::assets;
use digilegacy_foundation::{Envelope, Permission, PlatformError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

type AppState = Arc<Application>;

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
    let app = router(application);
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

pub fn router(store: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/commands", get(commands))
        .route("/api/v1/overview", get(overview))
        .route("/api/v1/assets", get(assets_list))
        .route("/api/v1/assets/{id}", get(asset_get))
        .route("/api/v1/plan", get(plan_get))
        .route("/api/v1/security", get(security_get))
        .route("/api/v1/agent/runs", get(agent_runs))
        .route("/api/v1/invoke", post(invoke))
        .with_state(store)
        .fallback(not_found)
}

async fn health(State(app): State<AppState>) -> Json<Envelope<Value>> {
    Json(Envelope::ok(app.health()))
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

async fn overview(State(app): State<AppState>) -> impl IntoResponse {
    dispatch(&app, "app.overview", json!({}))
}

async fn invoke(
    State(app): State<AppState>,
    Json(request): Json<InvokeRequest>,
) -> impl IntoResponse {
    dispatch(&app, &request.name, request.args)
}

async fn assets_list(
    State(app): State<AppState>,
    Query(input): Query<assets::ListInput>,
) -> impl IntoResponse {
    let (status, envelope) = dispatch(&app, "assets.list", json!(input));
    (status, envelope)
}

async fn asset_get(State(app): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let (status, envelope) = dispatch(&app, "assets.get", json!({ "id": id }));
    (status, envelope)
}

async fn plan_get(State(app): State<AppState>) -> impl IntoResponse {
    dispatch(&app, "plan.get", json!({}))
}

async fn security_get(State(app): State<AppState>) -> impl IntoResponse {
    dispatch(&app, "security.status", json!({}))
}

async fn agent_runs(State(app): State<AppState>, Query(query): Query<Value>) -> impl IntoResponse {
    let (status, envelope) = dispatch(&app, "agent.runs.list", query);
    (status, envelope)
}

fn error_response(error: PlatformError) -> (StatusCode, Json<Envelope<Value>>) {
    let status =
        StatusCode::from_u16(error.http_status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(Envelope::err(error)))
}

fn dispatch(app: &Application, name: &str, args: Value) -> (StatusCode, Json<Envelope<Value>>) {
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
    {
        return error_response(PlatformError::new(
            "AUTH_REQUIRED",
            "写命令需要一个作用域写 token；M2 只开放读接口",
        ));
    }

    let result = app.dispatch(name, args);

    match result {
        Ok(data) => (StatusCode::OK, Json(Envelope::ok(data))),
        Err(error) => error_response(error),
    }
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

    fn test_app() -> Application {
        Application::new(Arc::new(
            digilegacy_store::Store::open_in_memory(None).unwrap(),
        ))
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
        let (status, _) = dispatch(&app, "nope.nope", json!({}));
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn write_command_requires_a_token_over_http() {
        let app = test_app();
        let (status, envelope) = dispatch(&app, "assets.create", json!({}));
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(envelope.code, "AUTH_REQUIRED");
    }

    #[test]
    fn key_and_network_commands_are_forbidden_over_http() {
        let app = test_app();
        assert_eq!(
            dispatch(&app, "connections.test", json!({})).0,
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn read_command_serves_real_data() {
        let app = test_app();
        let (status, envelope) = dispatch(&app, "assets.list", json!({}));
        assert_eq!(status, StatusCode::OK);
        assert!(envelope.success);
        let data = envelope.0.data.clone().unwrap();
        assert_eq!(data["total"], 0);
        assert!(data["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn overview_and_plan_are_live() {
        let app = test_app();
        let (status, envelope) = dispatch(&app, "app.overview", json!({}));
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.0.data.as_ref().unwrap()["assetsTotal"], 0);

        let (status, envelope) = dispatch(&app, "plan.get", json!({}));
        assert_eq!(status, StatusCode::OK);
        assert_eq!(envelope.0.data.as_ref().unwrap()["trigger"]["days"], 30);
    }
}
