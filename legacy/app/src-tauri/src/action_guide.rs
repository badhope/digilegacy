// 家人执行包渲染：自包含、可打印、按平台分组。
//
// 与 report.rs 的分工：
// - report.rs 出「文件」：谁是谁、有哪些资产、处置意愿 → 适合归档与公证。
// - 本模块出「行动清单」：先做什么、找谁、带什么材料、多久、卡在哪。
//
// 两者共享同一份 `InheritanceReport`（含 policies），因此不会各写一份而漂移。
// 本模块不含任何凭据或密码。

use crate::family_faq::{render_faq_html, FAQ_STYLE};
use crate::models::{DigitalAsset, InheritanceReport};
use crate::report::{escape_html, ReportLocale};

/// 转义：FAQ 与执行清单共用同一实现，避免两处各写一份。
pub fn guide_html(input: &str) -> String {
    escape_html(input)
}

const STYLE: &str = r#"
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif;max-width:820px;margin:0 auto;padding:44px 48px;color:#232019;background:#fff;line-height:1.65}
h1,h2,h3{margin:0}
.doc-header{display:flex;flex-direction:column;gap:6px;padding-bottom:12px;border-bottom:1px solid #e4ddd3;margin-bottom:32px}
.doc-brand{font-size:13px;font-weight:600;color:#6b6157;letter-spacing:.5px}
.doc-no{text-align:right;font-size:11px;color:#a49a8d}
.doc-title{text-align:center;font-size:25px;font-weight:700;margin-bottom:10px;letter-spacing:4px}
.doc-meta{text-align:center;color:#8a8073;font-size:12px;margin-bottom:28px}
.section{margin-bottom:28px}
.section-title{font-size:15px;font-weight:600;padding-bottom:9px;border-bottom:2px solid #e4ddd3;margin-bottom:14px}
.intro{background:#f6f3ee;border-left:3px solid #b4594a;padding:12px 14px;border-radius:8px;margin-bottom:22px;font-size:13px;line-height:1.7}
.step{border:1px solid #e4ddd3;border-radius:10px;padding:16px 18px;margin-bottom:14px;break-inside:avoid}
.step-head{display:flex;align-items:center;gap:10px;margin-bottom:10px}
.step-no{display:inline-flex;align-items:center;justify-content:center;width:26px;height:26px;border-radius:50%;background:#b4594a;color:#fff;font-weight:700;font-size:13px;flex:0 0 auto}
.step h3{font-size:16px;margin:0}
.meta{font-size:13px;color:#4a433a;margin:5px 0;line-height:1.65}
.meta ul{margin:4px 0 0 20px;padding:0}
.warn{color:#a06a2c}
.doc-footer{display:flex;justify-content:space-between;margin-top:40px;padding-top:14px;border-top:1px solid #e4ddd3;font-size:11px;color:#a49a8d}
@page{margin:18mm 16mm}
@media print{body{padding:0}.step{break-inside:avoid}}
"#;

fn disposal_label(method: &str, locale: ReportLocale) -> &'static str {
    match (method, locale) {
        ("inherit", ReportLocale::Zh) => "指定继承",
        ("liquidate", ReportLocale::Zh) => "变现分配",
        ("memorialize", ReportLocale::Zh) => "保留纪念",
        ("authorize_view", ReportLocale::Zh) => "授权查阅",
        ("delete", ReportLocale::Zh) => "彻底删除",
        ("donate", ReportLocale::Zh) => "捐赠",
        ("no_action", ReportLocale::Zh) => "待设定",
        ("inherit", ReportLocale::En) => "Inherit",
        ("liquidate", ReportLocale::En) => "Liquidate",
        ("memorialize", ReportLocale::En) => "Memorialize",
        ("authorize_view", ReportLocale::En) => "Authorize access",
        ("delete", ReportLocale::En) => "Delete",
        ("donate", ReportLocale::En) => "Donate",
        ("no_action", ReportLocale::En) => "Not decided",
        (_, ReportLocale::Zh) => "未知",
        (_, ReportLocale::En) => "Unknown",
    }
}

/// 渲染家人执行清单。
///
/// `backup_hint` / `recovery_hint` 是持有者填写的保管位置；
/// 为空时**必须显式警告**而不是留白——空白会让家人以为「不需要」。
pub fn render_action_guide_html(
    report: &InheritanceReport,
    locale: ReportLocale,
    backup_hint: &str,
    recovery_hint: &str,
) -> String {
    let zh = locale == ReportLocale::Zh;
    let t = |zh_text: &'static str, en_text: &'static str| -> &'static str {
        if zh {
            zh_text
        } else {
            en_text
        }
    };

    // 按平台聚合：同一平台的多条资产合并成一件事，避免家人以为要跑多趟。
    let mut order: Vec<i64> = Vec::new();
    for a in &report.assets {
        if !order.contains(&a.platform_id) {
            order.push(a.platform_id);
        }
    }

    let mut steps = String::new();
    let mut step_no = 0;
    for pid in &order {
        let assets: Vec<&DigitalAsset> = report
            .assets
            .iter()
            .filter(|a| a.platform_id == *pid)
            .collect();
        if assets.is_empty() {
            continue;
        }
        step_no += 1;
        let name = escape_html(&assets[0].platform_name);
        let policy = report.policies.iter().find(|p| p.platform_id == *pid);

        let mut items = String::new();
        for a in &assets {
            items.push_str(&format!(
                "<li>{}：{}（{}）</li>",
                escape_html(a.heir_name.as_deref().unwrap_or(t("未指定", "Not set"))),
                disposal_label(&a.disposal_method, locale),
                escape_html(&a.account_identifier),
            ));
        }

        let dash = |v: &str| -> String {
            if v.trim().is_empty() {
                "-".to_string()
            } else {
                escape_html(v)
            }
        };
        let docs = policy
            .map(|p| dash(&p.required_documents))
            .unwrap_or_else(|| "-".to_string());
        let process = policy
            .map(|p| dash(&p.process_description))
            .unwrap_or_else(|| {
                escape_html(t(
                    "该平台暂无政策记录，请以官方公告为准",
                    "No policy record; check the official announcement",
                ))
            });
        let difficulty = policy
            .map(|p| dash(&p.estimated_difficulty))
            .unwrap_or_else(|| "-".to_string());
        let duration = policy
            .map(|p| dash(&p.estimated_time_days))
            .unwrap_or_else(|| "-".to_string());

        let source_note = match policy {
            Some(p) if p.has_source => {
                let reviewed = if p.last_reviewed.trim().is_empty() {
                    String::new()
                } else {
                    format!(
                        "（{} {}）",
                        t("核对日期", "reviewed"),
                        escape_html(&p.last_reviewed)
                    )
                };
                format!(
                    "<div class=\"meta\">{}：{}{}</div>",
                    t("来源", "Source"),
                    escape_html(&p.data_source),
                    reviewed
                )
            }
            _ => format!(
                "<div class=\"meta warn\">{}</div>",
                escape_html(t(
                    "本条政策没有来源标注，办理前请向平台官方客服核实。",
                    "This policy entry has no source annotation; confirm with official support before proceeding.",
                ))
            ),
        };

        steps.push_str(&format!(
            "<section class=\"step\">\
<div class=\"step-head\"><span class=\"step-no\">{step_no}</span><h3>{name}</h3></div>\
<div class=\"meta\">{docs_label}：{docs}</div>\
<div class=\"meta\">{proc_label}：{process}</div>\
<div class=\"meta\">{diff_label}：{difficulty}&nbsp;&nbsp;&nbsp;{dur_label}：{duration}</div>\
<div class=\"meta\">{heir_label}：<ul>{items}</ul></div>\
{source_note}\
</section>",
            step_no = step_no,
            name = name,
            docs_label = t("所需材料", "Documents required"),
            docs = docs,
            proc_label = t("办理流程", "Procedure"),
            process = process,
            diff_label = t("难度", "Difficulty"),
            difficulty = difficulty,
            dur_label = t("预计时长", "Duration"),
            duration = duration,
            heir_label = t("由谁办理", "Handled by"),
            items = items,
            source_note = source_note,
        ));
    }

    if steps.is_empty() {
        steps = format!(
            "<p class=\"meta\">{}</p>",
            escape_html(t(
                "尚未登记任何数字资产。",
                "No digital assets registered yet."
            ))
        );
    }

    // 继承人名单：家人需要知道「谁负责哪一块」以及「有问题找谁」。
    let heirs_block = if report.heirs.is_empty() {
        let name = report.user.heir_primary_name.trim();
        if name.is_empty() {
            format!(
                "<p class=\"meta warn\">{}</p>",
                escape_html(t(
                    "尚未登记继承人。家人将无法确定由谁出面办理。",
                    "No heir recorded. Your family will not know who is authorised to act.",
                ))
            )
        } else {
            format!(
                "<p class=\"meta\">{}{}</p>",
                escape_html(name),
                escape_html(&format!("（{}）", report.user.heir_primary_relation))
            )
        }
    } else {
        let mut s = String::new();
        for h in &report.heirs {
            let mark = if h.is_executor {
                t(
                    "（执行人，负责统筹）",
                    " (executor, coordinates everything)",
                )
            } else {
                ""
            };
            let share = if h.share_note.trim().is_empty() {
                String::new()
            } else {
                format!("－{}", escape_html(&h.share_note))
            };
            let contact = [h.phone.trim(), h.email.trim()]
                .iter()
                .filter(|v| !v.is_empty())
                .map(|v| escape_html(v))
                .collect::<Vec<_>>()
                .join(" / ");
            let contact_note = if contact.is_empty() {
                String::new()
            } else {
                format!("　{}", contact)
            };
            s.push_str(&format!(
                "<p class=\"meta\"><strong>{}</strong>（{}）{}{}{}</p>",
                escape_html(&h.name),
                escape_html(&h.relation),
                escape_html(mark),
                share,
                contact_note
            ));
        }
        s
    };

    let backup_block = if backup_hint.trim().is_empty() {
        format!(
            "<p class=\"meta warn\">{}</p>",
            escape_html(t(
                "尚未填写加密备份的存放位置。请向持有人本人确认，或在其实体遗物中查找——没有备份文件就无法读取凭据。",
                "No encrypted-backup location recorded. Ask the holder, or look among their belongings — without the backup file, credentials cannot be read.",
            ))
        )
    } else {
        format!("<p class=\"meta\">{}</p>", escape_html(backup_hint))
    };
    let recovery_block = if recovery_hint.trim().is_empty() {
        format!(
            "<p class=\"meta warn\">{}</p>",
            escape_html(t(
                "未记录恢复码的保管位置。恢复码只在设置主密码时显示一次，通常为手抄件。",
                "No recovery-code location recorded. The code was shown once when the master password was set, usually written on paper.",
            ))
        )
    } else {
        format!("<p class=\"meta\">{}</p>", escape_html(recovery_hint))
    };

    let faq_block = format!("<style>{}</style>{}", FAQ_STYLE, render_faq_html(locale));
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}"><head><meta charset="UTF-8"><title>{title}</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>{style}</style></head>
<body>
<div class="doc-header">
  <div class="doc-brand">归零 DigiLegacy</div>
  <div class="doc-no">{generated}</div>
</div>
<div class="doc-title">{title}</div>
<div class="doc-meta">{meta}</div>

<div class="intro">{intro}</div>

<div class="section">
  <div class="section-title">{heirs_title}</div>
  {heirs_block}
</div>

<div class="section">
  <div class="section-title">{first_title}</div>
  {backup_block}
  {recovery_block}
  <p class="meta">{credential_note}</p>
</div>

<div class="section">
  <div class="section-title">{steps_title}</div>
  {steps}
</div>

<div class="section">
  <div class="section-title">{support_title}</div>
  <p class="meta">{support_1}</p>
  <p class="meta">{support_2}</p>
  <p class="meta">{support_3}</p>
</div>

<div class="section">
  <div class="section-title">{faq_title}</div>
  {faq}
</div>

<div class="doc-footer">
  <span>{footer}</span>
  <span>{printed}</span>
</div>
</body></html>"#,
        lang = match locale {
            ReportLocale::Zh => "zh-CN",
            ReportLocale::En => "en",
        },
        title = t("数字遗产执行清单", "Digital Legacy Action Guide"),
        style = STYLE,
        generated = escape_html(&report.generated_at),
        meta = t(
            "按顺序从上往下办理；每一步都可以打印后逐项打勾。",
            "Work top to bottom; print this and tick each step off.",
        ),
        intro = t(
            "这份清单是给家人的行动指引，不含任何密码。它按平台排列：先做第 1 步，做完再做第 2 步。",
            "This guide is an action checklist for the family and contains no passwords. Steps are ordered by platform: finish step 1 before starting step 2.",
        ),
        heirs_title = t("由谁负责", "Who handles this"),
        heirs_block = heirs_block,
        first_title = t("开始之前：先找到两样东西", "Before you start: find two things"),
        backup_block = backup_block,
        recovery_block = recovery_block,
        credential_note = t(
            "凭据本身不在本文件里，也不在继承报告里。它们保存在加密备份中，需要备份文件 + 主密码（或恢复码）才能读取。",
            "Credentials are neither in this file nor in the inheritance report. They live in the encrypted backup and require the backup file plus the master password (or the recovery code).",
        ),
        steps_title = t("逐平台办理", "Platform by platform"),
        steps = steps,
        support_title = t("遇到阻碍时", "If you get stuck"),
        support_1 = t(
            "1. 要求平台出具书面答复（邮件或工单编号）；口头拒绝无法作为后续依据。",
            "1. Ask the platform for a written reply (email or ticket number); a verbal refusal is not usable evidence later.",
        ),
        support_2 = t(
            "2. 平台政策会变。标注「未核实」的条目，请先致电官方客服确认，再按客服口径办理。",
            "2. Policies change. For entries marked unsourced, call official support first and follow their instructions.",
        ),
        support_3 = t(
            "3. 如遇平台拒绝，保留书面材料并咨询律师；部分平台（如微信）明确不支持账号本身继承，仅余额可处理。",
            "3. If a platform refuses, keep the written record and consult a lawyer. Some platforms (e.g. WeChat) do not allow the account itself to be inherited; only balances can be handled.",
        ),
        faq_title = t("常见问题", "Frequently asked questions"),
        faq = faq_block,
        footer = t("归零 DigiLegacy · 本地生成", "DigiLegacy · generated locally"),
        printed = t("可打印", "Printable"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{PlatformPolicyBrief, UserProfile};

    fn asset(platform_id: i64, method: &str, heir: Option<&str>, ident: &str) -> DigitalAsset {
        DigitalAsset {
            id: Some(platform_id),
            platform_id,
            platform_name: format!("平台{}", platform_id),
            account_identifier: ident.to_string(),
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

    #[test]
    fn groups_assets_by_platform() {
        let r = report(
            vec![
                asset(1, "inherit", Some("王五"), "a"),
                asset(1, "memorialize", None, "b"),
                asset(2, "liquidate", None, "c"),
            ],
            vec![policy(1, true), policy(2, false)],
        );
        let html = render_action_guide_html(&r, ReportLocale::Zh, "保险柜", "笔记本");
        assert!(html.contains("数字遗产执行清单"));
        assert_eq!(
            html.matches("class=\"step-no\"").count(),
            2,
            "同平台资产必须合并成一步"
        );
        assert!(html.contains("保险柜"));
        assert!(html.contains("笔记本"));
    }

    #[test]
    fn warns_when_custody_unknown() {
        let r = report(
            vec![asset(1, "inherit", Some("王五"), "a")],
            vec![policy(1, true)],
        );
        let html = render_action_guide_html(&r, ReportLocale::Zh, "", "");
        assert!(html.contains("尚未填写加密备份的存放位置"));
        assert!(html.contains("未记录恢复码的保管位置"));
    }

    #[test]
    fn english_guide_is_english() {
        let r = report(
            vec![asset(1, "inherit", Some("Wang"), "a")],
            vec![policy(1, true)],
        );
        let html = render_action_guide_html(&r, ReportLocale::En, "safe", "notebook");
        assert!(html.contains("Digital Legacy Action Guide"));
        assert!(html.contains(r#"<html lang="en">"#));
        assert!(!html.contains("数字遗产执行清单"));
    }

    #[test]
    fn escapes_untrusted_values() {
        let evil = "<script>alert(1)</script>";
        let r = report(
            vec![asset(1, "inherit", Some(evil), evil)],
            vec![policy(1, true)],
        );
        let html = render_action_guide_html(&r, ReportLocale::Zh, evil, evil);
        assert!(!html.contains("<script>"), "不得出现未转义脚本");
        assert!(html.contains("&lt;script&gt;"));
    }

    /// 执行清单必须带家人 FAQ：这是「照着做」的最后一道保险。
    #[test]
    fn action_guide_includes_family_faq() {
        let r = report(
            vec![asset(1, "inherit", Some("王五"), "a")],
            vec![policy(1, true)],
        );
        let html = render_action_guide_html(&r, ReportLocale::Zh, "x", "y");
        assert!(html.contains("常见问题"), "必须有 FAQ 区块");
        assert!(html.contains("账号密码在哪里"), "必须回答最要紧的问题");
        assert!(html.contains("class=\"faq\""), "FAQ 必须可折叠/可打印");
    }

    #[test]
    fn marks_unsourced_policy() {
        let r = report(
            vec![asset(1, "inherit", Some("王五"), "a")],
            vec![policy(1, false)],
        );
        let html = render_action_guide_html(&r, ReportLocale::Zh, "x", "y");
        assert!(html.contains("没有来源标注"));
    }
}
