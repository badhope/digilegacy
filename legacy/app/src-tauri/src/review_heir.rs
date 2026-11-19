//! 新审核场景：「继承人读得懂」（heir-comprehension）。
//!
//! 定位：这是把 AI 与产品核心承诺（「给家人工具包而不是谜题」）直接挂钩的场景。
//! 政策核验审「数据对不对」，这个场景审「家人能不能照着做」——审的对象是
//! 实际会交到家人手里的两份产物：
//! 1. **家人执行包**（action guide）：按平台分组的行动清单；
//! 2. **继承报告正文**（report）：结构化的资产/政策/权威来源汇总。
//!
//! 审什么（六项，全部是「家人视角」的可判读标准）：
//! - 每一条资产是否都有「找谁 / 带什么 / 做什么」三要素；
//! - 是否出现只有开发者才懂的词（占位符、IPC、KDF、指纹……）；
//! - 步骤之间是否互相依赖却没说明先后顺序；
//! - 是否给出无法兑现的时间承诺；
//! - 是否泄露凭据/完整账号（执行包允许有账号标识，但绝不能有密码）；
//! - 是否暗示「本清单可替代公证/律师」。
//!
//! 与既有场景的关系：family_note 审「用户手填的几段文字」，本场景审
//! 「系统生成的完整交付物」——粒度更大，因此用 Strict 模式逐项作答。

use crate::models::InheritanceReport;
use crate::review::{fingerprint, ReviewItem, ReviewProfile};

pub const HEIR_COMPREHENSION_REVIEW: &str = "heir-comprehension";

const STRICT_CHECKS: &[(&str, &str)] = &[
    (
        "next-step",
        "每条资产都有可执行的下一步（找谁/带什么/做什么）",
    ),
    ("jargon", "无只有开发者才懂的术语或内部占位符"),
    ("ordering", "有先后依赖的步骤已标注顺序"),
    ("promise", "无无法兑现的时间承诺"),
    ("secret", "无凭据或密码明文"),
    ("disclaimer", "未暗示可替代公证/律师/平台审核"),
];

pub fn heir_comprehension_profile() -> ReviewProfile {
    ReviewProfile {
        id: HEIR_COMPREHENSION_REVIEW.to_string(),
        title_key: "review.profile.heirComprehension".to_string(),
        criteria: "假设读者是一位刚失去亲人、第一次处理数字遗产的普通家人。逐项检查：
1) 每条资产是否都写清了「找谁（客服/表单/线下）、带什么（证明材料）、做什么（第一步动作）」；
2) 是否出现家人无法理解的术语：占位符、KDF、加密指纹、IPC、schema 等；
3) 有先后依赖的步骤（如先办公证再联系平台）是否标明了顺序；
4) 是否出现无法兑现的时间承诺（如「3 天办完」）；
5) 是否出现密码、恢复码、完整凭据的明文；
6) 是否暗示本清单可以替代公证处、律师或平台的人工审核。"
            .to_string(),
        use_ai: true,
        strictness: crate::review::ReviewStrictness::Strict,
        strict_checks: STRICT_CHECKS
            .iter()
            .map(|(id, label)| crate::review::StrictCheck {
                id: id.to_string(),
                label: label.to_string(),
            })
            .collect(),
    }
}

