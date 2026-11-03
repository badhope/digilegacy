//! 数字资产、处置意愿与继承人名单 IPC 命令（从 commands.rs 拆出，纯移动，行为零变更）。
//!
//! 入参上限校验 `validate_asset_request` 随资产命令走，与写入路径保持同文件。

use crate::db;
use crate::models::*;
use tauri::command;

// ========== 数字资产 ==========

/// IPC 入参上限：文本字段单条 8KB、ID 必须为正。
/// 没有上限时，一次失控调用就能把几十 MB 写进 SQLite（且会被每日备份复制 7 份）。
fn validate_asset_request(req: &AssetRequest) -> Result<(), String> {
    if req.platform_id <= 0 {
        return Err("平台 id 必须为正数".to_string());
    }
    for (name, value) in [
        ("账号标识", Some(req.account_identifier.as_str())),
        ("账号描述", Some(req.account_description.as_str())),
        ("备注", Some(req.notes.as_str())),
        ("继承人姓名", req.heir_name.as_deref()),
        ("继承人联系方式", req.heir_contact.as_deref()),
    ] {
        if let Some(v) = value {
            if v.chars().count() > 8_000 {
                return Err(format!("{}过长（上限 8000 字符）", name));
            }
        }
    }
    if let Some(v) = req.estimated_value {
        if !v.is_finite() || v < 0.0 || v > 1e12 {
            return Err("预估价值必须是 0 到 1 万亿之间的有限数值".to_string());
        }
    }
    Ok(())
}

#[command]
pub fn add_asset(req: AssetRequest) -> Result<ApiResponse<i64>, String> {
    if let Err(e) = validate_asset_request(&req) {
        return Ok(ApiResponse::fail(&e));
    }
    let id = db::add_asset_db(&req)?;
    Ok(ApiResponse::ok_with_msg(id, "数字资产添加成功"))
}

#[command]
pub fn update_asset(id: i64, req: AssetRequest) -> Result<ApiResponse<()>, String> {
    let exists = db::get_asset_by_id_db(id)?.is_some();
    if !exists {
        return Ok(ApiResponse::fail("资产不存在或已被删除"));
    }
    if let Err(e) = validate_asset_request(&req) {
        return Ok(ApiResponse::fail(&e));
    }
    db::update_asset_db(id, &req)?;
    Ok(ApiResponse::ok_with_msg((), "资产更新成功"))
}

#[command]
pub fn delete_asset(id: i64) -> Result<ApiResponse<()>, String> {
    if db::get_asset_by_id_db(id)?.is_none() {
        return Ok(ApiResponse::fail("资产不存在或已被删除"));
    }
    db::delete_asset_db(id)?;
    Ok(ApiResponse::ok_with_msg((), "资产已删除"))
}

#[command]
pub fn get_all_assets() -> Result<ApiResponse<Vec<DigitalAsset>>, String> {
    db::get_all_assets_db().map(ApiResponse::ok)
}

#[command]
pub fn get_asset_by_id(id: i64) -> Result<ApiResponse<Option<DigitalAsset>>, String> {
    db::get_asset_by_id_db(id).map(ApiResponse::ok)
}

#[command]
pub fn get_assets_by_category(category: String) -> Result<ApiResponse<Vec<DigitalAsset>>, String> {
    db::get_assets_by_category_db(&category).map(ApiResponse::ok)
}

// ========== 处置意愿 ==========

#[command]
pub fn set_disposal_wish(
    mut disposal_method: String,
    asset_id: i64,
    heir_name: Option<String>,
    heir_contact: Option<String>,
    mut notes: String,
) -> Result<ApiResponse<()>, String> {
    // 此命令绕过 add/update_asset 的校验直接写库，必须自带同样的边界，
    // 否则它是唯一一条能把任意长度文本与非法枚举写进资产表的入口。
    if let Err(e) = crate::models::validate_disposal(&disposal_method) {
        return Ok(ApiResponse::fail(&e));
    }
    for (name, value) in [
        ("备注", Some(notes.as_str())),
        ("继承人姓名", heir_name.as_deref()),
        ("继承人联系方式", heir_contact.as_deref()),
    ] {
        if let Some(v) = value {
            if v.chars().count() > 8_000 {
                return Ok(ApiResponse::fail(&format!(
                    "{}过长（上限 8000 字符）",
                    name
                )));
            }
        }
    }
    let asset = db::get_asset_by_id_db(asset_id)?.ok_or("资产不存在")?;

    let req = AssetRequest {
        platform_id: asset.platform_id,
        account_identifier: asset.account_identifier,
        account_description: asset.account_description,
        category: asset.category,
        estimated_value: asset.estimated_value,
        disposal_method: std::mem::take(&mut disposal_method),
        heir_name,
        heir_contact,
        notes: std::mem::take(&mut notes),
    };

    db::update_asset_db(asset_id, &req)?;
    Ok(ApiResponse::ok_with_msg((), "处置意愿已设定"))
}

// ========== 继承人名单（多继承人 + 执行人） ==========

#[command]
pub fn get_heirs() -> Result<ApiResponse<Vec<Heir>>, String> {
    db::get_heirs_db().map(ApiResponse::ok)
}

/// 整组替换：家庭成员是一个整体，逐条增删会短暂留下「两个执行人」的非法状态。
#[command]
pub fn set_heirs(heirs: Vec<Heir>) -> Result<ApiResponse<()>, String> {
    if heirs.len() > db::MAX_HEIRS {
        return Ok(ApiResponse::fail(&format!(
            "继承人最多 {} 位",
            db::MAX_HEIRS
        )));
    }
    match db::replace_heirs_db(&heirs) {
        Ok(()) => Ok(ApiResponse::ok_with_msg((), "继承人名单已保存")),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}
