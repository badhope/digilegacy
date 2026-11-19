//! 检查点 IPC 命令：创建 / 列出 / 回滚 / 删除。
//!
//! 回滚复用既有的认证式导入路径，因此同样要求源口令与 AEAD 校验通过；
//! 回滚前先自动打一个 `pre-restore` 检查点，保证回滚本身也可撤销。
//!
//! 数据目录走 `crate::paths` 而不是 `AppHandle`：桌面、CLI、HTTP 服务
//! 三个入口共用同一套命令逻辑，行为不会漂移。

use crate::checkpoint;
use crate::db;
use crate::models::*;
use tauri::command;

fn data_dir() -> Result<std::path::PathBuf, String> {
    crate::paths::data_dir()
}

/// 手动创建检查点：先导出完整备份 JSON，再落到检查点目录。
#[command]
pub fn create_checkpoint(label: String) -> Result<ApiResponse<checkpoint::CheckpointMeta>, String> {
    let label = label.trim();
    if label.chars().count() > 80 {
        return Ok(ApiResponse::fail("检查点名称过长（上限 80 字符）"));
    }
    let dir = data_dir()?;
    let json = db::export_backup_json_db()?;
    let meta = checkpoint::create(&dir, &json, label, false)?;
    Ok(ApiResponse::ok_with_msg(meta, "检查点已创建"))
}

#[command]
pub fn list_checkpoints() -> Result<ApiResponse<Vec<checkpoint::CheckpointMeta>>, String> {
    let dir = data_dir()?;
    Ok(ApiResponse::ok(checkpoint::list(&dir)))
}

#[command]
pub fn delete_checkpoint(id: String) -> Result<ApiResponse<()>, String> {
    let dir = data_dir()?;
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
) -> Result<ApiResponse<String>, String> {
    let dir = data_dir()?;
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
