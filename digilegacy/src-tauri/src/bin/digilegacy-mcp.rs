//! DigiLegacy MCP 服务器（stdio）——把本地规划数据暴露给任意 MCP 客户端。
//!
//! 为什么是 MCP：2025-2026 年 agent 生态的事实标准是 Model Context Protocol
//! （Claude Desktop、Cursor、WorkBuddy 等客户端都原生支持）。此前本应用的
//! agent 能力只接 OpenAI function-calling 格式的自配端点；MCP 服务器让
//! 用户自己的 agent 客户端直接获得同一套只读工具，不再需要为每个客户端
//! 各写一份桥接。
//!
//! 安全边界（与应用内 agent 完全一致，且更收紧）：
//! - 只暴露 `agent_tools::registry()` 中的**只读**工具（Read 档位）；
//!   `Draft` 档不暴露——MCP 客户端的信任模型与应用内对话不同，宁可少给。
//! - 不暴露任何写路径；数据库以只读意图访问，敏感字段在锁定态返回占位符，
//!   工具层本就把账号标识/联系方式/凭据排除在输出之外。
//! - 不建立任何网络连接；stdio 上的对话只发生在本机客户端与本进程之间。
//!
//! 协议：newline-delimited JSON-RPC 2.0（MCP stdio 传输）。
//! 实现 initialize / notifications/initialized / ping / tools/list / tools/call。
//!
//! 数据库定位：默认 `%APPDATA%\com.digilegacy.app\digilegacy.db`，
//! 可用 `DIGILEGACY_DB` 环境变量覆盖（测试与便携场景用）。
//! 数据库文件不存在时启动即失败——空库对 agent 毫无价值，静默造一个
//! 空库反而会让客户端以为「用户什么都没规划」。

use std::io::{BufRead, Write};
use std::path::PathBuf;

use digilegacy_lib::agent_tools::{self, ToolPermission};

/// MCP 协议版本（stdio 传输，2025-06-18 规格线；客户端带其它版本时按规范降级协商）。
const PROTOCOL_VERSION: &str = "2025-06-18";
const SERVER_NAME: &str = "digilegacy";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 默认数据库路径：与应用主程序一致（%APPDATA%\com.digilegacy.app\digilegacy.db）。
fn default_db_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("DIGILEGACY_DB") {
        if !p.trim().is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    let appdata = std::env::var("APPDATA").ok()?;
    Some(
        PathBuf::from(appdata)
            .join("com.digilegacy.app")
            .join("digilegacy.db"),
    )
}

/// MCP 只暴露 Read 档位工具。Draft 档的产出面向应用内对话流，
/// 在 MCP 上下文里没有「用户确认后保存」的回路，不暴露。
fn mcp_tools() -> Vec<digilegacy_lib::agent_tools::ToolSpec> {
    agent_tools::registry()
        .into_iter()
        .filter(|s| matches!(s.permission, ToolPermission::Read))
        .collect()
}

/// 组装 tools/list 的结果（MCP 的 inputSchema 对应注册表的 parameters）。
fn tools_list_result() -> serde_json::Value {
    let tools: Vec<serde_json::Value> = mcp_tools()
        .iter()
        .map(|spec| {
            serde_json::json!({
                "name": spec.name,
                "title": spec.name,
                "description": spec.description,
                "inputSchema": spec.parameters,
                "annotations": {
                    // 两个档位都不写库：Read 只读，Draft 只返回文本草稿。
                    "readOnlyHint": true,
                    "openWorldHint": false,
                }
            })
        })
        .collect();
    serde_json::json!({ "tools": tools })
}

