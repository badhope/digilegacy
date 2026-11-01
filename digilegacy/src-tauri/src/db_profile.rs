//! 用户档案读写（从 db.rs 拆出，纯移动）。
//!
//! 关键不变式：12 个敏感字段共用同一份列清单（`PROFILE_SENSITIVE_COLUMNS`），
//! 「启用加密 / 改密 / 恢复重设」三条重加密路径都必须走这份清单——
//! 历史上 `encrypt_existing_data_db` 自持少一列的副本，导致索引越界 panic。

use chrono::Local;
use rusqlite::{params, Connection};

use crate::models::*;

use super::{ensure_vault_writable, get_conn, reject_placeholder};

/// 档案中需要加密的字段，顺序即 SELECT / UPDATE 的参数顺序。
/// 「启用加密」「改密」「恢复重设」三条重加密路径共用同一份列清单：
/// 历史缺陷是 `encrypt_existing_data_db` 自持一份少了一列的副本，导致索引越界 panic。
pub(crate) const PROFILE_SENSITIVE_COLUMNS: [&str; 12] = [
    "full_name",
    "phone",
    "email",
    "id_last4",
    "emergency_contact_name",
    "emergency_contact_phone",
    "emergency_contact_relation",
    "emergency_contact_email",
    "heir_primary_name",
    "heir_primary_phone",
    "heir_primary_relation",
    "heir_primary_email",
];

/// 读取最新一条档案的 (id, 敏感字段原文)。无档案返回 None。
pub(crate) fn select_profile_sensitive(
    conn: &Connection,
) -> Result<Option<(i64, Vec<String>)>, String> {
    let sql = format!(
        "SELECT id, {} FROM user_profile ORDER BY id DESC LIMIT 1",
        PROFILE_SENSITIVE_COLUMNS.join(", ")
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询档案失败: {}", e))?;
    let row = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let mut vals = Vec::with_capacity(PROFILE_SENSITIVE_COLUMNS.len());
            for i in 1..=PROFILE_SENSITIVE_COLUMNS.len() {
                vals.push(row.get::<_, Option<String>>(i)?.unwrap_or_default());
            }
            Ok((id, vals))
        })
        .map_err(|e| format!("读取档案失败: {}", e))?
        .next()
        .transpose()
        .map_err(|e| format!("收集档案失败: {}", e))?;
    Ok(row)
}

/// 按下标顺序写回档案敏感字段，参数个数由列清单决定，不会与 UPDATE 占位符错位。
pub(crate) fn update_profile_sensitive(
    conn: &Connection,
    id: i64,
    vals: &[String],
) -> Result<(), String> {
    if vals.len() != PROFILE_SENSITIVE_COLUMNS.len() {
        return Err(format!(
            "档案字段数量不一致：期望 {}，实际 {}",
            PROFILE_SENSITIVE_COLUMNS.len(),
            vals.len()
        ));
    }
    let sql = format!(
        "UPDATE user_profile SET {} WHERE id=?{}",
        PROFILE_SENSITIVE_COLUMNS
            .iter()
            .enumerate()
            .map(|(i, col)| format!("{}=?{}", col, i + 1))
            .collect::<Vec<_>>()
            .join(", "),
        PROFILE_SENSITIVE_COLUMNS.len() + 1
    );
    let mut params_vec: Vec<&dyn rusqlite::ToSql> =
        vals.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    params_vec.push(&id);
    conn.execute(&sql, params_vec.as_slice())
        .map_err(|e| format!("写入档案失败: {}", e))?;
    Ok(())
}

pub fn get_user_profile_db() -> Result<Option<UserProfile>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn.prepare("SELECT id, full_name, phone, email, id_last4, emergency_contact_name, emergency_contact_phone, emergency_contact_relation, emergency_contact_email, heir_primary_name, heir_primary_phone, heir_primary_relation, heir_primary_email, check_in_interval_days, last_check_in, dms_enabled, created_at, updated_at FROM user_profile ORDER BY id DESC LIMIT 1")
        .map_err(|e| format!("查询用户档案失败: {}", e))?;

    let mut rows = stmt.query([]).map_err(|e| format!("查询执行失败: {}", e))?;

    if let Some(row) = rows.next().map_err(|e| format!("读取失败: {}", e))? {
        let dec = |idx: usize| -> String {
            let raw: String = row
                .get::<_, Option<String>>(idx)
                .ok()
                .flatten()
                .unwrap_or_default();
            crate::vault::decrypt_or_placeholder(&raw)
        };
        Ok(Some(UserProfile {
            id: row.get(0).map_err(|e| format!("{}", e))?,
            full_name: dec(1),
            phone: dec(2),
            email: dec(3),
            id_last4: dec(4),
            emergency_contact_name: dec(5),
            emergency_contact_phone: dec(6),
            emergency_contact_relation: dec(7),
            emergency_contact_email: dec(8),
            heir_primary_name: dec(9),
            heir_primary_phone: dec(10),
            heir_primary_relation: dec(11),
            heir_primary_email: dec(12),
            check_in_interval_days: row.get(13).map_err(|e| format!("{}", e))?,
            last_check_in: row.get(14).ok(),
            dms_enabled: row.get::<_, i64>(15).map_err(|e| format!("{}", e))? != 0,
            created_at: row.get(16).map_err(|e| format!("{}", e))?,
            updated_at: row.get(17).map_err(|e| format!("{}", e))?,
        }))
    } else {
        Ok(None)
    }
}

