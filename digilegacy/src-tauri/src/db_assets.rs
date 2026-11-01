//! 数字资产 CRUD（从 db.rs 拆出，纯移动，行为零变更）。
//!
//! 拆分理由：db.rs 曾同时承担连接生命周期、schema、资产 CRUD、档案、安全配置、
//! 备份导入导出与 DMS 六件事，单文件近 3000 行。资产 CRUD 是最常改动的域
//! （新增字段、调整查询），单独成文件后改动不会与加密/备份逻辑混在同一 diff 里。
//!
//! 依赖方向：本模块只依赖 db.rs 的连接与写入门禁（向上不依赖命令层），
//! 加密仍统一走 `vault`，不在本模块内自行处理密钥。

use chrono::Local;
use rusqlite::params;

use crate::models::*;

use super::{ensure_vault_writable, get_conn, reject_placeholder, row_to_asset};

pub fn add_asset_db(req: &AssetRequest) -> Result<i64, String> {
    validate_category(&req.category)?;
    validate_disposal(&req.disposal_method)?;
    validate_disposal_heir(&req.disposal_method, req.heir_name.as_deref())?;
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    ensure_vault_writable(conn)?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let account_identifier = crate::vault::encrypt_if_needed(&req.account_identifier)?;
    let account_description = crate::vault::encrypt_if_needed(&req.account_description)?;
    let heir_name = crate::vault::encrypt_if_needed(req.heir_name.as_deref().unwrap_or(""))?;
    let heir_contact = crate::vault::encrypt_if_needed(req.heir_contact.as_deref().unwrap_or(""))?;
    let notes = crate::vault::encrypt_if_needed(&req.notes)?;

    conn.execute(
        "INSERT INTO digital_assets (platform_id, account_identifier, account_description, category, estimated_value, value_currency, disposal_method, heir_name, heir_contact, notes, is_confirmed, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'CNY', ?6, ?7, ?8, ?9, 1, ?10, ?10)",
        params![
            req.platform_id,
            account_identifier,
            account_description,
            req.category,
            req.estimated_value.unwrap_or(0.0),
            req.disposal_method,
            heir_name,
            heir_contact,
            notes,
            now,
        ],
    ).map_err(|e| format!("添加资产失败: {}", e))?;

    Ok(conn.last_insert_rowid())
}

pub fn update_asset_db(id: i64, req: &AssetRequest) -> Result<(), String> {
    validate_category(&req.category)?;
    validate_disposal(&req.disposal_method)?;
    validate_disposal_heir(&req.disposal_method, req.heir_name.as_deref())?;
    // 占位符检查必须在任何写操作之前：这是「数据被覆盖」而非「写入失败」的风险。
    reject_placeholder("账号标识", &req.account_identifier)?;
    reject_placeholder("账号描述", &req.account_description)?;
    reject_placeholder("备注", &req.notes)?;
    if let Some(v) = req.heir_name.as_deref() {
        reject_placeholder("继承人姓名", v)?;
    }
    if let Some(v) = req.heir_contact.as_deref() {
        reject_placeholder("继承人联系方式", v)?;
    }
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    ensure_vault_writable(conn)?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let account_identifier = crate::vault::encrypt_if_needed(&req.account_identifier)?;
    let account_description = crate::vault::encrypt_if_needed(&req.account_description)?;
    let heir_name = crate::vault::encrypt_if_needed(req.heir_name.as_deref().unwrap_or(""))?;
    let heir_contact = crate::vault::encrypt_if_needed(req.heir_contact.as_deref().unwrap_or(""))?;
    let notes = crate::vault::encrypt_if_needed(&req.notes)?;

    conn.execute(
        "UPDATE digital_assets SET platform_id=?1, account_identifier=?2, account_description=?3, category=?4, estimated_value=?5, disposal_method=?6, heir_name=?7, heir_contact=?8, notes=?9, is_confirmed=1, updated_at=?10 WHERE id=?11",
        params![
            req.platform_id,
            account_identifier,
            account_description,
            req.category,
            req.estimated_value.unwrap_or(0.0),
            req.disposal_method,
            heir_name,
            heir_contact,
            notes,
            now,
            id,
        ],
    ).map_err(|e| format!("更新资产失败: {}", e))?;

    Ok(())
}

