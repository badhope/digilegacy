//! DigiLegacy Platform API — M0 skeleton.
//!
//! Only health/command discovery are live. Every declared command is reachable
//! through the same envelope and permission gate, but returns `UNSUPPORTED`
//! until its domain handler lands in M2. The Web console uses a generated mock
//! in M0, so the shell can be exercised without faking writes.

mod generated;

use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use digilegacy_contracts as contracts;
use digilegacy_foundation::{CommandBus, Envelope, Permission, PlatformError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tracing::info;

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

    let app = router();
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

pub fn router() -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/commands", get(commands))
        .route("/api/v1/overview", get(overview))
        .route("/api/v1/invoke", post(invoke))
        .fallback(not_found)
}

async fn health() -> Json<Envelope<Value>> {
    Json(Envelope::ok(json!({
        "status": "ok",
        "mode": "m0",
        "version": env!("CARGO_PKG_VERSION"),
        "contracts": contracts::COMMANDS.len(),
    })))
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

async fn overview() -> impl IntoResponse {
    dispatch("app.overview", json!({}))
}

async fn invoke(Json(request): Json<InvokeRequest>) -> impl IntoResponse {
    dispatch(&request.name, request.args)
}

fn dispatch(name: &str, args: Value) -> (StatusCode, Json<Envelope<Value>>) {
    let spec = match generated::find(name) {
        Some(spec) => spec,
        None => {
            let error = PlatformError::not_found(&format!("命令 {name}"));
            return (StatusCode::NOT_FOUND, Json(Envelope::err(error)));
        }
    };

    let permission = Permission::parse(spec.permission);
    if permission
        .map(|value| !value.allowed_on_http())
        .unwrap_or(true)
    {
        let error = PlatformError::new(
            "FORBIDDEN",
            "该命令不允许通过 Platform API 执行；请在本地 CLI 或桌面端完成",
        );
        return (StatusCode::FORBIDDEN, Json(Envelope::err(error)));
    }

    if permission
        .map(Permission::requires_http_write_token)
        .unwrap_or(false)
    {
        let error = PlatformError::new("AUTH_REQUIRED", "M0 尚未接入写权限 token；写命令保持关闭");
        return (StatusCode::UNAUTHORIZED, Json(Envelope::err(error)));
    }

    let bus = CommandBus::new();
    let envelope = bus.dispatch(name, args);
    let status = match envelope.code.as_str() {
        "OK" => StatusCode::OK,
        "UNSUPPORTED" => StatusCode::NOT_IMPLEMENTED,
        code => contracts::find_error(code)
            .and_then(|spec| StatusCode::from_u16(spec.http).ok())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
    };
    (status, Json(envelope))
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

    #[test]
    fn generated_contract_is_visible_to_http() {
        let spec = contracts::find_command("assets.list").expect("assets.list");
        assert_eq!(spec.http, "GET /api/v1/assets");
        assert_eq!(spec.permission, "read");
    }

    #[test]
    fn unknown_command_maps_to_404() {
        let (status, _) = dispatch("nope.nope", json!({}));
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn write_command_requires_a_token_over_http() {
        let (status, envelope) = dispatch("assets.create", json!({}));
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(envelope.code, "AUTH_REQUIRED");
    }

    #[test]
    fn key_and_network_commands_are_forbidden_over_http() {
        assert_eq!(
            dispatch("connections.test", json!({})).0,
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn known_read_command_is_not_implemented_in_m0() {
        let (status, envelope) = dispatch("assets.list", json!({}));
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(envelope.code, "UNSUPPORTED");
    }
}
