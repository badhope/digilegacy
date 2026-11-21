//! AGT-01: deterministic Agent runtime.
//!
//! A run is an ordered list of registered tool calls. Read/draft tools execute
//! immediately; write tools create a human approval and pause. Approving the
//! approval resumes the same run and executes the exact stored arguments.

use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use digilegacy_contracts::find_command;
use digilegacy_domains::{approvals, runs};
use digilegacy_foundation::PlatformError;

use crate::{map_domain, Application};

const MAX_STEPS: usize = 20;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartInput {
    task: String,
    steps: Vec<StepInput>,
    #[serde(default)]
    max_steps: Option<usize>,
    #[serde(default)]
    max_write_steps: Option<usize>,
    #[serde(default)]
    plan_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInput {
    pub task: String,
    #[serde(default)]
    pub max_steps: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepInput {
    tool: String,
    #[serde(default = "empty_object")]
    args: Value,
}

fn empty_object() -> Value {
    json!({})
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn internal(message: impl Into<String>) -> PlatformError {
    PlatformError::new("INTERNAL", message.into())
}

fn parse_run_id(id: &str) -> Result<i64, PlatformError> {
    id.parse::<i64>()
        .map_err(|_| PlatformError::invalid_args(format!("运行 ID 无效: {id}")))
}

fn tools_used(steps: &[runs::RunStep]) -> String {
    let mut names: Vec<&str> = steps
        .iter()
        .filter(|step| step.status != "queued")
        .map(|step| step.tool.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    names.join(",")
}

fn rounds(steps: &[runs::RunStep]) -> i64 {
    steps.iter().filter(|step| step.status != "queued").count() as i64
}

fn elapsed_ms(app: &Application, run_id: i64) -> i64 {
    app.store()
        .read(|conn| {
            conn.query_row(
                "SELECT started_at FROM agent_run_log WHERE id=?1",
                [run_id],
                |r| r.get::<_, String>(0),
            )
        })
        .ok()
        .and_then(|started| {
            started
                .parse::<chrono::DateTime<chrono::FixedOffset>>()
                .ok()
        })
        .map(|started| {
            (Utc::now() - started.with_timezone(&Utc))
                .num_milliseconds()
                .max(0)
        })
        .unwrap_or(0)
}

fn cancel_requested(app: &Application, run_id: i64) -> bool {
    app.store()
        .read(|conn| {
            conn.query_row(
                "SELECT cancel_requested FROM agent_run_log WHERE id=?1",
                [run_id],
                |row| row.get::<_, i64>(0),
            )
        })
        .map(|value| value != 0)
        .unwrap_or(false)
}

fn request_cancel(app: &Application, run_id: i64) -> Result<(), PlatformError> {
    app.store()
        .write(|conn| {
            conn.execute(
                "UPDATE agent_run_log SET cancel_requested=1, outcome='cancelling' WHERE id=?1
                 AND outcome NOT IN ('succeeded','failed','rejected','cancelled')",
                [run_id],
            )?;
            Ok(())
        })
        .map_err(|e| internal(format!("写入取消请求失败: {e}")))?;
    Ok(())
}

fn expire_approvals(app: &Application, run_id: i64) -> Result<(), PlatformError> {
    app.store()
        .write(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE agent_approvals SET status='expired', decided_at=?2
                 WHERE run_id=?1 AND status='pending'",
                rusqlite::params![run_id.to_string(), now()],
            )?;
            tx.commit()?;
            Ok(())
        })
        .map_err(|e| internal(format!("取消审批失败: {e}")))?;
    Ok(())
}

fn abandon(
    app: &Application,
    run_id: i64,
    steps: &mut [runs::RunStep],
) -> Result<(), PlatformError> {
    for step in steps.iter_mut() {
        if matches!(step.status.as_str(), "queued" | "pending_approval") {
            step.status = "cancelled".to_string();
            step.at = now();
        }
    }
    expire_approvals(app, run_id)?;
    save(app, run_id, "cancelled", steps, "运行已取消")?;
    app.audit("agent.run.cancelled", &run_id.to_string())?;
    Ok(())
}

fn save(
    app: &Application,
    run_id: i64,
    outcome: &str,
    steps: &[runs::RunStep],
    final_text: &str,
) -> Result<bool, PlatformError> {
    let steps_json =
        serde_json::to_string(steps).map_err(|e| internal(format!("步骤序列化失败: {e}")))?;
    let used = tools_used(steps);
    let count = rounds(steps);
    let duration = elapsed_ms(app, run_id);
    let rows = app
        .store()
        .write(|conn| {
            let rows = conn.execute(
                "UPDATE agent_run_log
                 SET outcome=?2, rounds=?3, tools_used=?4, steps_json=?5, final_text=?6, duration_ms=?7
                 WHERE id=?1 AND NOT (cancel_requested=1 AND ?2='succeeded')",
                rusqlite::params![run_id, outcome, count, used, steps_json, final_text, duration],
            )?;
            Ok(rows)
        })
        .map_err(|e| internal(format!("运行记录写入失败: {e}")))?;
    Ok(rows > 0)
}

fn object_key(args: &Value) -> String {
    if let Some(id) = args.get("id") {
        if let Some(text) = id.as_str() {
            return text.to_string();
        }
        return id.to_string();
    }
    if let Some(name) = args.get("name").and_then(Value::as_str) {
        return name.to_string();
    }
    String::new()
}

fn create_approval(
    app: &Application,
    run_id: i64,
    index: usize,
    tool: &str,
    args: &Value,
) -> Result<String, PlatformError> {
    let id = format!("appr_{run_id}_{index}");
    let diff = serde_json::to_string_pretty(args).unwrap_or_else(|_| "{}".to_string());
    let now = now();
    app.store()
        .write(|conn| {
            conn.execute(
                "INSERT INTO agent_approvals
                 (id, run_id, tool, object_key, diff_json, status, reason, created_at, decided_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending', '', ?6, '')",
                rusqlite::params![id, run_id.to_string(), tool, object_key(args), diff, now],
            )?;
            Ok(())
        })
        .map_err(|e| internal(format!("审批记录写入失败: {e}")))?;
    Ok(id)
}

fn create_run(
    app: &Application,
    task: &str,
    steps: &[runs::RunStep],
    plan_id: &str,
    usage: &runs::Usage,
) -> Result<i64, PlatformError> {
    let started = now();
    let steps_json =
        serde_json::to_string(steps).map_err(|e| internal(format!("步骤序列化失败: {e}")))?;
    let run_id = app
        .store()
        .write(|conn| {
            conn.execute(
                "INSERT INTO agent_run_log
                 (started_at, task, outcome, rounds, tools_used, steps_json, final_text,
                  tokens_in, tokens_out, cost_micros, cost_priced, plan_id)
                 VALUES (?1, ?2, 'running', 0, '', ?3, '', ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    started,
                    task,
                    steps_json,
                    usage.tokens_in,
                    usage.tokens_out,
                    usage.cost_micros,
                    if usage.priced { 1 } else { 0 },
                    plan_id
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .map_err(|e| internal(format!("运行记录创建失败: {e}")))?;
    Ok(run_id)
}

fn execute_from(
    app: &Application,
    run_id: i64,
    steps: &mut [runs::RunStep],
    start: usize,
) -> Result<(), PlatformError> {
    for index in start..steps.len() {
        if cancel_requested(app, run_id) {
            abandon(app, run_id, steps)?;
            return Ok(());
        }
        if steps[index].status != "queued" {
            continue;
        }
        let tool = steps[index].tool.clone();
        let args = steps[index].args.clone();
        let command = match digilegacy_contracts::resolve_tool(&tool) {
            Some(command) => command,
            None => {
                steps[index].status = "failed".to_string();
                steps[index].error = format!("未注册工具: {tool}");
                steps[index].at = now();
                save(app, run_id, "failed", steps, &steps[index].error.clone())?;
                return Ok(());
            }
        };
        if command == "agent.runs.start" || command == "agent.approvals.decide" {
            steps[index].status = "failed".to_string();
            steps[index].error = format!("工具不允许在运行内调用: {tool}");
            steps[index].at = now();
            save(app, run_id, "failed", steps, &steps[index].error.clone())?;
            return Ok(());
        }
        let spec = match find_command(command) {
            Some(spec) => spec,
            None => {
                steps[index].status = "failed".to_string();
                steps[index].error = format!("命令未注册: {command}");
                steps[index].at = now();
                save(app, run_id, "failed", steps, &steps[index].error.clone())?;
                return Ok(());
            }
        };

        if spec.permission == "write" {
            let approval_id = create_approval(app, run_id, index, &tool, &args)?;
            if cancel_requested(app, run_id) {
                steps[index].status = "cancelled".to_string();
                steps[index].at = now();
                abandon(app, run_id, steps)?;
                return Ok(());
            }
            steps[index].status = "pending_approval".to_string();
            steps[index].at = now();
            let message = format!("等待审批 {approval_id}: {tool}");
            save(app, run_id, "waiting_approval", steps, &message)?;
            app.audit("agent.run.waiting_approval", &run_id.to_string())?;
            return Ok(());
        }

        match app.dispatch(command, args) {
            Ok(result) => {
                steps[index].status = "done".to_string();
                steps[index].result = Some(result);
                steps[index].error.clear();
                steps[index].at = now();
                if cancel_requested(app, run_id) {
                    abandon(app, run_id, steps)?;
                    return Ok(());
                }
                save(app, run_id, "running", steps, "")?;
            }
            Err(error) => {
                if cancel_requested(app, run_id) {
                    abandon(app, run_id, steps)?;
                    return Ok(());
                }
                steps[index].status = "failed".to_string();
                steps[index].error = format!("{}: {}", error.code, error.message);
                steps[index].at = now();
                save(app, run_id, "failed", steps, &steps[index].error.clone())?;
                return Ok(());
            }
        }
    }
    if cancel_requested(app, run_id) {
        abandon(app, run_id, steps)?;
        return Ok(());
    }
    let done = steps.iter().filter(|step| step.status == "done").count();
    if save(app, run_id, "succeeded", steps, &format!("完成 {done} 步"))? {
        app.audit("agent.run.succeeded", &run_id.to_string())?;
    } else {
        abandon(app, run_id, steps)?;
    }
    Ok(())
}

pub fn start(app: &Application, args: Value) -> Result<Value, PlatformError> {
    let input: StartInput = serde_json::from_value(args)
        .map_err(|e| PlatformError::invalid_args(format!("运行参数无效: {e}")))?;
    if input.task.trim().is_empty() {
        return Err(PlatformError::invalid_args("缺少 task"));
    }
    let max_steps = input.max_steps.unwrap_or(MAX_STEPS).clamp(1, MAX_STEPS);
    if input.steps.is_empty() || input.steps.len() > max_steps {
        return Err(PlatformError::invalid_args(format!(
            "steps 数量必须在 1..={max_steps}"
        )));
    }
    let max_write_steps = input.max_write_steps.unwrap_or(5).clamp(0, MAX_STEPS);
    let write_steps = input
        .steps
        .iter()
        .filter(|step| {
            digilegacy_contracts::resolve_tool(&step.tool)
                .and_then(find_command)
                .map(|spec| spec.permission == "write")
                .unwrap_or(false)
        })
        .count();
    if write_steps > max_write_steps {
        return Err(PlatformError::invalid_args(format!(
            "写操作步骤 {write_steps} 超过预算 {max_write_steps}"
        )));
    }
    for step in &input.steps {
        if step.tool.trim().is_empty() {
            return Err(PlatformError::invalid_args("步骤缺少 tool"));
        }
        if digilegacy_contracts::resolve_tool(&step.tool).is_none() {
            return Err(PlatformError::invalid_args(format!(
                "未注册工具: {}",
                step.tool
            )));
        }
    }

    let steps: Vec<runs::RunStep> = input
        .steps
        .iter()
        .map(|step| runs::RunStep {
            tool: step.tool.clone(),
            args: step.args.clone(),
            status: "queued".to_string(),
            result: None,
            error: String::new(),
            at: String::new(),
        })
        .collect();
    let plan_id = input.plan_id.unwrap_or_default();
    let usage = if plan_id.is_empty() {
        runs::Usage::default()
    } else {
        let record = runs::plan_record(app.store(), &plan_id)
            .ok_or_else(|| PlatformError::invalid_args("计划不存在或已过期"))?;
        if record.task.trim() != input.task.trim() {
            return Err(PlatformError::invalid_args("planId 与运行 task 不匹配"));
        }
        record.usage
    };
    let run_id = create_run(app, input.task.trim(), &steps, &plan_id, &usage)?;
    app.audit("agent.run.start", &run_id.to_string())?;

    let mut steps = steps;
    execute_from(app, run_id, &mut steps, 0)?;
    runs::get(app.store(), &run_id.to_string())
        .map(|detail| json!(detail))
        .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))
}

pub fn plan(app: &Application, args: Value) -> Result<Value, PlatformError> {
    let input: PlanInput = serde_json::from_value(args)
        .map_err(|e| PlatformError::invalid_args(format!("规划参数无效: {e}")))?;
    let max_steps = input.max_steps.unwrap_or(MAX_STEPS).clamp(1, MAX_STEPS);
    let task = input.task.trim().to_string();
    let planner_config = app.planner_config()?;
    let skill_prompt =
        digilegacy_domains::skills::enabled_prompt(app.store(), 12_000).map_err(map_domain)?;
    // The Application is synchronous and is called from async HTTP handlers.
    // Run the blocking planner on a dedicated thread so reqwest does not drop
    // its runtime inside the server's tokio worker.
    let plan_task = task.clone();
    let plan = std::thread::spawn(move || {
        digilegacy_agent::planner::plan_with_config_and_skills(
            &planner_config,
            &plan_task,
            max_steps,
            &skill_prompt,
        )
    })
    .join()
    .map_err(|_| PlatformError::new("INTERNAL", "规划线程异常退出"))?
    .map_err(|error| PlatformError::new(error.code(), error.message()))?;
    let plan_id = format!("plan_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0));
    let steps_json = serde_json::to_string(&plan.steps)
        .map_err(|error| internal(format!("计划序列化失败: {error}")))?;
    let usage = runs::Usage {
        tokens_in: plan.usage.tokens_in,
        tokens_out: plan.usage.tokens_out,
        cost_micros: plan.usage.cost_micros,
        priced: plan.usage.priced,
    };
    runs::record_plan(
        app.store(),
        &plan_id,
        &task,
        &plan.model,
        &usage,
        &steps_json,
    )
    .map_err(|error| internal(format!("计划记录写入失败: {error}")))?;
    app.audit("agent.run.plan", &plan_id)?;
    Ok(json!({
        "planId": plan_id,
        "steps": plan.steps,
        "model": plan.model,
        "usage": usage,
    }))
}

pub fn cancel(app: &Application, args: Value) -> Result<Value, PlatformError> {
    let id = args
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
    let run_id = parse_run_id(id)?;
    let detail =
        runs::get(app.store(), id).map_err(|e| PlatformError::new("NOT_FOUND", e.to_string()))?;
    if detail.status == "cancelling" {
        return Ok(json!(detail));
    }
    if matches!(
        detail.status.as_str(),
        "succeeded" | "failed" | "rejected" | "cancelled"
    ) {
        return Err(PlatformError::new("CONFLICT", "终态运行不能取消"));
    }
    if detail.status == "running" {
        request_cancel(app, run_id)?;
        expire_approvals(app, run_id)?;
        app.audit("agent.run.cancel_requested", &run_id.to_string())?;
        return runs::get(app.store(), id)
            .map(|detail| json!(detail))
            .map_err(|e| PlatformError::new("INTERNAL", e.to_string()));
    }

    request_cancel(app, run_id)?;
    let mut steps = detail.steps;
    for step in &mut steps {
        if matches!(step.status.as_str(), "queued" | "pending_approval") {
            step.status = "cancelled".to_string();
            step.at = now();
        }
    }
    expire_approvals(app, run_id)?;
    save(app, run_id, "cancelled", &steps, "运行已取消")?;
    app.audit("agent.run.cancelled", &run_id.to_string())?;
    runs::get(app.store(), id)
        .map(|detail| json!(detail))
        .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))
}

pub fn retry(app: &Application, args: Value) -> Result<Value, PlatformError> {
    let id = args
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
    let detail =
        runs::get(app.store(), id).map_err(|e| PlatformError::new("NOT_FOUND", e.to_string()))?;
    if !matches!(detail.status.as_str(), "failed" | "rejected" | "cancelled") {
        return Err(PlatformError::new(
            "CONFLICT",
            "只有 failed/rejected/cancelled 运行可以重试",
        ));
    }
    let steps: Vec<runs::RunStep> = detail
        .steps
        .iter()
        .map(|step| runs::RunStep {
            tool: step.tool.clone(),
            args: step.args.clone(),
            status: "queued".to_string(),
            result: None,
            error: String::new(),
            at: String::new(),
        })
        .collect();
    let new_id = create_run(app, &detail.task, &steps, "", &runs::Usage::default())?;
    app.audit("agent.run.retry", &new_id.to_string())?;
    let mut steps = steps;
    execute_from(app, new_id, &mut steps, 0)?;
    runs::get(app.store(), &new_id.to_string())
        .map(|detail| json!(detail))
        .map_err(|e| PlatformError::new("INTERNAL", e.to_string()))
}

pub fn resume(app: &Application, approval: &approvals::Approval) -> Result<(), PlatformError> {
    let run_id = match parse_run_id(&approval.run_id) {
        Ok(id) => id,
        Err(_) => return Ok(()),
    };
    let detail = match runs::get(app.store(), &approval.run_id) {
        Ok(detail) => detail,
        Err(_) => return Ok(()),
    };
    let mut steps = detail.steps;
    let index = steps
        .iter()
        .position(|step| step.status == "pending_approval" && step.tool == approval.tool);
    let index = match index {
        Some(index) => index,
        None => return Ok(()),
    };

    if cancel_requested(app, run_id) {
        steps[index].status = "cancelled".to_string();
        steps[index].at = now();
        abandon(app, run_id, &mut steps)?;
        return Ok(());
    }

    if approval.status == "rejected" {
        steps[index].status = "rejected".to_string();
        steps[index].at = now();
        save(app, run_id, "rejected", &steps, "审批被拒绝")?;
        app.audit("agent.run.rejected", &run_id.to_string())?;
        return Ok(());
    }

    let tool = steps[index].tool.clone();
    let args = steps[index].args.clone();
    let command = match digilegacy_contracts::resolve_tool(&tool) {
        Some(command) => command,
        None => {
            steps[index].status = "failed".to_string();
            steps[index].error = format!("未注册工具: {tool}");
            steps[index].at = now();
            save(app, run_id, "failed", &steps, &steps[index].error.clone())?;
            return Ok(());
        }
    };
    match app.dispatch(command, args) {
        Ok(result) => {
            steps[index].status = "done".to_string();
            steps[index].result = Some(result);
            steps[index].error.clear();
            steps[index].at = now();
            if cancel_requested(app, run_id) {
                abandon(app, run_id, &mut steps)?;
                return Ok(());
            }
            save(app, run_id, "running", &steps, "")?;
            app.audit("agent.run.resume", &run_id.to_string())?;
        }
        Err(error) => {
            steps[index].status = "failed".to_string();
            steps[index].error = format!("{}: {}", error.code, error.message);
            steps[index].at = now();
            save(app, run_id, "failed", &steps, &steps[index].error.clone())?;
            return Ok(());
        }
    }

    execute_from(app, run_id, &mut steps, index + 1)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use digilegacy_store::Store;

    fn app() -> Application {
        Application::new(Arc::new(Store::open_in_memory(None).unwrap()))
    }

    #[test]
    fn plan_uses_saved_llm_connection_model() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let read = stream.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                let split = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|index| index + 4);
                if let Some(header_end) = split {
                    let headers = String::from_utf8_lossy(&request[..header_end]).to_string();
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= header_end + content_length {
                        break;
                    }
                }
            }
            let split = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| index + 4)
                .unwrap();
            let body: Value = serde_json::from_slice(&request[split..]).unwrap();
            assert_eq!(body["model"], "agnes-3.0-flash");
            let system_prompt = body["messages"][0]["content"].as_str().unwrap();
            assert!(system_prompt.contains("Prefer primary sources."));
            assert!(system_prompt.contains("数字资产盘点"));
            assert!(system_prompt.contains("执行人交付手册"));
            assert!(system_prompt.contains("平台遗产政策核对"));
            let content = json!({
                "steps": [{ "tool": "overview.get", "args": {} }]
            })
            .to_string();
            let response_body = json!({
                "choices": [{ "message": { "content": content } }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        let app =
            Application::with_config_key(Arc::new(Store::open_in_memory(None).unwrap()), [9u8; 32]);
        app.connections_upsert(json!({
            "kind": "llm",
            "label": "Agnes",
            "endpoint": format!("http://{address}/v1"),
            "token": "provider-secret",
            "model": "agnes-3.0-flash"
        }))
        .unwrap();
        app.dispatch(
            "skills.upsert",
            json!({
                "name": "Research",
                "instructions": "Prefer primary sources."
            }),
        )
        .unwrap();

        let plan = plan(&app, json!({ "task": "只读汇总", "maxSteps": 3 })).unwrap();
        assert_eq!(plan["model"], "agnes-3.0-flash");
        assert_eq!(plan["usage"]["tokensIn"], 10);
        server.join().unwrap();
    }

    #[test]
    fn execute_from_abandons_when_cancel_requested() {
        let app = app();
        let mut steps = vec![runs::RunStep {
            tool: "overview.get".to_string(),
            args: json!({}),
            status: "queued".to_string(),
            result: None,
            error: String::new(),
            at: String::new(),
        }];
        let steps_json = serde_json::to_string(&steps).unwrap();
        let run_id = app
            .store()
            .write(|conn| {
                conn.execute(
                    "INSERT INTO agent_run_log (started_at, task, outcome, steps_json)
                     VALUES (?1, '协作取消', 'running', ?2)",
                    rusqlite::params![now(), steps_json],
                )?;
                Ok(conn.last_insert_rowid())
            })
            .unwrap();
        request_cancel(&app, run_id).unwrap();
        execute_from(&app, run_id, &mut steps, 0).unwrap();
        let detail = runs::get(app.store(), &run_id.to_string()).unwrap();
        assert_eq!(detail.status, "cancelled");
        assert_eq!(detail.steps[0].status, "cancelled");
    }
}
