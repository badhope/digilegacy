//! 继承报告 HTML 生成：**后端唯一事实源**。
//!
//! 为什么不让前端抓 DOM：旧实现用 `document.querySelector('.report-page').outerHTML`
//! 拼导出文件，带来三个问题——
//! 1. 锁定态下页面里是「【加密数据 · 需解锁】」占位符，会被原样打印进报告；
//! 2. 页面样式（含 Naive UI 注入）与导出样式两套维护，容易漂移；
//! 3. 同一份内容在「单文件导出」和「ZIP 工具包」里走两条代码路径，曾导致
//!    ZIP 里是裸片段（缺 DOCTYPE/打印样式）。
//!
//! 现在由 Rust 直接渲染：不可信内容统一 HTML 转义，字体/打印样式集中在此，
//! 前端只负责请求与展示，导出物与预览的结构性差异降到零。
//!
//! 导出内容必须覆盖「家人照着就能办」所需的全部信息：平台政策、所需材料、
//! 预计时长、难度、官方入口、继承人联系方式。少任何一项，报告就退化成
//! 一份「知道有这件事、但不知道怎么办」的清单。

use crate::models::{DigitalAsset, InheritanceReport, PlatformPolicyBrief};

const STYLE: &str = r#"
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif;max-width:820px;margin:0 auto;padding:44px 48px;color:#232019;background:#fff;line-height:1.6}
h1,h2,h3{margin:0}
table{width:100%;border-collapse:collapse;font-size:14px}
td,th{border-bottom:1px solid #e8e2d8;padding:10px 12px;text-align:left;vertical-align:middle;word-break:break-word}
th{background:#f6f3ee;color:#6b6157;font-weight:600;font-size:13px;border-bottom:2px solid #e4ddd3}
.muted{color:#6b6157}
.doc-header{display:flex;flex-direction:column;gap:6px;padding-bottom:12px;border-bottom:1px solid #e4ddd3;margin-bottom:40px}
.doc-brand{display:flex;align-items:center;gap:8px;font-size:13px;font-weight:600;color:#6b6157;letter-spacing:.5px}
.doc-no{text-align:right;font-size:11px;color:#a49a8d;letter-spacing:.5px}
.doc-title{text-align:center;font-size:25px;font-weight:700;margin-bottom:12px;letter-spacing:6px}
.doc-meta{text-align:center;color:#8a8073;font-size:12px;margin-bottom:40px}
.section{margin-bottom:28px}
.section-title{font-size:15px;font-weight:600;padding-bottom:9px;border-bottom:2px solid #e4ddd3;margin-bottom:14px;letter-spacing:.5px}
.stats{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;margin:16px 0}
.stats div{padding:12px;border:1px solid #e4ddd3;border-radius:8px;text-align:center}
.stats .num{font-size:32px;font-weight:700}
.note{padding:12px;background:#f6f3ee;border-radius:8px;border-left:3px solid #b4594a;margin-bottom:12px}
.policy{padding:14px 16px;border:1px solid #e4ddd3;border-radius:10px;margin-bottom:14px;break-inside:avoid}
.policy-head{display:flex;justify-content:space-between;gap:12px;align-items:baseline;margin-bottom:8px}
.policy-name{font-size:15px;font-weight:700}
.policy-badges{display:flex;gap:8px;flex-wrap:wrap;font-size:11px;color:#6b6157}
.policy-badge{padding:2px 8px;border-radius:999px;background:#f6f3ee;border:1px solid #e4ddd3}
.policy-row{margin:6px 0;font-size:13px}
.policy-label{color:#8a8073;font-weight:600}
.policy-link{font-size:12px;color:#8a8073;word-break:break-all}
.unverified{color:#a06a2c}
.doc-footer{display:flex;justify-content:space-between;margin-top:44px;padding-top:14px;border-top:1px solid #e4ddd3;font-size:11px;color:#a49a8d}
@page{margin:18mm 16mm}
@media print{body{padding:0}.section{break-inside:avoid}.no-print{display:none}}
"#;

/// 占位符：未解密的值绝不能出现在任何导出物里。
const PLACEHOLDER: &str = "【加密数据 · 需解锁】";

/// 报告语言。导出物必须与界面语言一致——
/// 旧实现硬编码 `zh-CN`，英文用户导出的仍是中文文档。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportLocale {
    Zh,
    En,
}

impl ReportLocale {
    /// 从 IPC 传入的 locale 归一化。未知值回退中文（桌面历史默认）。
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "en_us" => Self::En,
            _ => Self::Zh,
        }
    }

    fn lang(self) -> &'static str {
        match self {
            Self::Zh => "zh-CN",
            Self::En => "en",
        }
    }
}

/// 一份报告里用到的全部静态文案。集中在这里，避免正文里散布 match。
struct Copy {
    title: &'static str,
    meta: &'static str,
    sec_holder: &'static str,
    sec_overview: &'static str,
    sec_assets: &'static str,
    sec_disposal: &'static str,
    sec_policies: &'static str,
    sec_authorities: &'static str,
    authority_reachable: &'static str,
    authority_matched: &'static str,
    authority_portal: &'static str,
    authority_checked: &'static str,
    sec_statement: &'static str,
    name: &'static str,
    phone: &'static str,
    email: &'static str,
    heir: &'static str,
    emergency: &'static str,
    total: &'static str,
    property: &'static str,
    personality: &'static str,
    total_value: &'static str,
    col_platform: &'static str,
    col_account: &'static str,
    col_category: &'static str,
    col_value: &'static str,
    col_disposal: &'static str,
    col_heir: &'static str,
    col_contact: &'static str,
    no_assets: &'static str,
    not_set: &'static str,
    docs: &'static str,
    process: &'static str,
    difficulty: &'static str,
    duration: &'static str,
    official: &'static str,
    risks: &'static str,
    no_policy: &'static str,
    sourced: &'static str,
    unsourced: &'static str,
    unsourced_note: &'static str,
    disposal_phrase: &'static str,
    heir_phrase: &'static str,
    footer: &'static str,
    page: &'static str,
    statement_1: &'static str,
    statement_2: &'static str,
    statement_3: &'static str,
    statement_4: &'static str,
}

const ZH: Copy = Copy {
    title: "数字遗产继承报告",
    meta: "｜ 仅供继承人内部使用",
    sec_holder: "一、持有人信息",
    sec_overview: "二、资产概览",
    sec_assets: "三、资产清单",
    sec_disposal: "四、处置安排",
    sec_policies: "五、平台办理指引",
    sec_authorities: "六、权威来源",
    authority_reachable: "入口可达",
    authority_matched: "内容已核对",
    authority_portal: "检索入口（非条文页）",
    authority_checked: "核验日期",
    sec_statement: "七、声明",
    name: "姓名",
    phone: "联系电话",
    email: "邮箱",
    heir: "主要继承人",
    emergency: "紧急联系人",
    total: "资产总数",
    property: "财产型",
    personality: "人格型",
    total_value: "预估总值",
    col_platform: "平台",
    col_account: "账号标识",
    col_category: "类别",
    col_value: "预估价值",
    col_disposal: "处置方式",
    col_heir: "继承人",
    col_contact: "继承人联系方式",
    no_assets: "尚未登记任何数字资产",
    not_set: "未指定",
    docs: "所需材料",
    process: "办理流程",
    difficulty: "难度",
    duration: "预计时长",
    official: "官方入口",
    risks: "风险提示",
    no_policy: "该平台暂无政策记录，请以平台官方公告为准",
    sourced: "有来源标注",
    unsourced: "无来源标注",
    unsourced_note: "本条政策没有来源标注，办理前请向平台官方客服核实",
    disposal_phrase: "处置",
    heir_phrase: "继承人",
    footer: "归零 DigiLegacy · 本地生成",
    page: "共 1 页",
    statement_1: "1. 本报告由「归零 DigiLegacy」根据持有人登记的信息生成，不含任何账号密码或密钥。",
    statement_2: "2. 账号继承须遵守各平台用户协议与适用法律，通常需提供死亡证明与继承权公证书。",
    statement_3: "3. 报告为明文文件，请与纸质文书同等保管。",
    statement_4: "4. 平台政策可能随时间变化，办理前请以平台最新公告为准。",
};

const EN: Copy = Copy {
    title: "Digital Legacy Inheritance Report",
    meta: "｜ For the family's internal use only",
    sec_holder: "1. Holder information",
    sec_overview: "2. Asset overview",
    sec_assets: "3. Asset list",
    sec_disposal: "4. Disposal arrangements",
    sec_policies: "5. Platform-by-platform instructions",
    sec_authorities: "6. Authoritative sources",
    authority_reachable: "Page reachable",
    authority_matched: "Content verified",
    authority_portal: "Search portal (not the article page)",
    authority_checked: "Checked",
    sec_statement: "7. Statement",
    name: "Name",
    phone: "Phone",
    email: "Email",
    heir: "Primary heir",
    emergency: "Emergency contact",
    total: "Total assets",
    property: "Property",
    personality: "Personality",
    total_value: "Estimated value",
    col_platform: "Platform",
    col_account: "Account",
    col_category: "Category",
    col_value: "Value",
    col_disposal: "Disposal",
    col_heir: "Heir",
    col_contact: "Heir contact",
    no_assets: "No digital assets registered yet",
    not_set: "Not set",
    docs: "Documents required",
    process: "Procedure",
    difficulty: "Difficulty",
    duration: "Typical duration",
    official: "Official page",
    risks: "Risks",
    no_policy: "No policy record for this platform; check the platform's official announcement",
    sourced: "Source noted",
    unsourced: "No source",
    unsourced_note: "This policy entry has no source annotation; confirm with the platform's official support before proceeding",
    disposal_phrase: "Disposal",
    heir_phrase: "Heir",
    footer: "DigiLegacy · generated locally",
    page: "1 page",
    statement_1: "1. This report is generated by DigiLegacy from the information the holder registered. It contains no passwords or keys.",
    statement_2: "2. Account inheritance is subject to each platform's terms of service and applicable law; a death certificate and certificate of inheritance are usually required.",
    statement_3: "3. This report is a plaintext file. Treat it like a paper document.",
    statement_4: "4. Platform policies change over time; always confirm against the platform's latest announcement before acting.",
};

fn copy_for(locale: ReportLocale) -> &'static Copy {
    match locale {
        ReportLocale::Zh => &ZH,
        ReportLocale::En => &EN,
    }
}

/// 拒绝把「解密占位符」当成用户输入保存。
///
/// 与 `db::reject_placeholder` 同语义，但供不经过 db 模块的字段（保管位置）使用。
pub fn reject_placeholder_value(field: &str, value: &str) -> Result<(), String> {
    if value == PLACEHOLDER {
        return Err(format!(
            "{}当前处于加密锁定状态，无法保存。请先解锁后重试（本次未写入任何数据）",
            field
        ));
    }
    Ok(())
}

/// HTML 转义：报告里所有来自数据库/用户输入的文本都必须经过它。
pub fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// 报告是否含有未解密占位符。含占位符时禁止生成导出物——
/// 一份写着「需解锁」的报告对继承人毫无价值，且会误导读者以为数据就是这样。
pub fn contains_placeholder(report: &InheritanceReport) -> bool {
    let user_fields = [
        &report.user.full_name,
        &report.user.phone,
        &report.user.email,
        &report.user.emergency_contact_name,
        &report.user.emergency_contact_phone,
        &report.user.heir_primary_name,
        &report.user.heir_primary_phone,
    ];
    if user_fields.iter().any(|v| v.as_str() == PLACEHOLDER) {
        return true;
    }
    report.assets.iter().any(|a| {
        a.account_identifier == PLACEHOLDER
            || a.account_description == PLACEHOLDER
            || a.notes == PLACEHOLDER
            || a.heir_name.as_deref() == Some(PLACEHOLDER)
            || a.heir_contact.as_deref() == Some(PLACEHOLDER)
    })
}

fn category_label(cat: &str, locale: ReportLocale) -> &'static str {
    match (cat, locale) {
        ("property", ReportLocale::Zh) => "财产型",
        ("personality", ReportLocale::Zh) => "人格型",
        ("mixed", ReportLocale::Zh) => "混合型",
        ("property", ReportLocale::En) => "Property",
        ("personality", ReportLocale::En) => "Personality",
        ("mixed", ReportLocale::En) => "Mixed",
        (_, ReportLocale::Zh) => "未知",
        (_, ReportLocale::En) => "Unknown",
    }
}

pub(crate) fn disposal_label(method: &str, locale: ReportLocale) -> &'static str {
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

fn asset_row(asset: &DigitalAsset, locale: ReportLocale) -> String {
    let value = asset
        .estimated_value
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| format!("¥{:.2}", v))
        .unwrap_or_else(|| "-".to_string());
    let contact = asset
        .heir_contact
        .as_deref()
        .filter(|v| !v.is_empty())
        .map(escape_html)
        .unwrap_or_else(|| "-".to_string());
    format!(
        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
        escape_html(&asset.platform_name),
        escape_html(&asset.account_identifier),
        category_label(&asset.category, locale),
        value,
        disposal_label(&asset.disposal_method, locale),
        escape_html(asset.heir_name.as_deref().unwrap_or("-")),
        contact,
    )
}

/// 平台办理指引卡片：家人拿到后照着做的那一页。
pub(crate) fn policy_card(p: &PlatformPolicyBrief, locale: ReportLocale) -> String {
    let c = copy_for(locale);
    let dash = |v: &str| -> String {
        if v.trim().is_empty() {
            "-".to_string()
        } else {
            escape_html(v)
        }
    };
    let mut badges = String::new();
    if !p.estimated_difficulty.trim().is_empty() {
        badges.push_str(&format!(
            "<span class=\"policy-badge\">{}：{}</span>",
            c.difficulty,
            escape_html(&p.estimated_difficulty)
        ));
    }
    if !p.estimated_time_days.trim().is_empty() {
        badges.push_str(&format!(
            "<span class=\"policy-badge\">{}：{}</span>",
            c.duration,
            escape_html(&p.estimated_time_days)
        ));
    }
    for tag in p
        .tags
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
    {
        badges.push_str(&format!(
            "<span class=\"policy-badge\">{}</span>",
            escape_html(tag)
        ));
    }
    let source = if p.has_source {
        let reviewed = if p.last_reviewed.trim().is_empty() {
            String::new()
        } else {
            format!(
                "<span class=\"policy-badge\">{} {}</span>",
                if locale == ReportLocale::Zh {
                    "核对日期"
                } else {
                    "Reviewed"
                },
                escape_html(&p.last_reviewed)
            )
        };
        format!(
            "<span class=\"policy-badge\">{}</span>{}",
            escape_html(c.sourced),
            reviewed
        )
    } else {
        format!(
            "<span class=\"policy-badge unverified\">{}</span>",
            escape_html(c.unsourced)
        )
    };
    let risk = if p.risk_warnings.trim().is_empty() {
        String::new()
    } else {
        format!(
            "<div class=\"policy-row\"><span class=\"policy-label\">{}：</span>{}</div>",
            c.risks,
            escape_html(&p.risk_warnings)
        )
    };
    let monetization = if p.monetization_path.trim().is_empty() {
        String::new()
    } else {
        format!(
            "<div class=\"policy-row\"><span class=\"policy-label\">{}：</span>{}</div>",
            if locale == ReportLocale::Zh {
                "过户/变现路径"
            } else {
                "Transfer / liquidation path"
            },
            escape_html(&p.monetization_path)
        )
    };
    let link = if p.official_policy_url.trim().is_empty() {
        String::new()
    } else {
        // 只允许 http(s)：报告会被家人用浏览器打开，不能给出 javascript: 之类入口。
        let url = p.official_policy_url.trim();
        let lower = url.to_ascii_lowercase();
        if lower.starts_with("https://") || lower.starts_with("http://") {
            format!(
                "<div class=\"policy-row\"><span class=\"policy-label\">{}：</span><span class=\"policy-link\">{}</span></div>",
                c.official,
                escape_html(url)
            )
        } else {
            String::new()
        }
    };
    let unsourced_note = if p.has_source {
        String::new()
    } else {
        format!(
            "<div class=\"policy-row unverified\">{}</div>",
            escape_html(c.unsourced_note)
        )
    };
    format!(
        "<div class=\"policy\">\
<div class=\"policy-head\"><span class=\"policy-name\">{name}</span><span class=\"policy-badges\">{badges}{source}</span></div>\
<div class=\"policy-row\"><span class=\"policy-label\">{docs}：</span>{docs_v}</div>\
<div class=\"policy-row\"><span class=\"policy-label\">{process}：</span>{process_v}</div>\
{monetization}{risk}{link}{unsourced_note}\
</div>",
        name = escape_html(&p.name),
        badges = badges,
        source = source,
        docs = c.docs,
        docs_v = dash(&p.required_documents),
        process = c.process,
        process_v = dash(&p.process_description),
        monetization = monetization,
        risk = risk,
        link = link,
        unsourced_note = unsourced_note,
    )
}

/// 渲染完整、可独立打开的 HTML 文档（单文件导出与 ZIP 工具包共用同一份）。
pub fn render_report_html(report: &InheritanceReport, locale: ReportLocale) -> String {
    let c = copy_for(locale);
    let doc_no: String = report
        .generated_at
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();

    let rows = if report.assets.is_empty() {
        format!(
            "<tr><td colspan=\"7\" class=\"muted\">{}</td></tr>",
            c.no_assets
        )
    } else {
        report
            .assets
            .iter()
            .map(|a| asset_row(a, locale))
            .collect::<Vec<_>>()
            .join("")
    };

    let inheritable = if report
        .assets
        .iter()
        .all(|a| a.disposal_method == "no_action")
    {
        format!(
            "<p class=\"muted\">{}</p>",
            escape_html(if locale == ReportLocale::Zh {
                "尚未设定任何处置意愿"
            } else {
                "No disposal wishes set yet"
            })
        )
    } else {
        report
            .assets
            .iter()
            .filter(|a| a.disposal_method != "no_action")
            .map(|a| {
                let heir = a
                    .heir_name
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .unwrap_or(c.not_set);
                let contact = a
                    .heir_contact
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .unwrap_or(c.not_set);
                format!(
                    "<div class=\"note\"><strong>{}</strong><br><span class=\"muted\">{}：{}；{}：{}（{}）</span></div>",
                    escape_html(&a.platform_name),
                    c.disposal_phrase,
                    disposal_label(&a.disposal_method, locale),
                    c.heir_phrase,
                    escape_html(heir),
                    escape_html(contact)
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    let authorities = if report.authorities.is_empty() {
        String::new()
    } else {
        report
            .authorities
            .iter()
            .map(|a| {
                let title = if locale == ReportLocale::Zh {
                    &a.title_zh
                } else {
                    &a.title_en
                };
                let note = if locale == ReportLocale::Zh {
                    &a.note_zh
                } else {
                    &a.note_en
                };
                let portal = if a.kind == "portal" {
                    if locale == ReportLocale::Zh {
                        &a.portal_note_zh
                    } else {
                        &a.portal_note_en
                    }
                } else {
                    ""
                };
                // 两件事分开写：可达 ≠ 内容已核对
                let badges = format!(
                    "<span class=\"policy-badge\">{}</span><span class=\"policy-badge{}\">{}</span>",
                    escape_html(c.authority_reachable),
                    if a.content_matched { "" } else { " unverified" },
                    escape_html(if a.content_matched {
                        c.authority_matched
                    } else {
                        c.unsourced
                    })
                );
                let checked = if a.last_checked.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<span class=\"policy-badge\">{} {}</span>",
                        escape_html(c.authority_checked),
                        escape_html(&a.last_checked)
                    )
                };
                let portal_line = if portal.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<div class=\"policy-row\"><span class=\"policy-label\">{}</span>{}</div>",
                        escape_html(c.authority_portal),
                        escape_html(portal)
                    )
                };
                format!(
                    "<div class=\"policy\"><div class=\"policy-head\"><span class=\"policy-name\">{}</span><span class=\"policy-badges\">{}{}</span></div><div class=\"policy-row\"><span class=\"policy-label\">{}</span>{}</div>{}<div class=\"policy-row\"><span class=\"policy-label\">{}</span><span class=\"policy-link\">{}</span></div></div>",
                    escape_html(title),
                    badges,
                    checked,
                    escape_html(c.process),
                    escape_html(note),
                    portal_line,
                    escape_html(c.official),
                    escape_html(&a.url),
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    let policies = if report.policies.is_empty() {
        format!("<p class=\"muted\">{}</p>", c.no_policy)
    } else {
        report
            .policies
            .iter()
            .map(|p| policy_card(p, locale))
            .collect::<Vec<_>>()
            .join("")
    };

    // 有多继承人名单时优先展示名单（执行人加标记），否则回退旧字段。
    let heir_label = if report.heirs.is_empty() {
        format!(
            "{}（{}）",
            escape_html(&report.user.heir_primary_name),
            escape_html(&report.user.heir_primary_relation)
        )
    } else {
        report
            .heirs
            .iter()
            .map(|h| {
                let mark = if h.is_executor {
                    if locale == ReportLocale::Zh {
                        "（执行人）"
                    } else {
                        " (executor)"
                    }
                } else {
                    ""
                };
                let share = if h.share_note.trim().is_empty() {
                    String::new()
                } else {
                    format!("－{}", escape_html(&h.share_note))
                };
                format!(
                    "{}（{}）{}{}",
                    escape_html(&h.name),
                    escape_html(&h.relation),
                    mark,
                    share
                )
            })
            .collect::<Vec<_>>()
            .join("；")
    };
    let emergency_label = format!(
        "{}（{}）",
        escape_html(&report.user.emergency_contact_name),
        escape_html(&report.user.emergency_contact_relation)
    );

    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}"><head><meta charset="UTF-8"><title>{title}</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>{style}</style></head>
<body>
<div class="doc-header">
  <div class="doc-brand">{app_name}</div>
  <div class="doc-no">{doc_no_label} {doc_no}</div>
</div>
<div class="doc-title">{title}</div>
<div class="doc-meta">{generated_label} {generated}{meta}</div>

<div class="section">
  <div class="section-title">{sec_holder}</div>
  <table><tbody>
    <tr><td>{name_label}</td><td>{name}</td></tr>
    <tr><td>{phone_label}</td><td>{phone}</td></tr>
    <tr><td>{email_label}</td><td>{email}</td></tr>
    <tr><td>{heir_label}</td><td>{heir}</td></tr>
    <tr><td>{emergency_label}</td><td>{emergency}</td></tr>
  </tbody></table>
</div>

<div class="section">
  <div class="section-title">{sec_overview}</div>
  <div class="stats">
    <div><div class="num">{total}</div><div class="muted">{total_label}</div></div>
    <div><div class="num">{property}</div><div class="muted">{property_label}</div></div>
    <div><div class="num">{personality}</div><div class="muted">{personality_label}</div></div>
    <div><div class="num">¥{value:.2}</div><div class="muted">{value_label}</div></div>
  </div>
</div>

<div class="section">
  <div class="section-title">{sec_assets}</div>
  <table><thead><tr><th>{col_platform}</th><th>{col_account}</th><th>{col_category}</th><th>{col_value}</th><th>{col_disposal}</th><th>{col_heir}</th><th>{col_contact}</th></tr></thead>
  <tbody>{rows}</tbody></table>
</div>

<div class="section">
  <div class="section-title">{sec_disposal}</div>
  {inheritable}
</div>

<div class="section">
  <div class="section-title">{sec_policies}</div>
  {policies}
</div>

<div class="section">
  <div class="section-title">{sec_authorities}</div>
  {authorities}
</div>

<div class="section">
  <div class="section-title">{sec_statement}</div>
  <p>{statement_1}</p>
  <p>{statement_2}</p>
  <p>{statement_3}</p>
  <p>{statement_4}</p>
</div>

<div class="doc-footer">
  <span>{footer}</span>
  <span>{page}</span>
</div>
</body></html>"#,
        lang = locale.lang(),
        title = c.title,
        style = STYLE,
        app_name = "归零 DigiLegacy",
        doc_no_label = if locale == ReportLocale::Zh {
            "编号"
        } else {
            "No."
        },
        doc_no = escape_html(&doc_no),
        generated_label = if locale == ReportLocale::Zh {
            "生成时间"
        } else {
            "Generated"
        },
        generated = escape_html(&report.generated_at),
        meta = c.meta,
        sec_holder = c.sec_holder,
        sec_overview = c.sec_overview,
        sec_assets = c.sec_assets,
        sec_disposal = c.sec_disposal,
        sec_policies = c.sec_policies,
        sec_statement = c.sec_statement,
        name_label = c.name,
        phone_label = c.phone,
        email_label = c.email,
        heir_label = c.heir,
        emergency_label = c.emergency,
        name = escape_html(&report.user.full_name),
        phone = escape_html(&report.user.phone),
        email = escape_html(&report.user.email),
        heir = if report.user.heir_primary_name.trim().is_empty() {
            escape_html(c.not_set)
        } else {
            heir_label
        },
        emergency = if report.user.emergency_contact_name.trim().is_empty() {
            escape_html(c.not_set)
        } else {
            emergency_label
        },
        total = report.assets.len(),
        total_label = c.total,
        property = report.property_assets_count,
        property_label = c.property,
        personality = report.personality_assets_count,
        personality_label = c.personality,
        value = report.total_estimated_value,
        value_label = c.total_value,
        col_platform = c.col_platform,
        col_account = c.col_account,
        col_category = c.col_category,
        col_value = c.col_value,
        col_disposal = c.col_disposal,
        col_heir = c.col_heir,
        col_contact = c.col_contact,
        rows = rows,
        inheritable = inheritable,
        policies = policies,
        authorities = authorities,
        sec_authorities = c.sec_authorities,
        statement_1 = c.statement_1,
        statement_2 = c.statement_2,
        statement_3 = c.statement_3,
        statement_4 = c.statement_4,
        footer = c.footer,
        page = c.page,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::UserProfile;

    fn user() -> UserProfile {
        UserProfile {
            id: None,
            full_name: "张三".to_string(),
            phone: "13800000000".to_string(),
            email: "a@b.com".to_string(),
            id_last4: String::new(),
            emergency_contact_name: "李四".to_string(),
            emergency_contact_phone: "13900000000".to_string(),
            emergency_contact_relation: "子女".to_string(),
            emergency_contact_email: String::new(),
            heir_primary_name: "王五".to_string(),
            heir_primary_phone: "13700000000".to_string(),
            heir_primary_relation: "子女".to_string(),
            heir_primary_email: String::new(),
            check_in_interval_days: 30,
            last_check_in: None,
            dms_enabled: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn asset(over: impl FnOnce(&mut DigitalAsset)) -> DigitalAsset {
        let mut a = DigitalAsset {
            id: Some(1),
            platform_id: 1,
            platform_name: "支付宝".to_string(),
            account_identifier: "acc".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(100.0),
            value_currency: "CNY".to_string(),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: Some("13700000000".to_string()),
            notes: String::new(),
            has_credential: false,
            is_confirmed: true,
            created_at: "2026-01-01 00:00:00".to_string(),
            updated_at: "2026-01-01 00:00:00".to_string(),
        };
        over(&mut a);
        a
    }

    fn policy(over: impl FnOnce(&mut PlatformPolicyBrief)) -> PlatformPolicyBrief {
        let mut p = PlatformPolicyBrief {
            platform_id: 1,
            name: "支付宝".to_string(),
            required_documents: "死亡证明、继承公证书、继承人身份证".to_string(),
            process_description: "联系客服 95188 办理余额继承".to_string(),
            estimated_difficulty: "中等".to_string(),
            estimated_time_days: "30-90天".to_string(),
            official_policy_url: "https://help.alipay.com".to_string(),
            monetization_path: "余额可依法过户".to_string(),
            risk_warnings: "大额需公证".to_string(),
            tags: "需公证,高价值".to_string(),
            has_source: false,
            last_reviewed: String::new(),
            data_source: String::new(),
        };
        over(&mut p);
        p
    }

    fn report(assets: Vec<DigitalAsset>) -> InheritanceReport {
        InheritanceReport {
            user: user(),
            assets,
            heirs: Vec::new(),
            authorities: Vec::new(),
            policies: vec![policy(|_| {})],
            total_estimated_value: 100.0,
            property_assets_count: 1,
            personality_assets_count: 0,
            mixed_assets_count: 0,
            generated_at: "2026-10-26 12:00:00".to_string(),
        }
    }

    #[test]
    fn renders_standalone_document_with_print_styles() {
        let html = render_report_html(&report(vec![asset(|_| {})]), ReportLocale::Zh);
        assert!(html.starts_with("<!DOCTYPE html>"), "必须是完整文档");
        assert!(html.contains("@media print"), "必须带打印样式");
        assert!(html.contains("数字遗产继承报告"));
        assert!(html.contains("张三"));
        assert!(html.contains("支付宝"));
        assert!(html.contains("指定继承"));
        assert!(html.contains("¥100.00"));
    }

    /// 报告里的任何用户输入都必须转义——否则一个平台名就能注入标签。
    #[test]
    fn escapes_all_untrusted_values() {
        let evil = r#"<script>alert('x')</script>"#;
        let mut r = report(vec![asset(|a| {
            a.platform_name = evil.to_string();
            a.account_identifier = evil.to_string();
            a.heir_name = Some(evil.to_string());
            a.heir_contact = Some(evil.to_string());
        })]);
        r.user.full_name = evil.to_string();
        r.user.heir_primary_name = evil.to_string();
        r.policies = vec![policy(|p| {
            p.name = evil.to_string();
            p.process_description = evil.to_string();
            p.risk_warnings = evil.to_string();
        })];

        let html = render_report_html(&r, ReportLocale::Zh);
        assert!(!html.contains("<script>"), "不得出现未转义脚本标签");
        assert!(html.contains("&lt;script&gt;"), "必须转义为实体");
        assert!(html.contains("&#39;"), "单引号也必须转义");
    }

    /// 含占位符的报告禁止导出：写着「需解锁」的报告会误导继承人。
    #[test]
    fn placeholder_report_is_flagged() {
        assert!(!contains_placeholder(&report(vec![asset(|_| {})])));
        assert!(contains_placeholder(&report(vec![asset(|a| {
            a.account_identifier = PLACEHOLDER.to_string();
        })])));
        assert!(contains_placeholder(&report(vec![asset(|a| {
            a.heir_name = Some(PLACEHOLDER.to_string());
        })])));

        let mut broken_user = report(vec![]);
        broken_user.user.full_name = PLACEHOLDER.to_string();
        assert!(contains_placeholder(&broken_user), "档案占位符也必须被拦");
    }

    /// 空资产清单不能渲染成空表格（继承人会以为文件坏了）。
    #[test]
    fn empty_report_explains_itself() {
        let html = render_report_html(&report(vec![]), ReportLocale::Zh);
        assert!(html.contains("尚未登记任何数字资产"));
    }

    /// 金额格式化：负值/NaN/None 一律显示 "-"，不得出现 ¥-5 或 ¥NaN。
    #[test]
    fn value_column_never_shows_invalid_amounts() {
        for bad in [None, Some(-5.0), Some(f64::NAN), Some(0.0)] {
            let html = render_report_html(
                &report(vec![asset(|a| a.estimated_value = bad)]),
                ReportLocale::Zh,
            );
            assert!(!html.contains("NaN"), "不得出现 NaN");
            assert!(!html.contains("¥-"), "不得出现负金额");
        }
    }

    /// 导出必须包含平台办理指引——预览里有、导出里没有，是家人拿到删节版的主因。
    #[test]
    fn export_includes_platform_guidance_like_the_preview() {
        let html = render_report_html(&report(vec![asset(|_| {})]), ReportLocale::Zh);
        assert!(html.contains("平台办理指引"), "必须有办理指引章节");
        assert!(
            html.contains("死亡证明、继承公证书、继承人身份证"),
            "必须含所需材料"
        );
        assert!(
            html.contains("联系客服 95188 办理余额继承"),
            "必须含办理流程"
        );
        assert!(html.contains("中等"), "必须含难度");
        assert!(html.contains("30-90天"), "必须含预计时长");
        assert!(html.contains("https://help.alipay.com"), "必须含官方入口");
        assert!(html.contains("大额需公证"), "必须含风险提示");
    }

    /// 继承人联系方式必须进导出：预览显示了，导出丢了，家人联系不上人。
    #[test]
    fn export_includes_heir_contact() {
        let html = render_report_html(&report(vec![asset(|_| {})]), ReportLocale::Zh);
        assert!(html.contains("13700000000"), "必须含继承人联系方式");
        assert!(html.contains("继承人联系方式"), "必须有该列");
    }

    /// 无来源标注的政策必须显式提示，不能让家人误以为是官方口径。
    #[test]
    fn unsourced_policies_are_marked_in_export() {
        let html = render_report_html(&report(vec![asset(|_| {})]), ReportLocale::Zh);
        assert!(html.contains("无来源标注"), "无来源必须标注");
        assert!(
            html.contains("办理前请向平台官方客服核实"),
            "必须给出核实建议"
        );

        let mut with_source = report(vec![asset(|_| {})]);
        with_source.policies = vec![policy(|p| p.has_source = true)];
        let html = render_report_html(&with_source, ReportLocale::Zh);
        assert!(html.contains("有来源标注"));
        assert!(!html.contains("现该条政策没有来源标注"));
    }

    /// 非 http(s) 的官方链接不得进报告：报告直接在浏览器打开，不能给出可执行协议。
    #[test]
    fn non_http_policy_links_are_dropped() {
        let mut r = report(vec![asset(|_| {})]);
        r.policies = vec![policy(|p| {
            p.official_policy_url = "javascript:alert(1)".to_string()
        })];
        let html = render_report_html(&r, ReportLocale::Zh);
        assert!(!html.contains("javascript:"), "非 http(s) 链接必须丢弃");
    }

    /// 导出必须跟随界面语言——否则英文用户拿到中文文档。
    #[test]
    fn english_locale_produces_english_document() {
        let html = render_report_html(&report(vec![asset(|_| {})]), ReportLocale::En);
        assert!(html.contains(r#"<html lang="en">"#), "lang 必须是 en");
        assert!(html.contains("Digital Legacy Inheritance Report"));
        assert!(html.contains("Holder information"));
        assert!(html.contains("Documents required"));
        assert!(
            !html.contains("数字遗产继承报告"),
            "英文报告不得混入中文标题"
        );
        assert!(!html.contains("指定继承"), "英文报告不得混入中文处置文案");
    }

    #[test]
    fn locale_parse_normalises_inputs() {
        assert_eq!(ReportLocale::parse("en-US"), ReportLocale::En);
        assert_eq!(ReportLocale::parse("EN"), ReportLocale::En);
        assert_eq!(ReportLocale::parse("en_us"), ReportLocale::En);
        assert_eq!(ReportLocale::parse("zh-CN"), ReportLocale::Zh);
        assert_eq!(ReportLocale::parse(""), ReportLocale::Zh);
        assert_eq!(ReportLocale::parse("fr"), ReportLocale::Zh);
    }
}

#[cfg(test)]
mod authority_tests {
    use super::*;
    use crate::models::{Authority, DigitalAsset, PlatformPolicyBrief, UserProfile};

    fn report_with(authorities: Vec<Authority>) -> InheritanceReport {
        InheritanceReport {
            user: UserProfile::default(),
            assets: vec![DigitalAsset {
                id: Some(1),
                platform_id: 1,
                platform_name: "支付宝".to_string(),
                account_identifier: "acc".to_string(),
                account_description: String::new(),
                category: "property".to_string(),
                estimated_value: Some(1.0),
                value_currency: "CNY".to_string(),
                disposal_method: "inherit".to_string(),
                heir_name: Some("王五".to_string()),
                heir_contact: None,
                notes: String::new(),
                has_credential: false,
                is_confirmed: true,
                created_at: String::new(),
                updated_at: String::new(),
            }],
            heirs: Vec::new(),
            policies: vec![PlatformPolicyBrief::default()],
            authorities,
            total_estimated_value: 1.0,
            property_assets_count: 1,
            personality_assets_count: 0,
            mixed_assets_count: 0,
            generated_at: "2026-10-26 12:00:00".to_string(),
        }
    }

    fn authority(matched: bool, kind: &str) -> Authority {
        Authority {
            id: "a".to_string(),
            kind: kind.to_string(),
            title_zh: "《民法典》".to_string(),
            title_en: "Civil Code".to_string(),
            publisher: "全国人大".to_string(),
            url: "https://flk.npc.gov.cn/".to_string(),
            note_zh: "数据与网络虚拟财产受法律保护".to_string(),
            note_en: "Data is protected".to_string(),
            applies_to: vec!["property".to_string()],
            reachable: true,
            content_matched: matched,
            last_checked: "2026-10-27".to_string(),
            http_status: 200,
            observed_title: "国家法律法规数据库".to_string(),
            portal_note_zh: "检索入口".to_string(),
            portal_note_en: "portal".to_string(),
        }
    }

    /// 报告必须列出权威来源，且「可达」与「内容已核对」分开写。
    #[test]
    fn report_lists_authorities_with_two_separate_facts() {
        let html = render_report_html(
            &report_with(vec![authority(true, "portal")]),
            ReportLocale::Zh,
        );
        assert!(html.contains("权威来源"));
        assert!(html.contains("入口可达"));
        assert!(html.contains("内容已核对"));
        assert!(html.contains("《民法典》"));
    }

    /// 内容未核对的来源必须显式标注，不能和已核对的长得一样。
    #[test]
    fn unmatched_authority_is_marked() {
        let html = render_report_html(
            &report_with(vec![authority(false, "institution")]),
            ReportLocale::Zh,
        );
        assert!(html.contains("入口可达"));
        assert!(!html.contains("内容已核对"), "未匹配不得显示为已核对");
    }

    /// 检索入口要写明「不是条文页」，避免读者以为点开就是法条。
    #[test]
    fn portal_is_labelled_as_portal() {
        let html = render_report_html(
            &report_with(vec![authority(true, "portal")]),
            ReportLocale::Zh,
        );
        assert!(html.contains("检索入口（非条文页）"));
    }

    #[test]
    fn authorities_are_escaped() {
        let mut a = authority(true, "portal");
        a.title_zh = "<script>alert(1)</script>".to_string();
        let html = render_report_html(&report_with(vec![a]), ReportLocale::Zh);
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
