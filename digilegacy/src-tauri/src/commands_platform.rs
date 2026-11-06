//! 平台、政策库补丁与权威来源 IPC 命令（从 commands.rs 拆出，纯移动，行为零变更）。

use crate::db;
use crate::models::*;
use crate::platform_data::get_all_platforms as get_all_platforms_data;
use tauri::command;

// ========== 平台 ==========

#[command]
pub fn get_all_platforms() -> Result<ApiResponse<Vec<Platform>>, String> {
    db::get_all_platforms_db().map(ApiResponse::ok)
}

#[command]
pub fn get_all_platforms_extended() -> Result<ApiResponse<Vec<PlatformExtended>>, String> {
    // 必须从数据库读（而不是直接用内置 const 表），否则用户导入的政策补丁在
    // 继承人向导 / 变现路径卡里完全不生效——两个入口会显示不一致的政策。
    let platforms = db::get_all_platforms_db()?;
    // 扩展元数据先一次性建索引：原实现在 map 内调用 get_all_platforms_data()，
    // 每处理一个平台就重建整张 152 条内置表（O(n²) 且每次分配上千个 String）。
    let meta_by_id: std::collections::HashMap<i64, crate::platform_data::PlatformEntry> =
        get_all_platforms_data()
            .into_iter()
            .map(|m| (m.id, m))
            .collect();
    let extended = platforms
        .into_iter()
        .map(|p| {
            let meta = meta_by_id.get(&p.id).cloned().unwrap_or_else(|| {
                // 补丁新增的平台在内置库里没有扩展元数据，用空值占位
                crate::platform_data::PlatformEntry::new(
                    p.id,
                    &p.name,
                    "",
                    &p.category,
                    &p.subtype,
                    p.has_property,
                    p.has_personality,
                    p.inheritable,
                    &p.required_documents,
                    &p.process_description,
                    &p.estimated_difficulty,
                    &p.official_policy_url,
                    &p.estimated_time_days,
                    &p.notes,
                )
            });
            PlatformExtended {
                base: Platform {
                    id: p.id,
                    name: p.name,
                    category: p.category,
                    subtype: p.subtype,
                    has_property: p.has_property,
                    has_personality: p.has_personality,
                    inheritable: p.inheritable,
                    required_documents: p.required_documents,
                    process_description: p.process_description,
                    estimated_difficulty: p.estimated_difficulty,
                    official_policy_url: p.official_policy_url,
                    estimated_time_days: p.estimated_time_days,
                    notes: p.notes,
                },
                name_en: meta.name_en,
                data_source: meta.data_source,
                inheritance_form_url: meta.inheritance_form_url,
                monetization_path: meta.monetization_path,
                legal_template_ids: meta.legal_template_ids,
                risk_warnings: meta.risk_warnings,
                tags: meta.tags,
                last_reviewed: meta.last_reviewed,
            }
        })
        .collect();
    Ok(ApiResponse::ok(extended))
}

// ========== 政策库补丁（离线更新通道） ==========

#[command]
pub fn import_platform_patch(content: String) -> Result<ApiResponse<String>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    // 与备份导入同一量级的上限：政策补丁是纯文本 JSON
    if content.len() > 8 * 1024 * 1024 {
        return Ok(ApiResponse::fail("政策补丁过大（上限 8MB）"));
    }
    match db::import_platform_overrides_db(&content) {
        Ok((added, updated)) => Ok(ApiResponse::ok_with_msg(
            format!("新增 {} 条，覆盖 {} 条", added, updated),
            "政策库补丁已导入",
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

#[command]
pub fn export_platform_patch() -> Result<ApiResponse<String>, String> {
    let json = db::export_platform_overrides_db()?;
    let app_dir = crate::paths::data_dir()?;
    let dir = app_dir.join("政策补丁");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
    let name = format!(
        "digilegacy-platform-patch-{}.json",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let path = dir.join(name);
    // 与备份导出同样落在应用数据目录，不额外申请前端写文件权限
    std::fs::write(&path, json).map_err(|e| format!("写入补丁失败: {}", e))?;
    Ok(ApiResponse::ok_with_msg(
        path.to_string_lossy().to_string(),
        "政策补丁已导出",
    ))
}

#[command]
pub fn platform_patch_status() -> Result<ApiResponse<i64>, String> {
    db::count_platform_overrides_db().map(ApiResponse::ok)
}

#[command]
pub fn clear_platform_patch() -> Result<ApiResponse<usize>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    db::clear_platform_overrides_db().map(ApiResponse::ok)
}

// ========== 权威来源 ==========

/// 内置权威来源表。每条都带「可达 / 内容匹配」两个独立结论。
#[command]
pub fn list_authorities() -> Result<ApiResponse<Vec<Authority>>, String> {
    Ok(ApiResponse::ok(crate::models::authorities()))
}

/// 某个资产类别相关的来源（报告与继承人向导用）。
#[command]
pub fn authorities_for(category: String) -> Result<ApiResponse<Vec<Authority>>, String> {
    let list = crate::models::authorities()
        .into_iter()
        .filter(|a| a.applies_to.iter().any(|c| c == &category))
        .collect();
    Ok(ApiResponse::ok(list))
}