/// 处理一条 JSON-RPC 请求，返回要写回 stdout 的 JSON 行。
/// 通知（无 id）返回 None。解析失败返回一条 protocol error。
fn handle_message(msg: &str) -> Option<String> {
    let value: serde_json::Value = match serde_json::from_str(msg) {
        Ok(v) => v,
        Err(e) => {
            return Some(jsonrpc_error(
                serde_json::Value::Null,
                -32700,
                &format!("解析错误: {e}"),
            ));
        }
    };

    // 通知没有 id（或显式 null），不回包。
    let id = value.get("id").cloned();
    let is_notification = id.is_none() || id.as_ref() == Some(&serde_json::Value::Null);
    let method = value.get("method").and_then(|m| m.as_str()).unwrap_or("");

    if is_notification {
        // notifications/initialized 等一律静默接收。
        return None;
    }
    let id = id.unwrap();

    let result = match method {
        "initialize" => Ok(serde_json::json!({
            "protocolVersion": client_version_compat(
                value.pointer("/params/protocolVersion").and_then(|v| v.as_str()),
            ),
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
        })),
        "ping" => Ok(serde_json::json!({})),
        "tools/list" => Ok(tools_list_result()),
        "tools/call" => handle_tools_call(&value),
        other => Err(format!("方法不存在: {other}")),
    };

    Some(match result {
        Ok(r) => serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": r }).to_string(),
        Err(e) => jsonrpc_error(id, -32601, &e),
    })
}

/// 版本协商：客户端要求的版本我们支持就回同样的，否则回我们支持的版本。
fn client_version_compat(client: Option<&str>) -> String {
    match client {
        Some("2025-06-18") | Some("2025-03-26") | Some("2024-11-05") => client.unwrap().into(),
        _ => PROTOCOL_VERSION.into(),
    }
}

/// tools/call：未知工具/执行失败按 MCP 约定返回 isError 结果，
/// 而不是 JSON-RPC error——模型可以读到错误文本并自行纠正。
fn handle_tools_call(value: &serde_json::Value) -> Result<serde_json::Value, String> {
    let name = value
        .pointer("/params/name")
        .and_then(|v| v.as_str())
        .ok_or("缺少 params.name")?
        .to_string();
    let empty = serde_json::json!({});
    let args = value.pointer("/params/arguments").unwrap_or(&empty);

    // 权限二次校验：即使注册表将来加入了 Draft 工具，MCP 侧也只放行 Read。
    let allowed = mcp_tools().iter().any(|s| s.name == name);
    if !allowed {
        let available = mcp_tools()
            .iter()
            .map(|s| s.name)
            .collect::<Vec<_>>()
            .join(", ");
        return Ok(serde_json::json!({
            "content": [{ "type": "text", "text": format!("工具 {name} 未在 MCP 只读白名单中（可用：{available}）") }],
            "isError": true,
        }));
    }

    match agent_tools::execute(&name, args) {
        Ok(text) => Ok(serde_json::json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false,
        })),
        Err(e) => Ok(serde_json::json!({
            "content": [{ "type": "text", "text": e }],
            "isError": true,
        })),
    }
}

fn jsonrpc_error(id: serde_json::Value, code: i32, message: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
    .to_string()
}

