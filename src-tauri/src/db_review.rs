// 审核记录持久化 + 场景侧数据源（通用，不含业务名词）。
//
// 职责边界：
// - 本模块只管「读写审核留痕」与「列出某场景的待审条目」；
// - 判定如何产生由 `review.rs` 的状态机决定；
// - 某个具体场景要审什么，由 `review_profiles.rs` 声明。
//
// 这样新增场景（法律文书、家人说明、资产分类…）不需要动这里。

use chrono::Local;
use rusqlite::params;

use crate::db::get_conn;
use crate::review::{AiVerdict, HumanVerdict, ReviewItem, ReviewRecord, ReviewStatus};

/// 某场景下某条目的当前状态（由最新一条记录 + 内容指纹决定）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ItemState {
    pub key: String,
    pub title: String,
    /// 待审内容摘要：审核员必须能看见「审的是什么」，
    /// 否则界面只剩一个标题，实际要靠脑补。
    pub preview: String,
    pub status: ReviewStatus,
    /// 最新记录的 AI 结论与说明（没有记录时为 None）
    pub ai_verdict: Option<AiVerdict>,
    pub ai_note: String,
    pub human_note: String,
    /// 最近一次**人工**审核时间。
    /// 为空表示还没有人看过——AI 跑过但没有人工结论时这里必须为空，
    /// 否则界面会把「机器扫过」显示成「已审核」。
    pub reviewed_at: Option<String>,
    /// 记录是否因内容变化而失效
    pub stale: bool,
}

/// 内容摘要：截断到 300 字，保留换行为「 · 」便于单行显示。
fn preview_of(payload: &str) -> String {
    let flat: String = payload
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    if flat.chars().count() <= 300 {
        flat
    } else {
        let cut: String = flat.chars().take(300).collect();
        format!("{}…", cut)
    }
}

fn map_ai(s: &str) -> AiVerdict {
    match s {
        "pass" => AiVerdict::Pass,
        "flag" => AiVerdict::Flag,
        "skipped" => AiVerdict::Skipped,
        _ => AiVerdict::Unknown,
    }
}

fn map_human(s: Option<String>) -> Option<HumanVerdict> {
    match s.as_deref() {
        Some("approved") => Some(HumanVerdict::Approved),
        Some("rejected") => Some(HumanVerdict::Rejected),
        Some("needs_edit") => Some(HumanVerdict::NeedsEdit),
        _ => None,
    }
}

fn ai_str(v: &AiVerdict) -> &'static str {
    match v {
        AiVerdict::Pass => "pass",
        AiVerdict::Flag => "flag",
        AiVerdict::Unknown => "unknown",
        AiVerdict::Skipped => "skipped",
    }
}

fn human_str(v: &HumanVerdict) -> &'static str {
    match v {
        HumanVerdict::Approved => "approved",
        HumanVerdict::Rejected => "rejected",
        HumanVerdict::NeedsEdit => "needs_edit",
    }
}

/// 追加一条审核记录（只插不改）。
pub fn append_review_record_db(record: &ReviewRecord) -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    crate::db::ensure_vault_writable_pub(conn)?;
    conn.execute(
        "INSERT INTO review_records (profile_id, item_key, fingerprint, ai_verdict, ai_note, human_verdict, human_note, decided_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            record.profile_id,
            record.item_key,
            record.fingerprint,
            ai_str(&record.ai_verdict),
            record.ai_note,
            record.human_verdict.as_ref().map(human_str),
            record.human_note,
            record.decided_by,
            if record.created_at.is_empty() {
                Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                record.created_at.clone()
            },
        ],
    )
    .map_err(|e| format!("写入审核记录失败: {}", e))?;
    Ok(())
}

