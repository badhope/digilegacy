// 继承人名单：多名继承人 + 独立执行人。
//
// 为什么单独成文件：继承人数据有两条耦合约束，放在 db.rs 里容易被别处的
// 重加密逻辑漏掉——
// 1. 所有字段都加密（姓名/电话/邮箱/关系/分配说明）；
// 2. 必须恰好有一位执行人（DMS 第三级通知要有明确的收件人）。
//
// 写入采用「整组替换」而不是逐条 CRUD：家庭成员关系是整体，
// 逐条增删会在中途留下「两个执行人」或「没有执行人」的非法状态。

use chrono::Local;
use rusqlite::params;

use crate::db::get_conn;
use crate::models::Heir;

/// 名单上限：超过 20 位既不符合现实，也让报告无法阅读。
pub const MAX_HEIRS: usize = 20;

fn ensure_writable(conn: &rusqlite::Connection) -> Result<(), String> {
    crate::db::ensure_vault_writable_pub(conn)
}

/// 读取全部继承人（按 sort_order 升序）。
pub fn get_heirs_db() -> Result<Vec<Heir>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT id, COALESCE(name,''), COALESCE(relation,''), COALESCE(phone,''),
                    COALESCE(email,''), COALESCE(share_note,''), is_executor, sort_order
             FROM heirs ORDER BY sort_order, id",
        )
        .map_err(|e| format!("查询继承人失败: {}", e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Heir {
                id: Some(r.get::<_, i64>(0)?),
                name: crate::vault::decrypt_or_placeholder(&r.get::<_, String>(1)?),
                relation: crate::vault::decrypt_or_placeholder(&r.get::<_, String>(2)?),
                phone: crate::vault::decrypt_or_placeholder(&r.get::<_, String>(3)?),
                email: crate::vault::decrypt_or_placeholder(&r.get::<_, String>(4)?),
                share_note: crate::vault::decrypt_or_placeholder(&r.get::<_, String>(5)?),
                is_executor: r.get::<_, i64>(6)? != 0,
                sort_order: r.get::<_, i64>(7)?,
            })
        })
        .map_err(|e| format!("读取继承人失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集继承人失败: {}", e))?;
    Ok(rows)
}