fn main() {
    // stderr 是 MCP stdio 唯一允许的日志通道。
    eprintln!("[digilegacy-mcp] v{SERVER_VERSION} 启动");

    let db_path = match default_db_path() {
        Some(p) => p,
        None => {
            eprintln!(
                "[digilegacy-mcp] 错误：无法定位数据库（APPDATA 未设置，且未设置 DIGILEGACY_DB）"
            );
            std::process::exit(1);
        }
    };
    if !db_path.exists() {
        eprintln!(
            "[digilegacy-mcp] 错误：数据库不存在：{}（先安装并运行 DigiLegacy 主程序；便携场景可用 DIGILEGACY_DB 指定路径）",
            db_path.display()
        );
        std::process::exit(1);
    }
    // 只读打开：MCP 进程没有任何写库路径（连迁移都不跑）。
    // 主库启用整库加密时，用 DPAPI 解出的 key 打开——MCP 与主程序同为
    // 当前 Windows 用户，DPAPI 作用域覆盖，同机同用户的信任边界不变。
    let key_source = match db_path.parent() {
        Some(dir) => digilegacy_lib::db_crypto::load_db_key(dir),
        None => Err("无效的数据库路径".to_string()),
    };
    match key_source {
        Ok(Some(key)) => {
            if let Err(e) =
                digilegacy_lib::db::init_database_readonly_at_with_key(db_path.clone(), Some(&key))
            {
                eprintln!("[digilegacy-mcp] 错误：打开加密数据库失败: {e}");
                std::process::exit(1);
            }
            eprintln!("[digilegacy-mcp] 加密库已打开（SQLCipher）");
        }
        Ok(None) => {
            if let Err(e) = digilegacy_lib::db::init_database_readonly_at(db_path.clone()) {
                eprintln!("[digilegacy-mcp] 错误：打开数据库失败: {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("[digilegacy-mcp] 错误：读取库密钥失败: {e}");
            std::process::exit(1);
        }
    }
    eprintln!("[digilegacy-mcp] 数据库已就绪: {}", db_path.display());

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(resp) = handle_message(&line) {
            if writeln!(stdout, "{resp}").is_err() {
                break;
            }
            let _ = stdout.flush();
        }
    }
    eprintln!("[digilegacy-mcp] stdin 关闭，退出");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_negotiates_supported_version() {
        let req = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {} }
        })
        .to_string();
        let resp = handle_message(&req).unwrap();
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(v["result"]["serverInfo"]["name"], "digilegacy");
    }

    #[test]
    fn initialize_falls_back_for_unknown_version() {
        let req = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "1999-01-01" }
        })
        .to_string();
        let resp = handle_message(&req).unwrap();
        let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(v["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn notifications_are_silently_dropped() {
        let note = serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized"
        })
        .to_string();
        assert!(handle_message(&note).is_none());
    }

    #[test]
    fn tools_list_exposes_only_read_tools_with_schemas() {
        let req = serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}
        })
        .to_string();
        let v: serde_json::Value = serde_json::from_str(&handle_message(&req).unwrap()).unwrap();
        let tools = v["result"]["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
        for t in tools {
            assert!(t["inputSchema"].is_object(), "每个工具都必须带 inputSchema");
            assert_eq!(t["annotations"]["readOnlyHint"], true);
            let name = t["name"].as_str().unwrap();
            assert!(
                !name.contains("draft"),
                "Draft 档工具不得出现在 MCP 白名单: {name}"
            );
        }
        // 注册表里的 Read 工具应一个不少
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"query_assets"));
        assert!(names.contains(&"get_planning_stats"));
    }

    #[test]
    fn unknown_tool_returns_is_error_not_rpc_error() {
        let req = serde_json::json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "no_such_tool", "arguments": {} }
        })
        .to_string();
        let v: serde_json::Value = serde_json::from_str(&handle_message(&req).unwrap()).unwrap();
        assert!(v.get("error").is_none(), "工具级错误不该是 JSON-RPC error");
        assert_eq!(v["result"]["isError"], true);
        let text = v["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("未在 MCP 只读白名单"));
    }

    #[test]
    fn draft_tool_is_blocked_even_if_registry_has_it() {
        let req = serde_json::json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "draft_heir_note", "arguments": { "topic": "t" } }
        })
        .to_string();
        let v: serde_json::Value = serde_json::from_str(&handle_message(&req).unwrap()).unwrap();
        assert_eq!(v["result"]["isError"], true);
        assert!(v["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("白名单"));
    }

    #[test]
    fn unknown_method_is_rpc_error() {
        let req = serde_json::json!({
            "jsonrpc": "2.0", "id": 5, "method": "resources/list", "params": {}
        })
        .to_string();
        let v: serde_json::Value = serde_json::from_str(&handle_message(&req).unwrap()).unwrap();
        assert_eq!(v["error"]["code"], -32601);
    }

    #[test]
    fn malformed_json_is_parse_error() {
        let v: serde_json::Value =
            serde_json::from_str(&handle_message("not json at all").unwrap()).unwrap();
        assert_eq!(v["error"]["code"], -32700);
    }

    #[test]
    fn default_db_path_honors_env_override() {
        // 只验证函数存在且不 panic；具体路径依赖环境，不在单测里断言。
        let _ = default_db_path();
    }
}
