//! DigiLegacy MCP server — M0 skeleton.
//!
//! MCP exposes only read/draft tools generated from `contracts/tools.json`.
//! `tools/call` is intentionally not connected to a domain handler in M0; it
//! returns an MCP tool error, never a fake success.

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

const TOOLS_JSON: &str = include_str!("../tools.generated.json");
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
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
            "notifications/initialized" => {
                // Notification: no response is required.
                continue;
            }
            "ping" => success_response(id, json!({})),
            "tools/list" => {
                let tools: Value = serde_json::from_str(TOOLS_JSON).unwrap_or_else(|_| json!([]));
                success_response(id, json!({ "tools": tools }))
            }
            "tools/call" => {
                let name = request
                    .get("params")
                    .and_then(|params| params.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                tool_error(id, name)
            }
            "resources/list" => success_response(id, json!({ "resources": [] })),
            _ => error_response(id, -32601, "method not found"),
        };

        let _ = writeln!(stdout, "{response}");
        let _ = stdout.flush();
    }
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

fn tool_error(id: Value, name: &str) -> Value {
    success_response(
        id,
        json!({
            "content": [{
                "type": "text",
                "text": format!("工具 {name} 在 M0 只完成契约暴露；领域服务将在 M2/M5 接入。")
            }],
            "isError": true
        }),
    )
}
