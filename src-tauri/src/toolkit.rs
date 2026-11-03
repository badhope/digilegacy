// 家人执行包与演练。
//
// 为什么单独成文件：这是「产品定位」的落点——把持有者的规划转成家人能照着做的动作。
// 逻辑独立于报告渲染（report.rs）与命令层，便于单测。

use crate::action_guide::render_action_guide_html;
use crate::models::InheritanceReport;
use crate::report::ReportLocale;

/// 执行包内容。
///
/// 只返回 HTML：落盘文件名由命令层统一生成（与报告导出共用同一命名策略），
/// 避免「界面显示的名字」和「实际写盘的名字」两处各算一遍。
#[derive(Debug)]
pub struct ActionGuide {
    pub html: String,
}

/// 生成家人执行包。
///
/// 与报告共用 `contains_placeholder` 的拒绝策略：含未解密字段时不出包，
/// 否则家人会拿到一份写着「需解锁」的清单。
pub fn build_action_guide(
    report: &InheritanceReport,
    locale: ReportLocale,
    backup_location: &str,
    recovery_location: &str,
) -> Result<ActionGuide, String> {
    if crate::report::contains_placeholder(report) {
        return Err("报告中存在未解密字段，请先解锁后重新生成（本次未生成任何文件）".to_string());
    }
    let html = render_action_guide_html(report, locale, backup_location, recovery_location);
    Ok(ActionGuide { html })
}

/// 演练结果：不解密真实凭据，只用当前元数据走一遍「家人会看到什么」。
#[derive(Debug, serde::Serialize)]
pub struct DrillReport {
    /// 会被家人看见的平台数（去重后）。
    pub platforms: usize,
    /// 会被家人看见的资产条数。
    pub assets: usize,
    /// 未指定继承人的资产数（0 才算过）。
    pub assets_without_heir: usize,
    /// 没有政策记录的资产数（家人会看到「暂无政策」）。
    pub assets_without_policy: usize,
    /// 政策未核实的平台数。
    pub unverified_platforms: usize,
    /// 触发链是否可用。
    pub trigger_ready: bool,
    /// 当前可用通道。
    pub trigger_channel: String,
    /// 检查项：(是否通过, 说明)。说明文案由命令层按语言本地化。
    pub checks: Vec<DrillCheck>,
}

#[derive(Debug, serde::Serialize)]
pub struct DrillCheck {
    pub key: String,
    pub ok: bool,
    pub count: usize,
}

