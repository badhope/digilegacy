//! 用户档案、Dead Man's Switch、仪表板统计与 IMAP 签到 IPC 命令（从 commands.rs 拆出，纯移动，行为零变更）。

use crate::db;
use crate::db::CheckInStatus;
use crate::models::*;
use tauri::command;
use zeroize::Zeroize;

// ========== 用户档案 ==========

#[command]
pub fn set_user_profile(profile: UserProfile) -> Result<ApiResponse<()>, String> {
    // 档案字段会进入继承报告与 DMS 邮件正文，必须有上限；
    // 其中紧急联系人邮箱还会作为 SMTP 收件人。
    for (name, value) in [
        ("姓名", profile.full_name.as_str()),
        ("手机号", profile.phone.as_str()),
        ("邮箱", profile.email.as_str()),
        ("紧急联系人姓名", profile.emergency_contact_name.as_str()),
        ("紧急联系人电话", profile.emergency_contact_phone.as_str()),
        ("紧急联系人邮箱", profile.emergency_contact_email.as_str()),
        ("继承人姓名", profile.heir_primary_name.as_str()),
        ("继承人电话", profile.heir_primary_phone.as_str()),
        ("继承人邮箱", profile.heir_primary_email.as_str()),
    ] {
        if value.chars().count() > 500 {
            return Ok(ApiResponse::fail(&format!("{}过长（上限 500 字符）", name)));
        }
    }
    if !(1..=365).contains(&profile.check_in_interval_days) {
        return Ok(ApiResponse::fail("检查间隔必须在 1 到 365 天之间"));
    }
    db::save_user_profile_db(&profile)?;
    Ok(ApiResponse::ok_with_msg((), "用户档案已保存"))
}

#[command]
pub fn get_user_profile() -> Result<ApiResponse<Option<UserProfile>>, String> {
    db::get_user_profile_db().map(ApiResponse::ok)
}

// ========== 检查记录 / Dead Man's Switch ==========

#[command]
pub fn update_check_in() -> Result<ApiResponse<String>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    db::record_check_in_internal_db()?;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    Ok(ApiResponse::ok_with_msg(now, "检查完成，状态良好"))
}

#[command]
pub fn get_check_in_status() -> Result<ApiResponse<CheckInStatus>, String> {
    db::get_check_in_status_db().map(ApiResponse::ok)
}

/// 只读的 DMS 状态：供界面渲染，不触发投递。
#[command]
pub fn get_dms_status() -> Result<ApiResponse<crate::notify::DmsStatus>, String> {
    crate::notify::status_snapshot().map(ApiResponse::ok)
}

/// 本地发件箱历史（最近 50 条），让用户能看到「过去真的发过什么」。
#[command]
pub fn get_dms_history() -> Result<ApiResponse<Vec<crate::notify::DmsNotification>>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    crate::notify::outbox_history(50).map(ApiResponse::ok)
}

#[command]
pub fn set_dead_mans_switch(enabled: bool, interval_days: i64) -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    // 合法区间与前端下拉一致：1 天（测试/特殊场景）到 365 天。
    // 0 或负数会让「到期日」落在过去，第一天就触发三级通知；极端大值等于永不提醒。
    if !(1..=365).contains(&interval_days) {
        return Ok(ApiResponse::fail("检查间隔必须在 1 到 365 天之间"));
    }
    db::set_dms_config_db(enabled, interval_days)?;
    let msg = if enabled {
        format!("Dead Man's Switch 已开启，每 {} 天检查一次", interval_days)
    } else {
        "Dead Man's Switch 已关闭".to_string()
    };
    Ok(ApiResponse::ok_with_msg((), &msg))
}

/// 外出模式：暂停/恢复到期提醒的逾期评估。空串 = 恢复。
#[command]
pub fn set_dms_pause(until: String) -> Result<ApiResponse<()>, String> {
    let clean = until.trim().to_string();
    db::set_dms_pause_db(&clean)?;
    let msg = if clean.is_empty() {
        "已恢复到期提醒评估".to_string()
    } else {
        format!("已开启外出暂停，{clean} 前不会判逾期")
    };
    Ok(ApiResponse::ok_with_msg((), &msg))
}

/// 标记「本次规划已复核」，重置年度复核提醒。
#[command]
pub fn record_planning_review() -> Result<ApiResponse<()>, String> {
    db::record_planning_review_db()?;
    Ok(ApiResponse::ok_with_msg((), "已记录本次复核"))
}

// ========== 仪表板统计 ==========

#[command]
pub fn get_dashboard_stats() -> Result<ApiResponse<DashboardStats>, String> {
    db::get_dashboard_stats_db().map(ApiResponse::ok)
}

// ========== IMAP 入站签到（回邮件 = 我还安好） ==========

#[command]
pub fn get_imap_config() -> Result<ApiResponse<crate::imap_checkin::ImapConfig>, String> {
    crate::imap_checkin::get_config().map(ApiResponse::ok)
}

#[command]
pub fn set_imap_config(
    config: crate::imap_checkin::ImapConfig,
    password: Option<String>,
) -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let mut pass = password;
    let result = crate::imap_checkin::set_config(&config, pass.as_deref());
    if let Some(p) = pass.as_mut() {
        p.zeroize();
    }
    result.map(|_| ApiResponse::ok_with_msg((), "IMAP 签到配置已保存"))
}

/// 手动触发一次收件箱轮询（后台循环每 30 分钟也会跑）。
#[command]
pub fn poll_imap_checkin() -> Result<ApiResponse<crate::imap_checkin::ImapCheckResult>, String> {
    Ok(ApiResponse::ok(crate::imap_checkin::poll_once()))
}