pub fn save_user_profile_db(profile: &UserProfile) -> Result<(), String> {
    for (name, value) in [
        ("姓名", profile.full_name.as_str()),
        ("手机号", profile.phone.as_str()),
        ("邮箱", profile.email.as_str()),
        ("身份证后四位", profile.id_last4.as_str()),
        ("紧急联系人姓名", profile.emergency_contact_name.as_str()),
        ("紧急联系人电话", profile.emergency_contact_phone.as_str()),
        (
            "紧急联系人关系",
            profile.emergency_contact_relation.as_str(),
        ),
        ("紧急联系人邮箱", profile.emergency_contact_email.as_str()),
        ("继承人姓名", profile.heir_primary_name.as_str()),
        ("继承人电话", profile.heir_primary_phone.as_str()),
        ("继承人关系", profile.heir_primary_relation.as_str()),
        ("继承人邮箱", profile.heir_primary_email.as_str()),
    ] {
        reject_placeholder(name, value)?;
    }
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    ensure_vault_writable(conn)?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let full_name = crate::vault::encrypt_if_needed(&profile.full_name)?;
    let phone = crate::vault::encrypt_if_needed(&profile.phone)?;
    let email = crate::vault::encrypt_if_needed(&profile.email)?;
    let id_last4 = crate::vault::encrypt_if_needed(&profile.id_last4)?;
    let ec_name = crate::vault::encrypt_if_needed(&profile.emergency_contact_name)?;
    let ec_phone = crate::vault::encrypt_if_needed(&profile.emergency_contact_phone)?;
    let ec_rel = crate::vault::encrypt_if_needed(&profile.emergency_contact_relation)?;
    let ec_email = crate::vault::encrypt_if_needed(&profile.emergency_contact_email)?;
    let h_name = crate::vault::encrypt_if_needed(&profile.heir_primary_name)?;
    let h_phone = crate::vault::encrypt_if_needed(&profile.heir_primary_phone)?;
    let h_rel = crate::vault::encrypt_if_needed(&profile.heir_primary_relation)?;
    let h_email = crate::vault::encrypt_if_needed(&profile.heir_primary_email)?;

    match profile.id {
        Some(id) => {
            conn.execute(
                "UPDATE user_profile SET full_name=?1, phone=?2, email=?3, id_last4=?4, emergency_contact_name=?5, emergency_contact_phone=?6, emergency_contact_relation=?7, emergency_contact_email=?8, heir_primary_name=?9, heir_primary_phone=?10, heir_primary_relation=?11, heir_primary_email=?12, check_in_interval_days=?13, dms_enabled=?14, updated_at=?15 WHERE id=?16",
                params![
                    full_name, phone, email, id_last4,
                    ec_name, ec_phone, ec_rel, ec_email,
                    h_name, h_phone, h_rel, h_email,
                    profile.check_in_interval_days,
                    if profile.dms_enabled { 1 } else { 0 },
                    now, id,
                ],
            ).map_err(|e| format!("更新档案失败: {}", e))?;
        }
        None => {
            conn.execute(
                "INSERT INTO user_profile (full_name, phone, email, id_last4, emergency_contact_name, emergency_contact_phone, emergency_contact_relation, emergency_contact_email, heir_primary_name, heir_primary_phone, heir_primary_relation, heir_primary_email, check_in_interval_days, dms_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
                params![
                    full_name, phone, email, id_last4,
                    ec_name, ec_phone, ec_rel, ec_email,
                    h_name, h_phone, h_rel, h_email,
                    profile.check_in_interval_days,
                    if profile.dms_enabled { 1 } else { 0 },
                    now,
                ],
            ).map_err(|e| format!("保存档案失败: {}", e))?;
        }
    }

    Ok(())
}