/// 计算演练结果。纯函数，便于单测。
pub fn evaluate_drill(
    report: &InheritanceReport,
    trigger_ready: bool,
    trigger_channel: &str,
) -> DrillReport {
    let mut platform_ids: Vec<i64> = Vec::new();
    for a in &report.assets {
        if !platform_ids.contains(&a.platform_id) {
            platform_ids.push(a.platform_id);
        }
    }
    let assets_without_heir = report
        .assets
        .iter()
        .filter(|a| {
            a.disposal_method == "inherit"
                && a.heir_name
                    .as_deref()
                    .map(|n| n.trim().is_empty())
                    .unwrap_or(true)
        })
        .count();
    let assets_without_policy = report
        .assets
        .iter()
        .filter(|a| {
            !report
                .policies
                .iter()
                .any(|p| p.platform_id == a.platform_id)
        })
        .count();
    let unverified_platforms = report.policies.iter().filter(|p| !p.has_source).count();

    let mut checks = Vec::new();
    checks.push(DrillCheck {
        key: "heir".to_string(),
        ok: assets_without_heir == 0,
        count: assets_without_heir,
    });
    checks.push(DrillCheck {
        key: "policy".to_string(),
        ok: assets_without_policy == 0,
        count: assets_without_policy,
    });
    checks.push(DrillCheck {
        key: "verified".to_string(),
        ok: unverified_platforms == 0,
        count: unverified_platforms,
    });
    checks.push(DrillCheck {
        key: "trigger".to_string(),
        ok: trigger_ready,
        count: if trigger_ready { 0 } else { 1 },
    });

    DrillReport {
        platforms: platform_ids.len(),
        assets: report.assets.len(),
        assets_without_heir,
        assets_without_policy,
        unverified_platforms,
        trigger_ready,
        trigger_channel: trigger_channel.to_string(),
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DigitalAsset, PlatformPolicyBrief, UserProfile};

    fn asset(platform_id: i64, method: &str, heir: Option<&str>) -> DigitalAsset {
        DigitalAsset {
            id: Some(platform_id),
            platform_id,
            platform_name: format!("平台{}", platform_id),
            account_identifier: "acc".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(1.0),
            value_currency: "CNY".to_string(),
            disposal_method: method.to_string(),
            heir_name: heir.map(|s| s.to_string()),
            heir_contact: None,
            notes: String::new(),
            has_credential: false,
            is_confirmed: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn policy(platform_id: i64, has_source: bool) -> PlatformPolicyBrief {
        PlatformPolicyBrief {
            platform_id,
            name: format!("平台{}", platform_id),
            required_documents: "死亡证明".to_string(),
            process_description: "联系客服".to_string(),
            estimated_difficulty: "中等".to_string(),
            estimated_time_days: "30天".to_string(),
            official_policy_url: String::new(),
            monetization_path: String::new(),
            risk_warnings: String::new(),
            tags: String::new(),
            has_source,
            last_reviewed: String::new(),
            data_source: if has_source {
                "官方帮助中心".to_string()
            } else {
                "未核实（待人工核对官方公告）".to_string()
            },
        }
    }

    fn report(assets: Vec<DigitalAsset>, policies: Vec<PlatformPolicyBrief>) -> InheritanceReport {
        InheritanceReport {
            user: UserProfile::default(),
            assets,
            heirs: Vec::new(),
            policies,
            authorities: Vec::new(),
            total_estimated_value: 0.0,
            property_assets_count: 0,
            personality_assets_count: 0,
            mixed_assets_count: 0,
            generated_at: "2026-10-26 12:00:00".to_string(),
        }
    }

    /// 含占位符时拒绝出包：与报告同一条策略。
    #[test]
    fn action_guide_refuses_placeholder() {
        let mut a = asset(1, "inherit", Some("王五"));
        a.account_identifier = "【加密数据 · 需解锁】".to_string();
        let r = report(vec![a], vec![policy(1, true)]);
        let err = build_action_guide(&r, ReportLocale::Zh, "x", "y").unwrap_err();
        assert!(err.contains("未解密"), "实际: {}", err);
    }

    /// 演练必须抓出「选了继承却没填继承人」——这正是家人会卡住的地方。
    #[test]
    fn drill_flags_missing_heir_and_policy() {
        let r = report(
            vec![
                asset(1, "inherit", None),
                asset(2, "liquidate", None),
                asset(3, "inherit", Some("王五")),
            ],
            vec![policy(1, true), policy(3, false)],
        );
        let d = evaluate_drill(&r, false, "none");
        assert_eq!(d.assets, 3);
        assert_eq!(d.platforms, 3);
        assert_eq!(d.assets_without_heir, 1, "只有 inherit 且无继承人才算缺失");
        assert_eq!(d.assets_without_policy, 1, "平台 2 没有政策记录");
        assert_eq!(d.unverified_platforms, 1);
        assert!(!d.trigger_ready);
        let failed: Vec<&str> = d
            .checks
            .iter()
            .filter(|c| !c.ok)
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(failed, vec!["heir", "policy", "verified", "trigger"]);
    }

    #[test]
    fn drill_passes_when_everything_is_ready() {
        let r = report(
            vec![asset(1, "inherit", Some("王五"))],
            vec![policy(1, true)],
        );
        let d = evaluate_drill(&r, true, "smtp");
        assert!(d.checks.iter().all(|c| c.ok), "全部就绪时不应有失败项");
        assert_eq!(d.trigger_channel, "smtp");
    }
}
