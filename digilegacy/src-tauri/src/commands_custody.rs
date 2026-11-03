//! 保管位置 IPC 命令（从 commands.rs 拆出的试验模块）。

use crate::db;
use crate::models::ApiResponse;
use tauri::command;

#[command]
pub fn get_custody_hints() -> Result<ApiResponse<db::CustodyHints>, String> {
    db::get_custody_hints_db().map(ApiResponse::ok)
}

#[command]
pub fn set_custody_hints(hints: db::CustodyHints) -> Result<ApiResponse<()>, String> {
    for (name, v) in [
        ("备份位置", hints.backup_location.as_str()),
        ("恢复码位置", hints.recovery_code_location.as_str()),
    ] {
        if v.chars().count() > 500 {
            return Ok(ApiResponse::fail(&format!("{}过长（上限 500 字符）", name)));
        }
        if let Err(e) = crate::report::reject_placeholder_value(name, v) {
            return Ok(ApiResponse::fail(&e));
        }
    }
    match db::set_custody_hints_db(&hints) {
        Ok(()) => Ok(ApiResponse::ok_with_msg((), "保管位置已保存")),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}
