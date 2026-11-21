//! Agent harness read model: planner source, run/approval/usage totals,
//! tool catalog exposure, MCP servers and skill registry counts.

use serde_json::{json, Value};

use digilegacy_contracts::TOOLS;
use digilegacy_domains::{connections, skills};
use digilegacy_foundation::PlatformError;

use crate::{map_domain, Application};

const LLM_ENV_NAMES: [&str; 3] = [
    "DIGILEGACY_LLM_BASE_URL",
    "DIGILEGACY_LLM_API_KEY",
    "DIGILEGACY_LLM_MODEL",
];

fn env_set(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
}

fn active_local_model() -> bool {
    let path = std::env::var("DIGILEGACY_LOCAL_MODEL_CONFIG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "data/models/active.json".to_string());
    std::path::Path::new(&path).is_file()
}

fn planner_source(app: &Application) -> Result<&'static str, PlatformError> {
    let page = connections::list(app.store()).map_err(map_domain)?;
    for item in page
        .items
        .iter()
        .filter(|item| item.kind == "llm" && item.enabled && item.source == "stored")
    {
        let config = app.load_connection_config(item)?.unwrap_or_default();
        if !config.endpoint.trim().is_empty()
            && config
                .token
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            && config
                .model
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        {
            return Ok("stored");
        }
    }
    if LLM_ENV_NAMES.iter().all(|name| env_set(name)) {
        return Ok("env");
    }
    if active_local_model() {
        return Ok("local");
    }
    Ok("none")
}

fn count(conn: &rusqlite::Connection, sql: &str) -> rusqlite::Result<i64> {
    conn.query_row(sql, [], |row| row.get(0))
}

pub fn status(app: &Application) -> Result<Value, PlatformError> {
    let source = planner_source(app)?;
    let planner = app.planner_config().ok();
    let endpoint_host = planner
        .as_ref()
        .and_then(|config| reqwest::Url::parse(&config.base_url).ok())
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_default();
    let model = planner
        .as_ref()
        .map(|config| config.model.clone())
        .unwrap_or_default();
    let timeout_secs = planner
        .as_ref()
        .map(|config| config.timeout.as_secs())
        .unwrap_or(0);

    let (runs_total, runs_running, runs_waiting, runs_failed, approvals_pending) =
        app.store()
            .read(|conn| {
                Ok((
                count(conn, "SELECT count(*) FROM agent_run_log")?,
                count(
                    conn,
                    "SELECT count(*) FROM agent_run_log WHERE outcome IN ('running','cancelling')",
                )?,
                count(
                    conn,
                    "SELECT count(*) FROM agent_run_log WHERE outcome='waiting_approval'",
                )?,
                count(conn, "SELECT count(*) FROM agent_run_log WHERE outcome='failed'")?,
                count(
                    conn,
                    "SELECT count(*) FROM agent_approvals WHERE status='pending'",
                )?,
            ))
            })
            .map_err(|error| PlatformError::new("INTERNAL", error.to_string()))?;

    let (tokens_in, tokens_out, cost_micros, plans, unpriced) = app
        .store()
        .read(|conn| {
            Ok((
                count(
                    conn,
                    "SELECT COALESCE(SUM(tokens_in),0) FROM agent_plan_log",
                )?,
                count(
                    conn,
                    "SELECT COALESCE(SUM(tokens_out),0) FROM agent_plan_log",
                )?,
                count(
                    conn,
                    "SELECT COALESCE(SUM(cost_micros),0) FROM agent_plan_log",
                )?,
                count(conn, "SELECT count(*) FROM agent_plan_log")?,
                count(conn, "SELECT count(*) FROM agent_plan_log WHERE priced=0")?,
            ))
        })
        .map_err(|error| PlatformError::new("INTERNAL", error.to_string()))?;

    let connection_page = connections::list(app.store()).map_err(map_domain)?;
    let mcp: Vec<_> = connection_page
        .items
        .iter()
        .filter(|item| item.kind == "mcp")
        .collect();
    let mcp_enabled = mcp.iter().filter(|item| item.enabled).count() as i64;

    let skill_page = skills::list(app.store()).map_err(map_domain)?;
    let skills_enabled = skill_page.iter().filter(|skill| skill.enabled).count() as i64;

    let mcp_tools = TOOLS.iter().filter(|tool| tool.mcp).count();
    let write_tools = TOOLS
        .iter()
        .filter(|tool| tool.permission == "write")
        .count();
    let draft_tools = TOOLS
        .iter()
        .filter(|tool| tool.permission == "draft")
        .count();

    Ok(json!({
        "planner": {
            "configured": planner.is_some(),
            "source": source,
            "model": model,
            "endpointHost": endpoint_host,
            "timeoutSecs": timeout_secs
        },
        "runs": {
            "total": runs_total,
            "running": runs_running,
            "waitingApproval": runs_waiting,
            "failed": runs_failed
        },
        "approvals": { "pending": approvals_pending },
        "usage": {
            "tokensIn": tokens_in,
            "tokensOut": tokens_out,
            "costMicros": cost_micros,
            "priced": plans > 0 && unpriced == 0
        },
        "tools": {
            "total": TOOLS.len() as i64,
            "mcp": mcp_tools as i64,
            "write": write_tools as i64,
            "draft": draft_tools as i64
        },
        "mcp": {
            "servers": mcp.len() as i64,
            "enabled": mcp_enabled
        },
        "skills": {
            "total": skill_page.len() as i64,
            "enabled": skills_enabled,
            "builtin": skill_page.iter().filter(|skill| skill.builtin).count() as i64
        },
        "builtinMcp": crate::builtin_mcp::catalog()
    }))
}
