//! Foundation primitives shared by every DigiLegacy entry point.
//!
//! M0 scope: stable envelope, error mapping, trace ids, command registry.
//! Domain handlers land in M2; until then a known command returns `UNSUPPORTED`.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub type CommandHandler = fn(Value) -> Result<Value, PlatformError>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope<T> {
    pub success: bool,
    pub code: String,
    pub message: String,
    pub data: Option<T>,
    pub details: Option<Value>,
    pub trace_id: String,
}

impl<T> Envelope<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            code: "OK".to_string(),
            message: "ok".to_string(),
            data: Some(data),
            details: None,
            trace_id: trace_id(),
        }
    }

    pub fn err(error: PlatformError) -> Self {
        Self {
            success: false,
            code: error.code,
            message: error.message,
            data: None,
            details: error.details,
            trace_id: trace_id(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformError {
    pub code: String,
    pub message: String,
    pub http_status: u16,
    pub retryable: bool,
    pub details: Option<Value>,
}

impl PlatformError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        let spec = digilegacy_contracts::find_error(code);
        Self {
            code: code.to_string(),
            message: message.into(),
            http_status: spec.map(|item| item.http).unwrap_or(500),
            retryable: spec.map(|item| item.retryable).unwrap_or(true),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn not_found(what: &str) -> Self {
        Self::new("NOT_FOUND", format!("{what}不存在或已删除"))
    }

    pub fn invalid_args(message: impl Into<String>) -> Self {
        Self::new("INVALID_ARGS", message)
    }

    pub fn unsupported(feature: &str) -> Self {
        Self::new("UNSUPPORTED", format!("{feature} 尚未接入平台用例层"))
    }

    pub fn cli_exit_code(&self) -> i32 {
        digilegacy_contracts::find_error(&self.code)
            .map(|item| item.cli_exit)
            .unwrap_or(5)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Read,
    Draft,
    Write,
    Key,
    Network,
    Desktop,
}

impl Permission {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read" => Some(Self::Read),
            "draft" => Some(Self::Draft),
            "write" => Some(Self::Write),
            "key" => Some(Self::Key),
            "network" => Some(Self::Network),
            "desktop" => Some(Self::Desktop),
            _ => None,
        }
    }

    /// HTTP policy: reads/drafts are open; writes need a scoped token;
    /// key/network/desktop commands are local-only.
    pub fn allowed_on_http(self) -> bool {
        matches!(self, Self::Read | Self::Draft | Self::Write)
    }

    pub fn requires_http_write_token(self) -> bool {
        matches!(self, Self::Write)
    }

    pub fn allowed_on_mcp(self) -> bool {
        matches!(self, Self::Read | Self::Draft)
    }

    pub fn allowed_for_agent(self) -> bool {
        matches!(self, Self::Read | Self::Draft | Self::Write)
    }
}

#[derive(Default)]
pub struct CommandBus {
    handlers: HashMap<&'static str, CommandHandler>,
}

impl CommandBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        name: &'static str,
        handler: CommandHandler,
    ) -> Result<(), PlatformError> {
        if digilegacy_contracts::find_command(name).is_none() {
            return Err(PlatformError::not_found(&format!("命令 {name}")));
        }
        self.handlers.insert(name, handler);
        Ok(())
    }

    pub fn registered(&self) -> usize {
        self.handlers.len()
    }

    pub fn dispatch(&self, name: &str, args: Value) -> Envelope<Value> {
        let spec = match digilegacy_contracts::find_command(name) {
            Some(spec) => spec,
            None => return Envelope::err(PlatformError::not_found(&format!("命令 {name}"))),
        };

        match self.handlers.get(name) {
            Some(handler) => match handler(args) {
                Ok(data) => Envelope::ok(data),
                Err(error) => Envelope::err(error),
            },
            None => Envelope::err(PlatformError::unsupported(spec.name)),
        }
    }
}

static TRACE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub fn trace_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = TRACE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("tr_{nanos:x}_{sequence:04x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn echo(args: Value) -> Result<Value, PlatformError> {
        Ok(args)
    }

    #[test]
    fn unknown_command_is_not_found() {
        let bus = CommandBus::new();
        let envelope = bus.dispatch("nope.nope", json!({}));
        assert!(!envelope.success);
        assert_eq!(envelope.code, "NOT_FOUND");
        assert!(envelope.trace_id.starts_with("tr_"));
    }

    #[test]
    fn known_command_without_handler_is_unsupported() {
        let bus = CommandBus::new();
        let envelope = bus.dispatch("app.overview", json!({}));
        assert!(!envelope.success);
        assert_eq!(envelope.code, "UNSUPPORTED");
    }

    #[test]
    fn registered_handler_returns_envelope() {
        let mut bus = CommandBus::new();
        bus.register("app.overview", echo).expect("known command");
        let envelope = bus.dispatch("app.overview", json!({"ok": true}));
        assert!(envelope.success);
        assert_eq!(envelope.code, "OK");
        assert_eq!(envelope.data, Some(json!({"ok": true})));
    }

    #[test]
    fn error_http_status_comes_from_contract() {
        let error = PlatformError::new("VAULT_LOCKED", "locked");
        assert_eq!(error.http_status, 423);
        assert_eq!(error.cli_exit_code(), 3);
        assert!(error.retryable);
    }

    #[test]
    fn http_cannot_run_key_network_or_desktop() {
        assert!(!Permission::Key.allowed_on_http());
        assert!(!Permission::Network.allowed_on_http());
        assert!(!Permission::Desktop.allowed_on_http());
        assert!(Permission::Read.allowed_on_http());
        assert!(Permission::Draft.allowed_on_http());
        assert!(Permission::Write.allowed_on_http());
        assert!(Permission::Write.requires_http_write_token());
    }
}