/// 整体替换继承人名单。
///
/// 校验顺序刻意如此：
/// 1. 先查占位符（锁定态读出来的值不能被写回）；
/// 2. 再查数量与非空姓名；
/// 3. 最后查「恰好一位执行人」——空名单允许（表示还没填），
///    但一旦有名单就必须有且只有一个执行人，否则 DMS 第三级没有收件人。
pub fn replace_heirs_db(heirs: &[Heir]) -> Result<(), String> {
    if heirs.len() > MAX_HEIRS {
        return Err(format!("继承人最多 {} 位", MAX_HEIRS));
    }
    for h in heirs {
        reject("继承人姓名", &h.name)?;
        reject("与本人关系", &h.relation)?;
        reject("联系电话", &h.phone)?;
        reject("邮箱", &h.email)?;
        reject("分配说明", &h.share_note)?;
        if h.name.trim().is_empty() {
            return Err("继承人姓名不能为空".to_string());
        }
        if h.name.chars().count() > 60
            || h.relation.chars().count() > 40
            || h.phone.chars().count() > 40
            || h.email.chars().count() > 200
            || h.share_note.chars().count() > 500
        {
            return Err(
                "继承人字段超长（姓名 60 / 关系 40 / 电话 40 / 邮箱 200 / 说明 500）".to_string(),
            );
        }
    }
    if !heirs.is_empty() {
        let executors = heirs.iter().filter(|h| h.is_executor).count();
        if executors != 1 {
            return Err(format!(
                "必须恰好指定一位遗产执行人（当前 {} 位）",
                executors
            ));
        }
    }

    {
        let guard = get_conn()?;
        let conn = guard.as_ref().unwrap();
        ensure_writable(conn)?;
    }

    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| format!("开启事务失败: {}", e))?;
    let result = (|| -> Result<(), String> {
        conn.execute("DELETE FROM heirs", [])
            .map_err(|e| format!("清空继承人失败: {}", e))?;
        for (idx, h) in heirs.iter().enumerate() {
            conn.execute(
                "INSERT INTO heirs (name, relation, phone, email, share_note, is_executor, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    crate::vault::encrypt_if_needed(&h.name)?,
                    crate::vault::encrypt_if_needed(&h.relation)?,
                    crate::vault::encrypt_if_needed(&h.phone)?,
                    crate::vault::encrypt_if_needed(&h.email)?,
                    crate::vault::encrypt_if_needed(&h.share_note)?,
                    h.is_executor as i64,
                    idx as i64,
                    now,
                ],
            )
            .map_err(|e| format!("写入继承人失败: {}", e))?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| format!("提交失败: {}", e))?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

fn reject(field: &str, value: &str) -> Result<(), String> {
    if crate::vault::is_placeholder(value) {
        return Err(format!(
            "{}当前处于加密锁定状态，无法保存。请先解锁后重试（本次未写入任何数据）",
            field
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::vault;

    fn fresh(tag: &str) {
        db::init_database_at(std::env::temp_dir().join(format!(
            "digilegacy_heirs_{}_{}.db",
            tag,
            std::process::id()
        )))
        .unwrap();
    }

    fn heir(name: &str, executor: bool) -> Heir {
        Heir {
            id: None,
            name: name.to_string(),
            relation: "子女".to_string(),
            phone: String::new(),
            email: String::new(),
            share_note: String::new(),
            is_executor: executor,
            sort_order: 0,
        }
    }

    #[test]
    fn roundtrip_multiple_heirs_in_order() {
        let _g = vault::test_lock();
        vault::clear_key();
        fresh("roundtrip");
        replace_heirs_db(&[heir("王五", true), heir("王六", false), heir("王七", false)]).unwrap();
        let got = get_heirs_db().unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].name, "王五");
        assert!(got[0].is_executor, "第一位必须是执行人");
        assert_eq!(
            got.iter().filter(|h| h.is_executor).count(),
            1,
            "执行人必须唯一"
        );
        assert_eq!(got[2].name, "王七", "顺序必须保持");
    }

    /// 执行人必须恰好一位：0 位则 DMS 第三级没有收件人，2 位则不知道听谁的。
    #[test]
    fn requires_exactly_one_executor() {
        let _g = vault::test_lock();
        vault::clear_key();
        fresh("executor");
        let e = replace_heirs_db(&[heir("甲", false), heir("乙", false)]).unwrap_err();
        assert!(e.contains("恰好"), "实际: {}", e);
        let e = replace_heirs_db(&[heir("甲", true), heir("乙", true)]).unwrap_err();
        assert!(e.contains("恰好"), "实际: {}", e);
        // 空名单允许（表示尚未填写）
        assert!(replace_heirs_db(&[]).is_ok());
    }

    #[test]
    fn rejects_empty_name_and_oversized_list() {
        let _g = vault::test_lock();
        vault::clear_key();
        fresh("validate");
        assert!(replace_heirs_db(&[heir("   ", true)]).is_err());
        let many: Vec<Heir> = (0..MAX_HEIRS + 1)
            .map(|i| heir(&format!("继承人{}", i), i == 0))
            .collect();
        let e = replace_heirs_db(&many).unwrap_err();
        assert!(e.contains("最多"), "实际: {}", e);
    }

    /// 占位符写回是历史 P0 同类缺陷：锁定态读出的值绝不能存回去。
    #[test]
    fn rejects_placeholder() {
        let _g = vault::test_lock();
        vault::clear_key();
        fresh("placeholder");
        let mut h = heir("王五", true);
        h.name = vault::PLACEHOLDER_LOCKED.to_string();
        let e = replace_heirs_db(&[h]).unwrap_err();
        assert!(e.contains("解锁"), "实际: {}", e);
    }

    /// 整组替换必须原子：一次失败的写入不能留下半份名单。
    #[test]
    fn replace_is_atomic() {
        let _g = vault::test_lock();
        vault::clear_key();
        fresh("atomic");
        replace_heirs_db(&[heir("原名单", true)]).unwrap();
        // 第二次写入非法（无执行人）→ 必须整体回滚，原名单保持
        let e = replace_heirs_db(&[heir("新名单", false)]).unwrap_err();
        assert!(e.contains("恰好"));
        let got = get_heirs_db().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "原名单", "失败的写入不得破坏原名单");
    }
}
