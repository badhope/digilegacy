//! DigiLegacy MCP server — read/draft tools over the shared application layer.
//!
//! `tools/call` maps the generated tool manifest to its `human_equivalent`
//! command and returns the real envelope inside MCP content. No fake success:
//! failures set `isError: true` and include the platform error code.

use digilegacy_application::Application;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

const TOOLS_JSON: &str = include_str!("../tools.generated.json");
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

struct Server {
    app: Option<Arc<Application>>,
    load_error: Option<String>,
}

impl Server {
    fn new() -> Self {
        match Application::from_env() {
            Ok(app) => Self {
                app: Some(Arc::new(app)),
                load_error: None,
            },
            Err(error) => Self {
                app: None,
                load_error: Some(error),
            },
        }
    }

    fn tools(&self) -> Vec<Value> {
        serde_json::from_str(TOOLS_JSON).unwrap_or_else(|_| Vec::new())
    }

    fn call(&self, name: &str, arguments: Value) -> Value {
        let Some(app) = &self.app else {
            return tool_content(
                &format!(
                    "平台数据层不可用：{}",
                    self.load_error.as_deref().unwrap_or("unknown")
                ),
                true,
            );
        };
        let tools = self.tools();
        let Some(tool) = tools.iter().find(|tool| tool["name"] == name) else {
            return tool_content(&format!("未知工具：{name}"), true);
        };
        let command = tool["_meta"]["digilegacy/command"]
            .as_str()
            .or_else(|| tool["human_equivalent"].as_str())
            .unwrap_or(name);
        match app.dispatch(command, arguments) {
            Ok(data) => tool_content(
                &serde_json::to_string_pretty(&data).unwrap_or_default(),
                false,
            ),
            Err(error) => tool_content(
                &serde_json::to_string_pretty(&json!({
                    "code": error.code,
                    "message": error.message,
                }))
                .unwrap_or_default(),
                true,
            ),
        }
    }
}

fn main() {
    let server = Server::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                let response = error_response(Value::Null, -32700, "parse error");
                let _ = writeln!(stdout, "{response}");
                let _ = stdout.flush();
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let response = match method {
            "initialize" => success_response(
                id,
                json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "digilegacy-mcp",
                        "version": SERVER_VERSION
                    }
                }),
            ),
            "notifications/initialized" => continue,
            "ping" => success_response(id, json!({})),
            "tools/list" => success_response(id, json!({ "tools": server.tools() })),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(Value::Null);
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                success_response(id, server.call(name, arguments))
            }
            "resources/list" => success_response(id, json!({ "resources": [] })),
            _ => error_response(id, -32601, "method not found"),
        };

        let _ = writeln!(stdout, "{response}");
        let _ = stdout.flush();
    }
}

fn tool_content(text: &str, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error
    })
}

fn success_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}
