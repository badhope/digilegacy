//! 通用审核管线 IPC 命令（从 commands.rs 拆出，纯移动，行为零变更）。
//!
//! AI 只能推进到「待人工」，`Approved` 只有人工可达。

use crate::db;
use crate::models::*;
use tauri::command;

// ========== 通用审核管线（AI 初审 + 人工终审） ==========

/// 已注册的审核场景。界面据此渲染场景切换，而不是写死「政策」一个。
#[command]
pub fn list_review_profiles() -> Result<ApiResponse<Vec<crate::review::ReviewProfile>>, String> {
    Ok(ApiResponse::ok(crate::review_profiles::all_profiles()))
}

/// 某场景的全部条目及其当前审核状态。
#[command]
pub fn list_review_items(
    profile_id: String,
) -> Result<ApiResponse<Vec<crate::db::ItemState>>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let Some(items) = crate::review_profiles::items_for(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    match crate::db::summarise_states_db(&profile_id, &items) {
        Ok(states) => Ok(ApiResponse::ok(states)),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 某场景的审核进度摘要：(已通过, 待人工, 待审)。
/// 界面用来回答「这个场景还差多少」——没有摘要时用户只能一条条数。
#[command]
pub fn review_progress(profile_id: String) -> Result<ApiResponse<serde_json::Value>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let Some(items) = crate::review_profiles::items_for(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let (approved, awaiting, pending) = crate::db::review_counts_db(&profile_id, &items)?;
    Ok(ApiResponse::ok(serde_json::json!({
        "total": items.len(),
        "approved": approved,
        "awaiting_human": awaiting,
        "pending": pending,
    })))
}

/// 该场景会送审哪些字段（界面在跑 AI 前展示，与助手的出境预览同一条原则）。
#[command]
pub fn review_payload_fields(profile_id: String) -> Result<ApiResponse<Vec<String>>, String> {
    Ok(ApiResponse::ok(crate::review_profiles::payload_fields_for(
        &profile_id,
    )))
}

/// 跑 AI 初审（只写 `AwaitingHuman`，绝不写 `Approved`）。
///
/// 没有配置 AI 端点时**不是错误**：返回 `Skipped` 的结果，条目进入纯人工队列。
#[command]
pub fn run_ai_review(
    profile_id: String,
    limit: Option<usize>,
) -> Result<ApiResponse<Vec<crate::db::ItemState>>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let Some(profile) = crate::review_profiles::profile_by_id(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let Some(items) = crate::review_profiles::items_for(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let states = crate::db::summarise_states_db(&profile_id, &items)?;
    // 只审「待审」的条目：已通过且内容未变的跳过，避免重复花费。
    let todo: Vec<&crate::review::ReviewItem> = items
        .iter()
        .filter(|i| {
            states
                .iter()
                .find(|s| s.key == i.key)
                .map(|s| s.status == crate::review::ReviewStatus::Pending)
                .unwrap_or(true)
        })
        .take(limit.unwrap_or(usize::MAX))
        .collect();

    if todo.is_empty() {
        return Ok(ApiResponse::ok_with_msg(states, "没有需要初审的条目"));
    }

    // 未配置 AI / 未启用：优雅降级，写一条 Skipped 留痕，条目转入人工队列。
    let cfg = db::get_assistant_config_db()?;
    let ai_ready = cfg.enabled
        && !cfg.endpoint.trim().is_empty()
        && !cfg.model.trim().is_empty()
        && cfg.has_key;
    if !profile.use_ai || !ai_ready {
        let reason = if !profile.use_ai {
            "该场景声明为纯人工审核"
        } else {
            "未配置 AI 端点，已转为纯人工审核"
        };
        for item in &todo {
            crate::db::append_review_record_db(&crate::review::ReviewRecord {
                profile_id: profile_id.clone(),
                item_key: item.key.clone(),
                fingerprint: item.fingerprint.clone(),
                ai_verdict: crate::review::AiVerdict::Skipped,
                ai_note: reason.to_string(),
                human_verdict: None,
                human_note: String::new(),
                decided_by: "ai".to_string(),
                created_at: String::new(),
            })?;
        }
        let after = crate::db::summarise_states_db(&profile_id, &items)?;
        return Ok(ApiResponse::ok_with_msg(after, reason));
    }

    let key = db::get_assistant_key_plain()?.unwrap_or_default();
    for item in &todo {
        let prompt = crate::review::ai_review_prompt(&profile, item);
        let (verdict, note) = match crate::ai_assist::call_chat_api(
            &cfg.endpoint,
            &key,
            &cfg.model,
            "你是内容初审员，只输出 PASS/FLAG/UNKNOWN 加依据。你不是最终决策者。",
            &prompt,
        ) {
            Ok(raw) => {
                let (v, n) = crate::review::parse_ai_verdict(&raw);
                // 严格模式：逐项作答缺任何一项，整条降级为 Unknown。
                // 模型跳过某一项往往说明它没看，而不是「没问题」。
                let (missing, flagged) = crate::review::validate_strict_answer(&profile, &raw);
                if !missing.is_empty() {
                    (
                        crate::review::AiVerdict::Unknown,
                        format!(
                            "{}\n\n[严格模式] 缺少以下必检项作答，本次初审作废：{}",
                            n,
                            missing.join(", ")
                        ),
                    )
                } else if !flagged.is_empty() && v == crate::review::AiVerdict::Pass {
                    // 逐项检查报了问题却给 PASS：以逐项结论为准，改为 FLAG
                    (
                        crate::review::AiVerdict::Flag,
                        format!("{}\n\n[严格模式] 逐项检查发现：{}", n, flagged.join(", ")),
                    )
                } else {
                    (v, n)
                }
            }
            Err(e) => (
                crate::review::AiVerdict::Unknown,
                format!("AI 调用失败：{}", e),
            ),
        };
        crate::db::append_review_record_db(&crate::review::ReviewRecord {
            profile_id: profile_id.clone(),
            item_key: item.key.clone(),
            fingerprint: item.fingerprint.clone(),
            ai_verdict: verdict,
            ai_note: note,
            human_verdict: None,
            human_note: String::new(),
            decided_by: "ai".to_string(),
            created_at: String::new(),
        })?;
    }
    let after = crate::db::summarise_states_db(&profile_id, &items)?;
    let msg = format!("AI 初审完成，{} 条待人工确认", todo.len());
    Ok(ApiResponse::ok_with_msg(after, &msg))
}

/// 人工终审。**只有这里能让条目变成 Approved。**
#[command]
pub fn submit_human_review(
    profile_id: String,
    item_key: String,
    verdict: String,
    note: Option<String>,
) -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let Some(_profile) = crate::review_profiles::profile_by_id(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let Some(items) = crate::review_profiles::items_for(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let Some(item) = items.iter().find(|i| i.key == item_key) else {
        return Ok(ApiResponse::fail("条目不存在"));
    };
    let human = match verdict.as_str() {
        "approved" => crate::review::HumanVerdict::Approved,
        "rejected" => crate::review::HumanVerdict::Rejected,
        "needs_edit" => crate::review::HumanVerdict::NeedsEdit,
        _ => return Ok(ApiResponse::fail("无效的审核结论")),
    };
    let note = note.unwrap_or_default();
    if note.chars().count() > 2_000 {
        return Ok(ApiResponse::fail("审核备注过长（上限 2000 字符）"));
    }
    // 严格场景：AI 提过疑点却要「通过」时，必须写明理由。
    // 这不是流程洁癖——空白放行会让留痕无法回答「当初为什么认为可以」。
    if human == crate::review::HumanVerdict::Approved {
        if let Some(profile) = crate::review_profiles::profile_by_id(&profile_id) {
            if profile.strictness == crate::review::ReviewStrictness::Strict {
                let history = crate::db::list_review_records_db(&profile_id, &item_key)?;
                let ai_flagged = history
                    .first()
                    .map(|r| {
                        matches!(
                            r.ai_verdict,
                            crate::review::AiVerdict::Flag | crate::review::AiVerdict::Unknown
                        )
                    })
                    .unwrap_or(false);
                if ai_flagged && note.trim().is_empty() {
                    return Ok(ApiResponse::fail(
                        "AI 初审对本条提出了疑点，通过时必须填写理由（用于留痕）",
                    ));
                }
            }
        }
    }
    // 沿用最近一次的 AI 结论一起留痕，便于回看「当时 AI 说了什么」。
    let history = crate::db::list_review_records_db(&profile_id, &item.key)?;
    let (ai_verdict, ai_note) = history
        .first()
        .map(|r| (r.ai_verdict.clone(), r.ai_note.clone()))
        .unwrap_or((crate::review::AiVerdict::Skipped, String::new()));
    crate::db::append_review_record_db(&crate::review::ReviewRecord {
        profile_id: profile_id.clone(),
        item_key: item.key.clone(),
        fingerprint: item.fingerprint.clone(),
        ai_verdict,
        ai_note,
        human_verdict: Some(human),
        human_note: note,
        decided_by: "human".to_string(),
        created_at: String::new(),
    })?;
    Ok(ApiResponse::ok_with_msg((), "审核结论已记录"))
}

/// 某条目的完整审核历史（新在前）。留痕只增不改。
#[command]
pub fn review_history(
    profile_id: String,
    item_key: String,
) -> Result<ApiResponse<Vec<crate::review::ReviewRecord>>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    crate::db::list_review_records_db(&profile_id, &item_key).map(ApiResponse::ok)
}
