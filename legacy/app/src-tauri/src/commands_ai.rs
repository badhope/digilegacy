//! AI 助手与内置 Agent IPC 命令（从 commands.rs 拆出，纯移动，行为零变更）。
//!
//! 边界不变：未配置端点时降级为本地作答；出站载荷白名单；工具循环有界。

use crate::db;
use crate::models::*;
use tauri::command;

// ========== AI 助手（可选 · 用户自配端点） ==========

#[command]
pub fn get_assistant_config() -> Result<ApiResponse<crate::ai_assist::AssistantConfig>, String> {
    db::get_assistant_config_db().map(ApiResponse::ok)
}

#[command]
pub fn set_assistant_config(
    enabled: bool,
    endpoint: String,
    model: String,
    api_key: Option<String>,
) -> Result<ApiResponse<()>, String> {
    if endpoint.len() > 300 {
        return Ok(ApiResponse::fail("端点地址过长"));
    }
    if !endpoint.trim().is_empty() {
        let ep = endpoint.trim();
        if !(ep.starts_with("https://") || ep.starts_with("http://")) {
            return Ok(ApiResponse::fail("端点必须以 https:// 或 http:// 开头"));
        }
    }
    if model.len() > 120 {
        return Ok(ApiResponse::fail("模型名过长"));
    }
    if let Some(k) = api_key.as_deref() {
        if k.len() > 512 {
            return Ok(ApiResponse::fail("API Key 过长"));
        }
    }
    // 开启时必须有端点与模型；否则给一个明确提示，而不是等到提问时才失败。
    if enabled && (endpoint.trim().is_empty() || model.trim().is_empty()) {
        return Ok(ApiResponse::fail(
            "启用助手需要填写端点与模型名；没有服务时请保持关闭",
        ));
    }
    match db::set_assistant_config_db(enabled, endpoint.trim(), model.trim(), api_key.as_deref()) {
        Ok(()) => Ok(ApiResponse::ok_with_msg(
            (),
            "助手配置已保存（Key 已加密存储，不会回传）",
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 发送前预览：逐项列出将出境的非身份字段，让用户在点「提问」前看见全貌。
#[command]
pub fn preview_assistant_payload(
    question: String,
) -> Result<ApiResponse<crate::ai_assist::OutboundPreview>, String> {
    let report = db::get_inheritance_report_db()?;
    if crate::report::contains_placeholder(&report) {
        return Ok(ApiResponse::fail(
            "当前数据处于加密锁定状态，请先解锁后再使用助手",
        ));
    }
    Ok(ApiResponse::ok(crate::ai_assist::preview_outbound(
        &report, &question,
    )))
}

/// 提问。**只读**：产出的文本不会写库，用户确认后由前端拼进工具包。
#[command]
pub fn ask_assistant(
    question: String,
    locale: Option<String>,
) -> Result<ApiResponse<String>, String> {
    let cfg = db::get_assistant_config_db()?;
    if !cfg.enabled {
        return Ok(ApiResponse::fail(
            "助手未启用。本应用不内置任何 AI 服务；如你有 OpenAI 兼容端点，可在「我的档案 → AI 助手」填写地址与 Key",
        ));
    }
    if cfg.endpoint.trim().is_empty() || cfg.model.trim().is_empty() {
        return Ok(ApiResponse::fail("助手配置不完整（缺端点或模型名）"));
    }
    let key = match db::get_assistant_key_plain()? {
        Some(k) => k,
        None => return Ok(ApiResponse::fail("尚未填写 API Key")),
    };
    let report = db::get_inheritance_report_db()?;
    let question_trimmed = question.trim().to_string();
    match crate::ai_assist::ask(
        &report,
        &question,
        &cfg.endpoint,
        &key,
        &cfg.model,
        crate::report::ReportLocale::parse(locale.as_deref().unwrap_or("zh-CN")),
    ) {
        Ok(answer) => {
            // 问答本地留痕：界面可回看与清除。留痕失败不阻断回答本身。
            if let Err(e) = db::append_ai_chat_db(&question_trimmed, &answer, &cfg.model) {
                eprintln!("保存问答历史失败: {}", e);
            }
            Ok(ApiResponse::ok_with_msg(
                answer,
                "助手已回答（仅本地留痕，未写入任何资产数据）",
            ))
        }
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 流式提问：回答经 Tauri channel 逐字推送到前端。
///
/// 与 ask_assistant 相同的安全约束（白名单 payload、占位符拦截、Key 不回传）；
/// 流结束后把完整问答落一条本地历史。
#[command]
pub fn ask_assistant_streaming(
    question: String,
    locale: Option<String>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<ApiResponse<String>, String> {
    let cfg = db::get_assistant_config_db()?;
    if !cfg.enabled {
        return Ok(ApiResponse::fail(
            "助手未启用。本应用不内置任何 AI 服务；如你有 OpenAI 兼容端点，可在「我的档案 → AI 助手」填写地址与 Key",
        ));
    }
    if cfg.endpoint.trim().is_empty() || cfg.model.trim().is_empty() {
        return Ok(ApiResponse::fail("助手配置不完整（缺端点或模型名）"));
    }
    let key = match db::get_assistant_key_plain()? {
        Some(k) => k,
        None => return Ok(ApiResponse::fail("尚未填写 API Key")),
    };
    let report = db::get_inheritance_report_db()?;
    let question_trimmed = question.trim().to_string();
    if question_trimmed.is_empty() {
        return Ok(ApiResponse::fail("问题不能为空"));
    }
    if question_trimmed.chars().count() > 2_000 {
        return Ok(ApiResponse::fail("问题过长（上限 2000 字符）"));
    }
    if crate::report::contains_placeholder(&report) {
        return Ok(ApiResponse::fail(
            "当前数据处于加密锁定状态，请先解锁后再使用助手",
        ));
    }
    let payload = crate::ai_assist::preview_payload(&report, &question_trimmed);
    let user_content = serde_json::to_string_pretty(&payload).unwrap_or_default();
    let system = crate::ai_assist::system_prompt_pub(crate::report::ReportLocale::parse(
        locale.as_deref().unwrap_or("zh-CN"),
    ));

    let mut full = String::new();
    let result = crate::ai_assist::call_chat_api_streaming(
        &cfg.endpoint,
        &key,
        &cfg.model,
        system,
        &user_content,
        &mut |delta| {
            full.push_str(delta);
            // channel 关闭/背压不致命：收完流并落历史
            let _ = on_delta.send(delta.to_string());
        },
    );
    match result {
        Ok(()) => {
            if full.trim().is_empty() {
                Ok(ApiResponse::fail("模型未返回内容"))
            } else {
                if let Err(e) = db::append_ai_chat_db(&question_trimmed, full.trim(), &cfg.model) {
                    eprintln!("保存问答历史失败: {}", e);
                }
                Ok(ApiResponse::ok_with_msg(full.trim().to_string(), "已完成"))
            }
        }
        Err(e) => {
            // 已输出的部分交还前端展示，不整段丢弃
            if !full.is_empty() {
                let _ = on_delta.send(format!("\n\n[中断] {}", e));
            }
            Ok(ApiResponse::fail(&e))
        }
    }
}

#[command]
pub fn list_ai_chat(limit: Option<usize>) -> Result<ApiResponse<Vec<serde_json::Value>>, String> {
    match db::list_ai_chat_db(limit.unwrap_or(50)) {
        Ok(rows) => Ok(ApiResponse::ok(rows)),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

#[command]
pub fn clear_ai_chat() -> Result<ApiResponse<()>, String> {
    match db::clear_ai_chat_db() {
        Ok(()) => Ok(ApiResponse::ok_with_msg((), "问答历史已清空")),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

// ========== 内置 Agent（受限本地工具循环） ==========

fn agent_config_and_key() -> Result<(crate::ai_assist::AssistantConfig, String), String> {
    let cfg = db::get_assistant_config_db()?;
    if !cfg.enabled {
        return Err("助手未启用。请先在「我的档案 → AI 助手」配置用户自有端点".to_string());
    }
    if cfg.endpoint.trim().is_empty() || cfg.model.trim().is_empty() || !cfg.has_key {
        return Err("助手配置不完整（缺端点、模型名或 API Key）".to_string());
    }
    let key = db::get_assistant_key_plain()?.ok_or("尚未填写 API Key")?;
    Ok((cfg, key))
}

fn run_agent_inner(
    task: String,
    on_event: Option<tauri::ipc::Channel<crate::agent::AgentEvent>>,
) -> Result<crate::agent::AgentOutcome, String> {
    if task.trim().is_empty() {
        return Err("任务描述不能为空".to_string());
    }
    if task.chars().count() > 2_000 {
        return Err("任务描述过长（上限 2000 字符）".to_string());
    }
    let (cfg, key) = agent_config_and_key()?;
    let report = db::get_inheritance_report_db()?;
    if crate::report::contains_placeholder(&report) {
        return Err("当前数据处于加密锁定状态，请先解锁后再使用 Agent".to_string());
    }
    // 每次任务开始前清掉上一次遗留的取消标志。
    crate::agent::reset_cancel();
    let result = match on_event {
        Some(channel) => crate::agent::run_task_with_events(
            &task,
            &cfg.endpoint,
            &key,
            &cfg.model,
            move |event| {
                let _ = channel.send(event);
            },
        ),
        None => crate::agent::run_task(&task, &cfg.endpoint, &key, &cfg.model),
    };
    // 审计留痕：成败都记。审计写入失败不吞掉任务结果——
    // 审计是尽力而为的旁路，不是事务的一部分。
    match &result {
        Ok(out) => {
            let rounds = out.steps.iter().map(|s| s.round).max().unwrap_or(0);
            let tools = out
                .steps
                .iter()
                .map(|s| s.tool.as_str())
                .collect::<Vec<_>>()
                .join(",");
            let outcome = if out.truncated { "truncated" } else { "final" };
            let _ = db::log_agent_run_db(&task, outcome, &out.final_text, rounds, &tools);
        }
        Err(e) => {
            let brief: String = e.chars().take(500).collect();
            let _ = db::log_agent_run_db(&task, "error", &brief, 0, "");
        }
    }
    result
}

#[command]
pub async fn run_agent_task(
    task: String,
) -> Result<ApiResponse<crate::agent::AgentOutcome>, String> {
    let result = tauri::async_runtime::spawn_blocking(move || run_agent_inner(task, None))
        .await
        .map_err(|e| format!("Agent 工作线程异常: {}", e))?;
    result
        .map(ApiResponse::ok)
        .or_else(|e| Ok(ApiResponse::fail(&e)))
}

/// Agent 运行审计：最近 N 条摘要（新的在前）。
#[command]
pub fn list_agent_run_log(
    limit: Option<usize>,
) -> Result<ApiResponse<Vec<db::AgentRunLogEntry>>, String> {
    db::list_agent_run_log_db(limit.unwrap_or(10))
        .map(ApiResponse::ok)
        .or_else(|e| Ok(ApiResponse::fail(&e)))
}

/// 清空全部 Agent 运行审计。
#[command]
pub fn clear_agent_run_log() -> Result<ApiResponse<()>, String> {
    db::clear_agent_run_log_db()?;
    Ok(ApiResponse::ok_with_msg((), "已清空 Agent 运行记录"))
}

/// 请求取消当前 Agent 任务（协作式：当前模型轮次结束后生效）。
#[command]
pub fn cancel_agent_task() -> Result<ApiResponse<()>, String> {
    crate::agent::request_cancel();
    Ok(ApiResponse::ok_with_msg(
        (),
        "已发出取消请求，当前轮次结束后生效",
    ))
}

/// 运行受限本地 Agent。事件经 Channel 推给前端，最终结果仍以 ApiResponse 返回。
#[command]
pub async fn run_agent_task_streaming(
    task: String,
    on_event: tauri::ipc::Channel<crate::agent::AgentEvent>,
) -> Result<ApiResponse<crate::agent::AgentOutcome>, String> {
    let result =
        tauri::async_runtime::spawn_blocking(move || run_agent_inner(task, Some(on_event)))
            .await
            .map_err(|e| format!("Agent 工作线程异常: {}", e))?;
    result
        .map(ApiResponse::ok)
        .or_else(|e| Ok(ApiResponse::fail(&e)))
}
