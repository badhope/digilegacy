//! 政策库补丁：离线更新平台继承政策的导入/导出/清除。
//!
//! 从 db.rs 拆出（纯移动，行为零变更）。这个域的坑集中在「补丁与 platforms 表的关系」：
//! - `digital_assets.platform_id` 有外键指向 `platforms`，补丁新增的平台必须镜像过去，
//!   否则界面上能选、一保存就 FOREIGN KEY constraint failed；
//! - 清除补丁时若仍有资产挂在补丁平台上，不能删父行（会级联失败或丢数据）。

use rusqlite::{params, Connection};

use crate::models::*;
use crate::platform_data::get_all_platforms;

use super::{get_conn, row_to_platform};

// ========== 政策库补丁（离线更新通道） ==========

/// 把用户导入的政策补丁叠加到内置平台列表上。
/// 同一 id 的补丁整体覆盖内置条目；补丁里出现内置库没有的 id 则新增。
pub(super) fn apply_platform_overrides(
    conn: &Connection,
    platforms: &mut Vec<Platform>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes FROM platform_overrides ORDER BY id",
        )
        .map_err(|e| format!("查询政策补丁失败: {}", e))?;

    let overrides: Vec<Platform> = stmt
        .query_map([], row_to_platform)
        .map_err(|e| format!("政策补丁解析失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("政策补丁收集失败: {}", e))?;

    for ov in overrides {
        match platforms.iter_mut().find(|p| p.id == ov.id) {
            Some(existing) => *existing = ov,
            None => platforms.push(ov),
        }
    }
    Ok(())
}

/// 导入政策补丁。返回 (新增数, 覆盖数)。
/// 只接受结构完整、id 为正、字段非空的条目；整体事务，任一条不合法则全部回滚。
pub fn import_platform_overrides_db(payload: &str) -> Result<(usize, usize), String> {
    #[derive(serde::Deserialize)]
    struct PatchFile {
        #[serde(default)]
        app: String,
        platforms: Vec<PlatformPatch>,
    }
    #[derive(serde::Deserialize)]
    struct PlatformPatch {
        id: i64,
        name: String,
        #[serde(default)]
        name_en: String,
        category: String,
        #[serde(default)]
        subtype: String,
        #[serde(default)]
        has_property: bool,
        #[serde(default)]
        has_personality: bool,
        #[serde(default)]
        inheritable: bool,
        #[serde(default)]
        required_documents: String,
        #[serde(default)]
        process_description: String,
        #[serde(default)]
        estimated_difficulty: String,
        #[serde(default)]
        official_policy_url: String,
        #[serde(default)]
        estimated_time_days: String,
        #[serde(default)]
        notes: String,
    }

    let parsed: PatchFile = serde_json::from_str(payload)
        .map_err(|e| format!("政策补丁解析失败（需为 JSON）: {}", e))?;
    if !parsed.app.is_empty() && parsed.app != "digilegacy" {
        return Err("不是归零政策库补丁（app 字段不匹配）".to_string());
    }
    if parsed.platforms.is_empty() {
        return Err("政策补丁中没有平台条目".to_string());
    }

    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("开启事务失败: {}", e))?;

    // 「新增 vs 覆盖」的判定要把已存在的补丁也算进去，
    // 否则重复导入同一份导出的补丁会被误报成全部新增。
    let existing_ids: std::collections::HashSet<i64> = {
        let mut stmt = tx
            .prepare("SELECT id FROM platforms UNION SELECT id FROM platform_overrides")
            .map_err(|e| format!("查询平台失败: {}", e))?;
        let ids: Vec<i64> = stmt
            .query_map([], |r| r.get::<_, i64>(0))
            .map_err(|e| format!("平台 id 读取失败: {}", e))?
            .filter_map(|r| r.ok())
            .collect();
        ids.into_iter().collect()
    };

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut added = 0usize;
    let mut updated = 0usize;
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO platform_overrides (id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
                 ON CONFLICT(id) DO UPDATE SET
                    name=excluded.name, name_en=excluded.name_en, category=excluded.category,
                    subtype=excluded.subtype, has_property=excluded.has_property,
                    has_personality=excluded.has_personality, inheritable=excluded.inheritable,
                    required_documents=excluded.required_documents,
                    process_description=excluded.process_description,
                    estimated_difficulty=excluded.estimated_difficulty,
                    official_policy_url=excluded.official_policy_url,
                    estimated_time_days=excluded.estimated_time_days,
                    notes=excluded.notes, updated_at=excluded.updated_at",
            )
            .map_err(|e| format!("准备补丁写入失败: {}", e))?;

        for p in &parsed.platforms {
            if p.id <= 0 {
                return Err(format!("平台 id 必须为正数（收到 {}）", p.id));
            }
            if p.name.trim().is_empty() || p.category.trim().is_empty() {
                return Err(format!("平台 {} 缺少名称或类别", p.id));
            }
            stmt.execute(params![
                p.id,
                p.name,
                p.name_en,
                p.category,
                p.subtype,
                if p.has_property { 1 } else { 0 },
                if p.has_personality { 1 } else { 0 },
                if p.inheritable { 1 } else { 0 },
                p.required_documents,
                p.process_description,
                p.estimated_difficulty,
                p.official_policy_url,
                p.estimated_time_days,
                p.notes,
                now,
            ])
            .map_err(|e| format!("写入政策补丁失败: {}", e))?;
            if existing_ids.contains(&p.id) {
                updated += 1
            } else {
                added += 1
            }
        }
    }
    tx.commit()
        .map_err(|e| format!("提交政策补丁失败: {}", e))?;

    // 补丁里「内置库没有」的平台只写进 platform_overrides，但 digital_assets 的外键
    // 指向 platforms 表——不镜像过去的话，界面上能选到、一登记资产就
    // "FOREIGN KEY constraint failed"（已由 repro_patch_only_platform_cannot_hold_assets 复现）。
    // 这里把补丁条目同步进 platforms 作为占位行，让外键成立；
    // 读取时 apply_platform_overrides 仍会用补丁内容覆盖展示。
    sync_platform_overrides_into_platforms(conn)?;

    Ok((added, updated))
}