pub fn delete_asset_db(id: i64) -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    // 删除同样是写操作：已设主密码的库在锁定态不得删除，
    // 否则「锁定」只挡住了读取，没挡住破坏。
    ensure_vault_writable(conn)?;
    conn.execute("DELETE FROM digital_assets WHERE id=?1", params![id])
        .map_err(|e| format!("删除资产失败: {}", e))?;
    Ok(())
}

pub fn get_all_assets_db() -> Result<Vec<DigitalAsset>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn.prepare(
        "SELECT a.id, a.platform_id, COALESCE(o.name, p.name) AS platform_name, a.account_identifier, a.account_description, a.category, a.estimated_value, a.value_currency, a.disposal_method, a.heir_name, a.heir_contact, a.notes, a.is_confirmed, a.created_at, a.updated_at, CASE WHEN a.credential_enc IS NOT NULL AND a.credential_enc != '' THEN 1 ELSE 0 END AS has_cred
         FROM digital_assets a
         LEFT JOIN platforms p ON a.platform_id = p.id
         LEFT JOIN platform_overrides o ON a.platform_id = o.id ORDER BY a.created_at DESC"
    ).map_err(|e| format!("查询资产失败: {}", e))?;

    let assets = stmt
        .query_map([], row_to_asset)
        .map_err(|e| format!("解析资产数据失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集资产失败: {}", e))?;

    Ok(assets)
}

pub fn get_asset_by_id_db(id: i64) -> Result<Option<DigitalAsset>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn.prepare(
        "SELECT a.id, a.platform_id, COALESCE(o.name, p.name) AS platform_name, a.account_identifier, a.account_description, a.category, a.estimated_value, a.value_currency, a.disposal_method, a.heir_name, a.heir_contact, a.notes, a.is_confirmed, a.created_at, a.updated_at, CASE WHEN a.credential_enc IS NOT NULL AND a.credential_enc != '' THEN 1 ELSE 0 END AS has_cred
         FROM digital_assets a
         LEFT JOIN platforms p ON a.platform_id = p.id
         LEFT JOIN platform_overrides o ON a.platform_id = o.id WHERE a.id = ?1"
    ).map_err(|e| format!("查询资产失败: {}", e))?;

    let mut rows = stmt
        .query(params![id])
        .map_err(|e| format!("查询执行失败: {}", e))?;

    if let Some(row) = rows.next().map_err(|e| format!("读取行失败: {}", e))? {
        Ok(Some(
            row_to_asset(row).map_err(|e| format!("解析资产失败: {}", e))?,
        ))
    } else {
        Ok(None)
    }
}

pub fn get_assets_by_category_db(category: &str) -> Result<Vec<DigitalAsset>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn.prepare(
        "SELECT a.id, a.platform_id, COALESCE(o.name, p.name) AS platform_name, a.account_identifier, a.account_description, a.category, a.estimated_value, a.value_currency, a.disposal_method, a.heir_name, a.heir_contact, a.notes, a.is_confirmed, a.created_at, a.updated_at, CASE WHEN a.credential_enc IS NOT NULL AND a.credential_enc != '' THEN 1 ELSE 0 END AS has_cred
         FROM digital_assets a
         LEFT JOIN platforms p ON a.platform_id = p.id
         LEFT JOIN platform_overrides o ON a.platform_id = o.id WHERE a.category = ?1 ORDER BY a.estimated_value DESC"
    ).map_err(|e| format!("按类别查询失败: {}", e))?;

    let assets = stmt
        .query_map(params![category], row_to_asset)
        .map_err(|e| format!("解析失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集失败: {}", e))?;

    Ok(assets)
}