/// 从报告生成本场景的送审条目。
///
/// 出境白名单遵守 ai_assist 的约束：只有平台名、处置方式、政策文案与
/// 「家人可见的说明」；凭据列（credential）绝不出现，账号标识以用户在
/// 执行包里本就会打印的形式出现（与 report.rs 的 asset_row 同一口径）。
pub fn heir_comprehension_items(report: &InheritanceReport) -> Vec<ReviewItem> {
    let mut items = Vec::new();

    // 条目 1：执行包正文（按平台分组的行动清单摘要）
    let mut guide = String::new();
    for a in &report.assets {
        let policy = report
            .policies
            .iter()
            .find(|p| p.platform_id == a.platform_id);
        guide.push_str(&format!(
            "【{}】处置：{}；继承人状态：{}\n所需材料：{}\n办理流程：{}\n难度：{}；预计时长：{}\n\n",
            a.platform_name,
            crate::report::disposal_label(&a.disposal_method, crate::report::ReportLocale::Zh),
            if a
                .heir_name
                .as_deref()
                .map(|v| !v.trim().is_empty() && !v.contains("需解锁"))
                .unwrap_or(false)
            {
                "已指定"
            } else {
                "未指定/未知"
            },
            policy
                .map(|p| p.required_documents.as_str())
                .unwrap_or("（无政策记录）"),
            policy.map(|p| p.process_description.as_str()).unwrap_or(""),
            policy
                .map(|p| p.estimated_difficulty.as_str())
                .unwrap_or(""),
            policy.map(|p| p.estimated_time_days.as_str()).unwrap_or(""),
        ));
    }
    if !guide.is_empty() {
        items.push(ReviewItem {
            key: "action-guide".to_string(),
            title: "家人执行包正文".to_string(),
            fingerprint: fingerprint(&guide),
            payload: guide,
        });
    }

    // 条目 2：继承人的分配说明汇总（分给谁、怎么分）
    if !report.heirs.is_empty() {
        let executor_count = report.heirs.iter().filter(|h| h.is_executor).count();
        let notes = report
            .heirs
            .iter()
            .filter(|h| !h.share_note.trim().is_empty() && !h.share_note.contains("需解锁"))
            .count();
        let heir_text = format!(
            "继承人数量：{}；执行人数量：{}；含分配说明：{}",
            report.heirs.len(),
            executor_count,
            notes
        );
        items.push(ReviewItem {
            key: "heir-plan".to_string(),
            title: "继承人分配说明".to_string(),
            fingerprint: fingerprint(&heir_text),
            payload: format!("继承人名单与分配说明：\n{}", heir_text),
        });
    }

    // 条目 3：给家人的开头说明（报告恒定生成；执行包不含称呼时家人会困惑这堆清单是谁的）
    {
        let text = "本清单由持有者整理。紧急联系人和恢复材料的位置请按执行包内的保管位置说明查找。"
            .to_string();
        items.push(ReviewItem {
            key: "opening-note".to_string(),
            title: "给家人的开头说明".to_string(),
            fingerprint: fingerprint(&text),
            payload: text,
        });
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn report() -> InheritanceReport {
        InheritanceReport {
            user: UserProfile {
                full_name: "张三".into(),
                emergency_contact_name: "李四".into(),
                emergency_contact_phone: "138".into(),
                ..Default::default()
            },
            assets: vec![DigitalAsset {
                id: Some(1),
                platform_id: 1,
                platform_name: "支付宝".into(),
                account_identifier: "acc-1".into(),
                category: "property".into(),
                disposal_method: "inherit".into(),
                heir_name: Some("李四".into()),
                ..Default::default()
            }],
            heirs: vec![Heir {
                name: "李四".into(),
                is_executor: true,
                share_note: "全部资产".into(),
                ..Default::default()
            }],
            policies: vec![PlatformPolicyBrief {
                platform_id: 1,
                name: "支付宝".into(),
                required_documents: "死亡证明".into(),
                process_description: "联系客服".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn profile_is_strict_with_six_checks() {
        let p = heir_comprehension_profile();
        assert_eq!(p.id, HEIR_COMPREHENSION_REVIEW);
        assert!(p.use_ai);
        assert_eq!(p.strictness, crate::review::ReviewStrictness::Strict);
        assert_eq!(p.strict_checks.len(), 6);
    }

    #[test]
    fn items_cover_guide_heirs_and_opening() {
        let items = heir_comprehension_items(&report());
        let keys: Vec<&str> = items.iter().map(|i| i.key.as_str()).collect();
        assert!(keys.contains(&"action-guide"), "必须送审执行包正文");
        assert!(keys.contains(&"heir-plan"), "必须送审分配说明");
        assert!(keys.contains(&"opening-note"), "必须送审开头说明");
    }

    /// 出境红线：凭据列绝不进入 payload；账号标识按执行包口径允许。
    #[test]
    fn payload_never_contains_credentials() {
        let mut r = report();
        r.assets[0].notes = "备注".into();
        let items = heir_comprehension_items(&r);
        for i in &items {
            assert!(
                !i.payload.contains("credential") && !i.payload.contains("凭据："),
                "payload 不得包含凭据字段: {}",
                i.key
            );
        }
        let guide = items.iter().find(|i| i.key == "action-guide").unwrap();
        assert!(guide.payload.contains("支付宝"));
        assert!(!guide.payload.contains("李四"));
        assert!(!guide.payload.contains("139"));
        let plan = items.iter().find(|i| i.key == "heir-plan").unwrap();
        assert!(!plan.payload.contains("李四"));
        assert!(!plan.payload.contains("全部资产"));
    }

    /// 指纹稳定性：同内容同指纹、内容变化指纹变（过期检测依赖这一点）。
    #[test]
    fn fingerprints_track_content_changes() {
        let r1 = report();
        let a = heir_comprehension_items(&r1);
        let b = heir_comprehension_items(&r1);
        assert_eq!(a[0].fingerprint, b[0].fingerprint);

        let mut r2 = report();
        r2.assets[0].disposal_method = "liquidate".into();
        let c = heir_comprehension_items(&r2);
        assert_ne!(a[0].fingerprint, c[0].fingerprint);
    }

    /// 场景注册后必须能被 profile_by_id 找到（管线发现机制的契约）。
    #[test]
    fn registered_in_profile_lookup() {
        // 注册动作在 review_profiles::all_profiles()，这里验证/items 路由。
        assert!(crate::review_profiles::items_for(HEIR_COMPREHENSION_REVIEW).is_some());
    }
}