/// 把 platform_overrides 里的条目镜像到 platforms 表（幂等）。
///
/// 只镜像外键所需的行；字段内容以补丁为准，所以这里写入的就是补丁值。
fn sync_platform_overrides_into_platforms(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "INSERT INTO platforms (id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes)
         SELECT id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes
         FROM platform_overrides
         WHERE id NOT IN (SELECT id FROM platforms)
         ON CONFLICT(id) DO NOTHING",
        [],
    )
    .map_err(|e| format!("同步政策补丁平台失败: {}", e))?;
    Ok(())
}

/// 导出当前全部政策补丁（供用户备份或分发给他人）。
pub fn export_platform_overrides_db() -> Result<String, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn
        .prepare("SELECT id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes FROM platform_overrides ORDER BY id")
        .map_err(|e| format!("查询政策补丁失败: {}", e))?;
    let rows: Vec<serde_json::Value> = stmt
        .query_map([], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "name_en": r.get::<_, String>(2)?,
                "category": r.get::<_, String>(3)?,
                "subtype": r.get::<_, String>(4)?,
                "has_property": r.get::<_, i64>(5)? != 0,
                "has_personality": r.get::<_, i64>(6)? != 0,
                "inheritable": r.get::<_, i64>(7)? != 0,
                "required_documents": r.get::<_, String>(8)?,
                "process_description": r.get::<_, String>(9)?,
                "estimated_difficulty": r.get::<_, String>(10)?,
                "official_policy_url": r.get::<_, String>(11)?,
                "estimated_time_days": r.get::<_, String>(12)?,
                "notes": r.get::<_, String>(13)?,
            }))
        })
        .map_err(|e| format!("政策补丁解析失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("政策补丁收集失败: {}", e))?;

    serde_json::to_string_pretty(&serde_json::json!({
        "app": "digilegacy",
        "kind": "platform-policy-patch",
        "version": 1,
        "exported_at": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "platforms": rows,
    }))
    .map_err(|e| format!("序列化政策补丁失败: {}", e))
}

/// 清除全部政策补丁，回到内置库状态。
///
/// 必须同时撤掉 `sync_platform_overrides_into_platforms` 写入的镜像行，
/// 否则「清除补丁」后平台列表仍留着补丁新增的平台（已由场景矩阵抓到）。
/// 内置平台 id 由 `platform_data` 决定，这里直接删「不在内置集合里」的行，
/// 并把被补丁覆盖过的内置行恢复为内置值。
pub fn clear_platform_overrides_db() -> Result<usize, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let n = conn
        .execute("DELETE FROM platform_overrides", [])
        .map_err(|e| format!("清除政策补丁失败: {}", e))?;

    let builtin = get_all_platforms();
    let builtin_ids: std::collections::HashSet<i64> = builtin.iter().map(|p| p.id).collect();
    let mut stmt = conn
        .prepare("SELECT id FROM platforms")
        .map_err(|e| format!("查询平台失败: {}", e))?;
    let extra: Vec<i64> = stmt
        .query_map([], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("读取平台 id 失败: {}", e))?
        .filter_map(|r| r.ok())
        .filter(|id| !builtin_ids.contains(id))
        .collect();
    drop(stmt);
    for id in extra {
        // 仍有资产挂在这些补丁平台上时不能直接删父行（外键会拦），
        // 也不该连资产一起删。保留一行「已归档」的平台占位，等用户自己处理
        // 这些资产（改挂到内置平台或删除），占用方是用户数据，不是补丁。
        let in_use: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM digital_assets WHERE platform_id=?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if in_use > 0 {
            // 改名标注来源，避免用户以为它还是可用平台
            conn.execute(
                "UPDATE platforms SET name = name || '（补丁已清除）' WHERE id=?1",
                params![id],
            )
            .map_err(|e| format!("标记补丁平台失败: {}", e))?;
            continue;
        }
        conn.execute("DELETE FROM platforms WHERE id=?1", params![id])
            .map_err(|e| format!("清理补丁镜像平台失败: {}", e))?;
    }
    // 被补丁覆盖过的内置行恢复为内置内容（重新 upsert 一次即可）
    crate::db::sync_platforms(conn)?;

    Ok(n)
}

/// 当前补丁条目数（界面显示用）。
pub fn count_platform_overrides_db() -> Result<i64, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.query_row("SELECT COUNT(*) FROM platform_overrides", [], |r| r.get(0))
        .map_err(|e| format!("统计政策补丁失败: {}", e))
}