/// 读取某场景某条目的全部历史（新的在前）。
pub fn list_review_records_db(
    profile_id: &str,
    item_key: &str,
) -> Result<Vec<ReviewRecord>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    crate::db::ensure_vault_readable_pub(conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT profile_id, item_key, fingerprint, ai_verdict, ai_note, human_verdict, human_note, decided_by, created_at
             FROM review_records WHERE profile_id=?1 AND item_key=?2 ORDER BY id DESC",
        )
        .map_err(|e| format!("查询审核记录失败: {}", e))?;
    let rows = stmt
        .query_map(params![profile_id, item_key], |r| {
            Ok(ReviewRecord {
                profile_id: r.get(0)?,
                item_key: r.get(1)?,
                fingerprint: r.get(2)?,
                ai_verdict: map_ai(&r.get::<_, String>(3)?),
                ai_note: r.get::<_, String>(4)?,
                human_verdict: map_human(r.get::<_, Option<String>>(5)?),
                human_note: r.get::<_, String>(6)?,
                decided_by: r.get::<_, String>(7)?,
                created_at: r.get::<_, String>(8)?,
            })
        })
        .map_err(|e| format!("读取审核记录失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集审核记录失败: {}", e))?;
    Ok(rows)
}

/// 汇总一组条目的当前状态。
///
/// `items` 由场景提供（含内容指纹），本函数只负责与历史记录对账。
pub fn summarise_states_db(
    profile_id: &str,
    items: &[ReviewItem],
) -> Result<Vec<ItemState>, String> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let records = list_review_records_db(profile_id, &item.key)?;
        let latest = records.first();
        // 判定是否失效统一走 review::is_record_current，
        // 避免「这里比较 fingerprint、那里比较别的东西」两套口径。
        let stale = latest
            .map(|r| !crate::review::is_record_current(r, &item.fingerprint))
            .unwrap_or(false);
        let (status, ai_verdict, ai_note, human_note, reviewed_at) = match latest {
            None => (
                ReviewStatus::Pending,
                None,
                String::new(),
                String::new(),
                None,
            ),
            Some(r) if stale => (
                // 内容已变：旧判定作废，回到待审
                ReviewStatus::Pending,
                Some(r.ai_verdict.clone()),
                r.ai_note.clone(),
                r.human_note.clone(),
                // 只有人工结论才算「审核过」；AI 跑过不算
                r.human_verdict.as_ref().map(|_| r.created_at.clone()),
            ),
            Some(r) => (
                ReviewStatus::from_verdicts(&r.ai_verdict, r.human_verdict.as_ref()),
                Some(r.ai_verdict.clone()),
                r.ai_note.clone(),
                r.human_note.clone(),
                r.human_verdict.as_ref().map(|_| r.created_at.clone()),
            ),
        };
        out.push(ItemState {
            key: item.key.clone(),
            title: item.title.clone(),
            preview: preview_of(&item.payload),
            status,
            ai_verdict,
            ai_note,
            human_note,
            reviewed_at,
            stale,
        });
    }
    Ok(out)
}

