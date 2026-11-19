// Custody（保管位置）读写。
//
// 为什么单独成文件而不塞进 db.rs：db.rs 已经 2.1k 行，而这是一块独立、
// 边界清晰的领域（两个加密字段 + 一张单行表）。风格与 db_platforms.rs 一致。

use chrono::Local;
use rusqlite::params;

use crate::db::get_conn;

/// 保管位置（解密后）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CustodyHints {
    /// 加密备份文件放在哪（如「家中保险柜 / 银行保管箱」）。
    pub backup_location: String,
    /// 恢复码抄在哪（如「纸质笔记本第 3 页」）。
    pub recovery_code_location: String,
}

fn ensure_writable(conn: &rusqlite::Connection) -> Result<(), String> {
    // 复用 db 模块的写入门禁：锁定态不得写入（否则明文直通）。
    crate::db::ensure_vault_writable_pub(conn)
}

pub fn get_custody_hints_db() -> Result<CustodyHints, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let row = conn.query_row(
        "SELECT COALESCE(backup_location_enc,''), COALESCE(recovery_code_location_enc,'') FROM custody_config WHERE id=1",
        [],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    );
    match row {
        Ok((b, r)) => Ok(CustodyHints {
            backup_location: crate::vault::decrypt_or_placeholder(&b),
            recovery_code_location: crate::vault::decrypt_or_placeholder(&r),
        }),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(CustodyHints::default()),
        Err(e) => Err(format!("读取保管位置失败: {}", e)),
    }
}

pub fn set_custody_hints_db(hints: &CustodyHints) -> Result<(), String> {
    // 与资产/档案同一守卫：锁定态写入会让 encrypt_if_needed 静默明文直通。
    {
        let guard = get_conn()?;
        let conn = guard.as_ref().unwrap();
        ensure_writable(conn)?;
    }
    let backup = crate::vault::encrypt_if_needed(&hints.backup_location)?;
    let recovery = crate::vault::encrypt_if_needed(&hints.recovery_code_location)?;
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO custody_config (id, backup_location_enc, recovery_code_location_enc, updated_at)
         VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET backup_location_enc=?1, recovery_code_location_enc=?2, updated_at=?3",
        params![backup, recovery, now],
    )
    .map_err(|e| format!("保存保管位置失败: {}", e))?;
    Ok(())
}
