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

use crate::Application;

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

fn save(
    app: &Application,
    run_id: i64,
    outcome: &str,
    steps: &[runs::RunStep],
    final_text: &str,
) -> Result<(), PlatformError> {
    let steps_json =
        serde_json::to_string(steps).map_err(|e| internal(format!("步骤序列化失败: {e}")))?;
    let used = tools_used(steps);
    let count = rounds(steps);
    app.store()
        .write(|conn| {
            conn.execute(
                "UPDATE agent_run_log
                 SET outcome=?2, rounds=?3, tools_used=?4, steps_json=?5, final_text=?6
                 WHERE id=?1",
                rusqlite::params![run_id, outcome, count, used, steps_json, final_text],
            )?;
            Ok(())
        })
        .map_err(|e| internal(format!("运行记录写入失败: {e}")))?;
    Ok(())
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
) -> Result<i64, PlatformError> {
    let started = now();
    let steps_json =
        serde_json::to_string(steps).map_err(|e| internal(format!("步骤序列化失败: {e}")))?;
    let run_id = app
        .store()
        .write(|conn| {
            conn.execute(
                "INSERT INTO agent_run_log
                 (started_at, task, outcome, rounds, tools_used, steps_json, final_text)
                 VALUES (?1, ?2, 'running', 0, '', ?3, '')",
                rusqlite::params![started, task, steps_json],
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
                save(app, run_id, "running", steps, "")?;
            }
            Err(error) => {
                steps[index].status = "failed".to_string();
                steps[index].error = format!("{}: {}", error.code, error.message);
                steps[index].at = now();
                save(app, run_id, "failed", steps, &steps[index].error.clone())?;
                return Ok(());
            }
        }
    }
    let done = steps.iter().filter(|step| step.status == "done").count();
    save(app, run_id, "succeeded", steps, &format!("完成 {done} 步"))?;
    app.audit("agent.run.succeeded", &run_id.to_string())?;
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
    let run_id = create_run(app, input.task.trim(), &steps)?;
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
    // The Application is synchronous and is called from async HTTP handlers.
    // Run the blocking planner on a dedicated thread so reqwest does not drop
    // its runtime inside the server's tokio worker.
    let plan = std::thread::spawn(move || digilegacy_agent::planner::plan(&task, max_steps))
        .join()
        .map_err(|_| PlatformError::new("INTERNAL", "规划线程异常退出"))?
        .map_err(|error| PlatformError::new(error.code(), error.message()))?;
    app.audit("agent.run.plan", "planner")?;
    Ok(json!(plan))
}

pub fn cancel(app: &Application, args: Value) -> Result<Value, PlatformError> {
    let id = args
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| PlatformError::invalid_args("缺少 id"))?;
    let run_id = parse_run_id(id)?;
    let detail =
        runs::get(app.store(), id).map_err(|e| PlatformError::new("NOT_FOUND", e.to_string()))?;
    if matches!(
        detail.status.as_str(),
        "succeeded" | "failed" | "rejected" | "cancelled"
    ) {
        return Err(PlatformError::new("CONFLICT", "终态运行不能取消"));
    }
    let mut steps = detail.steps;
    for step in &mut steps {
        if matches!(step.status.as_str(), "queued" | "pending_approval") {
            step.status = "cancelled".to_string();
            step.at = now();
        }
    }
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
    let new_id = create_run(app, &detail.task, &steps)?;
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