/// 统计某场景各状态数量（界面摘要用）。
pub fn review_counts_db(
    profile_id: &str,
    items: &[ReviewItem],
) -> Result<(usize, usize, usize), String> {
    let states = summarise_states_db(profile_id, items)?;
    let approved = states
        .iter()
        .filter(|s| s.status == ReviewStatus::Approved)
        .count();
    let awaiting = states
        .iter()
        .filter(|s| s.status == ReviewStatus::AwaitingHuman)
        .count();
    let pending = states
        .iter()
        .filter(|s| s.status == ReviewStatus::Pending)
        .count();
    Ok((approved, awaiting, pending))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::{fingerprint, AiVerdict, HumanVerdict};

    fn fresh(tag: &str) {
        crate::db::init_database_at(std::env::temp_dir().join(format!(
            "digilegacy_review_{}_{}.db",
            tag,
            std::process::id()
        )))
        .unwrap();
    }

    fn item(payload: &str) -> ReviewItem {
        ReviewItem {
            key: "k".to_string(),
            title: "条目".to_string(),
            payload: payload.to_string(),
            fingerprint: fingerprint(payload),
        }
    }

    fn record(ai: AiVerdict, human: Option<HumanVerdict>, fp: &str) -> ReviewRecord {
        ReviewRecord {
            profile_id: "p".to_string(),
            item_key: "k".to_string(),
            fingerprint: fp.to_string(),
            ai_verdict: ai,
            ai_note: String::new(),
            human_verdict: human,
            human_note: String::new(),
            decided_by: "test".to_string(),
            created_at: String::new(),
        }
    }

    /// AI 跑过但没有人工结论 → 状态「待人工」，且**不显示审核时间**。
    #[test]
    fn ai_only_record_is_not_reviewed_yet() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        fresh("ai_only");
        let it = item("内容");
        append_review_record_db(&record(AiVerdict::Pass, None, &it.fingerprint)).unwrap();
        let states = summarise_states_db("p", &[it]).unwrap();
        assert_eq!(states[0].status, ReviewStatus::AwaitingHuman);
        assert!(
            states[0].reviewed_at.is_none(),
            "AI 跑过不等于审核过，不能显示审核时间"
        );
    }

    /// 人工结论写入后状态与时间才成立。
    #[test]
    fn human_decision_sets_status_and_time() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        fresh("human");
        let it = item("内容");
        append_review_record_db(&record(
            AiVerdict::Pass,
            Some(HumanVerdict::Approved),
            &it.fingerprint,
        ))
        .unwrap();
        let states = summarise_states_db("p", &[it]).unwrap();
        assert_eq!(states[0].status, ReviewStatus::Approved);
        assert!(states[0].reviewed_at.is_some());
    }

    /// 内容变化 → 状态回到待审，并标 stale。
    #[test]
    fn content_change_invalidates_decision() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        fresh("stale");
        let old = item("旧内容");
        append_review_record_db(&record(
            AiVerdict::Pass,
            Some(HumanVerdict::Approved),
            &old.fingerprint,
        ))
        .unwrap();
        let new = item("新内容");
        let states = summarise_states_db("p", &[new]).unwrap();
        assert_eq!(states[0].status, ReviewStatus::Pending);
        assert!(states[0].stale, "内容变化必须标记为需重审");
    }

    /// 留痕只增不改：同一 key 多次审核必须全部保留。
    #[test]
    fn audit_trail_is_append_only() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        fresh("append");
        let it = item("内容");
        append_review_record_db(&record(AiVerdict::Flag, None, &it.fingerprint)).unwrap();
        append_review_record_db(&record(
            AiVerdict::Flag,
            Some(HumanVerdict::Approved),
            &it.fingerprint,
        ))
        .unwrap();
        let all = list_review_records_db("p", "k").unwrap();
        assert_eq!(all.len(), 2, "两条记录都必须保留");
        // 新的在前
        assert_eq!(all[0].human_verdict, Some(HumanVerdict::Approved));
    }

    /// 进度统计要把「已通过 / 待人工 / 待审」分开，不能合成一个数字。
    #[test]
    fn counts_split_by_status() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        fresh("counts");
        // 注意：条目 key 必须与记录里的 item_key 一致，
        // 否则查询按 key 定位不到记录（第一版测试就是这么写错的）。
        let mk = |key: &str| ReviewItem {
            key: key.to_string(),
            title: key.to_string(),
            payload: key.to_string(),
            fingerprint: fingerprint(key),
        };
        let a = mk("a");
        let b = mk("b");
        let c = mk("c");
        append_review_record_db(&ReviewRecord {
            profile_id: "p".to_string(),
            item_key: "a".to_string(),
            ..record(
                AiVerdict::Pass,
                Some(HumanVerdict::Approved),
                &a.fingerprint,
            )
        })
        .unwrap();
        append_review_record_db(&ReviewRecord {
            profile_id: "p".to_string(),
            item_key: "b".to_string(),
            ..record(AiVerdict::Pass, None, &b.fingerprint)
        })
        .unwrap();
        let (approved, awaiting, pending) = review_counts_db("p", &[a, b, c]).unwrap();
        assert_eq!((approved, awaiting, pending), (1, 1, 1));
    }
}
