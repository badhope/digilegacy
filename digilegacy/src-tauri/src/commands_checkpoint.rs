//! 检查点 IPC 命令：创建 / 列出 / 回滚 / 删除。
//!
//! 回滚复用既有的认证式导入路径，因此同样要求源口令与 AEAD 校验通过；
//! 回滚前先自动打一个 `pre-restore` 检查点，保证回滚本身也可撤销。

use crate::checkpoint;
use crate::db;
use crate::models::*;
use tauri::{command, AppHandle, Manager};

fn data_dir(app_handle: &AppHandle) -> Result<std::path::PathBuf, String> {
    app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取数据目录失败: {}", e))
}

/// 手动创建检查点：先导出完整备份 JSON，再落到检查点目录。
#[command]
pub fn create_checkpoint(
    label: String,
    app_handle: AppHandle,
) -> Result<ApiResponse<checkpoint::CheckpointMeta>, String> {
    let label = label.trim();
    if label.chars().count() > 80 {
        return Ok(ApiResponse::fail("检查点名称过长（上限 80 字符）"));
    }
    let dir = data_dir(&app_handle)?;
    let json = db::export_backup_json_db()?;
    let meta = checkpoint::create(&dir, &json, label, false)?;
    Ok(ApiResponse::ok_with_msg(meta, "检查点已创建"))
}

#[command]
pub fn list_checkpoints(
    app_handle: AppHandle,
) -> Result<ApiResponse<Vec<checkpoint::CheckpointMeta>>, String> {
    let dir = data_dir(&app_handle)?;
    Ok(ApiResponse::ok(checkpoint::list(&dir)))
}

#[command]
pub fn delete_checkpoint(id: String, app_handle: AppHandle) -> Result<ApiResponse<()>, String> {
    let dir = data_dir(&app_handle)?;
    checkpoint::delete(&dir, &id)?;
    Ok(ApiResponse::ok_with_msg((), "检查点已删除"))
}

/// 回滚到指定检查点。
///
/// 参数 `source_password` 是**该检查点创建时**的有效主密码：检查点是完整备份，
/// 导入时必须通过认证（源口令 + wrapped_dek + AEAD 校验），不能跳过。
#[command]
pub fn restore_checkpoint(
    id: String,
    source_password: Option<String>,
    app_handle: AppHandle,
) -> Result<ApiResponse<String>, String> {
    let dir = data_dir(&app_handle)?;
    let content = checkpoint::read(&dir, &id)?;

    // 先把「回滚前」的状态存下来，否则回滚成了一趟没有返程票的车。
    if let Ok(current) = db::export_backup_json_db() {
        let _ = checkpoint::create(&dir, &current, "pre-restore", true);
    }

    let password = source_password.unwrap_or_default();
    let password_opt = if password.is_empty() {
        None
    } else {
        Some(password.as_str())
    };

    match db::import_backup_json_db_with_mode_and_password(
        &content,
        db::ImportMode::Replace,
        password_opt,
    ) {
        Ok(rows) => Ok(ApiResponse::ok_with_msg(
            format!("已回滚，恢复 {} 条记录", rows),
            "已回滚到检查点",
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}
