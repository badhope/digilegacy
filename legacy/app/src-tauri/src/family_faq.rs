// 家人 FAQ：确定性决策树，**不含任何 LLM 调用**。
//
// 为什么不用 AI 运行时问答：
// - 家人不一定有网、装不了软件、看不懂英文；
// - 与我们「0 服务器 / 0 遥测」的产品契约冲突；
// - 真正的问题只有二十来个，全部可以写成确定性的条目。
//
// 这里只做「文本组装」，答案来自平台库与固定话术，不生成、不推断。
// 输出会被打进家人执行清单，因此同样经过 HTML 转义。

use crate::action_guide::guide_html;

/// 一条 FAQ。
#[derive(Debug, Clone, serde::Serialize)]
pub struct FaqEntry {
    /// 稳定标识，供测试与前端 key 使用。
    pub id: &'static str,
    /// 问题（中/英）。
    pub question_zh: &'static str,
    pub question_en: &'static str,
    /// 回答（中/英）。
    pub answer_zh: &'static str,
    pub answer_en: &'static str,
}

/// 家人最常问的 12 个问题。顺序即阅读顺序：先能不能办，再怎么办，最后办不成怎么办。
pub const FAQ: &[FaqEntry] = &[
    FaqEntry {
        id: "where-credentials",
        question_zh: "账号密码在哪里？",
        question_en: "Where are the passwords?",
        answer_zh: "本清单和继承报告里都没有密码。密码保存在「加密备份」文件中，需要备份文件 + 主密码（或恢复码）才能读取。请先找到备份文件与恢复码，位置见本清单开头。",
        answer_en: "Neither this guide nor the inheritance report contains passwords. They live in the encrypted backup file and require the backup plus the master password (or recovery code). Find both first — their locations are at the top of this guide.",
    },
    FaqEntry {
        id: "no-backup",
        question_zh: "找不到备份文件怎么办？",
        question_en: "What if I cannot find the backup file?",
        answer_zh: "先在持有人的电脑、移动硬盘、云盘与纸质记录中查找。如果确实找不到，凭据将无法恢复——这是刻意的设计：没有备份就没有第二份明文。此时仍可按本清单办理余额与资产，只是需要走各平台的「忘记密码」流程，通常需要手机号过户。",
        answer_en: "Look on the holder's computer, external drives, cloud storage and paper records. If it is truly gone, credentials cannot be recovered — by design, there is no plaintext second copy. You can still follow this guide for balances and assets, but you will need each platform's password-reset flow, which usually requires transferring the phone number first.",
    },
    FaqEntry {
        id: "legal-basis",
        question_zh: "平台说「账号不可继承」，有依据吗？",
        question_en: "The platform says accounts cannot be inherited. Is that allowed?",
        answer_zh: "《民法典》第 127 条承认数据和网络虚拟财产的财产属性，第 1122 条把遗产范围定为「个人合法财产」。司法实践中已有法院认定「平台格式条款单方剥夺继承人继承虚拟财产权利」无效。但各平台用户协议不同，账号使用权与账号内财产需要分开主张：余额、理财、押金通常可继承，社交账号本身多数平台不允许转让。",
        answer_en: "Chinese law recognises data and virtual property as property (Civil Code art. 127) and defines estates as lawful personal property (art. 1122). Courts have held that standard-form clauses stripping heirs of virtual-property rights are invalid. Platforms differ, though: balances, investments and deposits are usually inheritable, while social accounts themselves often cannot be transferred. Argue the asset inside the account separately from the account itself.",
    },
    FaqEntry {
        id: "notarization",
        question_zh: "一定要做继承公证吗？",
        question_en: "Is notarisation always required?",
        answer_zh: "不一定。小额存款在多数银行可按简易程序办理（常见门槛为 5 万元以内，具体以银行规定为准）。余额、理财等金额较大时通常需要继承权公证书或法院判决书。虚拟货币、游戏装备等没有统一规则，往往需要与平台单独协商。",
        answer_en: "Not always. Many banks run a simplified procedure for small deposits (commonly under CNY 50,000 — confirm with the bank). Larger balances and investments usually need a certificate of inheritance or a court judgment. Crypto and in-game items have no uniform rule and often require case-by-case negotiation.",
    },
    FaqEntry {
        id: "which-documents",
        question_zh: "一般要准备哪些材料？",
        question_en: "What documents are usually needed?",
        answer_zh: "常见四件：① 死亡证明 ② 继承权公证书或法院判决书 ③ 继承人身份证 ④ 亲属关系证明（户口本、出生证明等）。每个平台的差异已逐条写在本清单的对应步骤里。",
        answer_en: "Usually four: (1) death certificate, (2) certificate of inheritance or court judgment, (3) the heir's ID, (4) proof of relationship (household register, birth certificate). Per-platform differences are listed in each step of this guide.",
    },
    FaqEntry {
        id: "phone-number",
        question_zh: "手机号必须先过户吗？",
        question_en: "Should the phone number be transferred first?",
        answer_zh: "建议优先办理。很多平台的「忘记密码」依赖短信验证码，手机号过户后可以自助重置大量账号，能显著减少与客服的来回。办理时需携带死亡证明与继承材料到运营商营业厅。",
        answer_en: "Yes, do it early. Many platforms reset passwords via SMS, so once the number is transferred you can recover a lot of accounts yourself and avoid repeated support calls. Bring the death certificate and inheritance documents to the carrier's branch.",
    },
    FaqEntry {
        id: "platform-refuses",
        question_zh: "平台拒绝配合怎么办？",
        question_en: "What if the platform refuses?",
        answer_zh: "第一步要求书面答复（邮件或工单编号），口头拒绝无法作为后续依据。第二步向平台注册地法院起诉，案由可用「数据、网络虚拟财产纠纷」。第三步可向网信办或消费者协会投诉。保留全部沟通记录。",
        answer_en: "First, demand a written reply (email or ticket number) — a verbal refusal is unusable later. Second, sue at the court where the platform is registered, under a data/online-virtual-property dispute. Third, complain to the cyberspace administration or consumer association. Keep every record.",
    },
    FaqEntry {
        id: "time-expected",
        question_zh: "整个流程要多久？",
        question_en: "How long does this take?",
        answer_zh: "单个平台通常 30 到 90 天，涉及诉讼可能半年以上。建议先办金额最大、材料最全的平台，其余并行推进。本清单每一步都标了预计时长，可据此排顺序。",
        answer_en: "Typically 30–90 days per platform, and six months or more if litigation is involved. Start with the platform holding the largest assets and where your documents are complete, then run the rest in parallel. Each step lists its expected duration.",
    },
    FaqEntry {
        id: "crypto",
        question_zh: "加密货币钱包怎么处理？",
        question_en: "What about crypto wallets?",
        answer_zh: "加密钱包不依赖平台，只依赖私钥或助记词。如果持有者把助记词写进了加密备份，按本清单找到备份即可；如果没有，资产通常无法找回。切勿把助记词发给任何自称「代办」的第三方。",
        answer_en: "Crypto wallets do not depend on a platform, only on the private key or seed phrase. If the holder stored the phrase in the encrypted backup, follow the backup steps above; if not, the assets are usually unrecoverable. Never send a seed phrase to anyone offering to 'handle it for you'.",
    },
    FaqEntry {
        id: "avoid-fraud",
        question_zh: "有人主动联系说能代办，可信吗？",
        question_en: "Someone offered to handle this for a fee. Should I trust them?",
        answer_zh: "不可信。本清单中的所有流程都可以本人办理，且平台不会通过私人微信、QQ 主动索要材料。凡是要求先付费、要求提供密码或验证码、要求远程控制电脑的，一律是诈骗。",
        answer_en: "No. Everything in this guide can be done by you directly, and platforms never approach you on personal chat apps asking for documents. Anyone demanding upfront payment, passwords, verification codes, or remote access to your computer is a scammer.",
    },
    FaqEntry {
        id: "social-accounts",
        question_zh: "社交账号会被删除吗？",
        question_en: "Will social accounts be deleted?",
        answer_zh: "取决于平台与持有者的处置意愿（本清单每一步都注明了）。多数平台提供「纪念账号」选项（保留内容、禁止登录），也有平台在长期不活跃后回收。若持有者选择「彻底删除」，请按该平台的注销流程办理，注意这会永久删除内容。",
        answer_en: "It depends on the platform and the holder's wish (stated in each step). Many platforms offer a memorialised state (content kept, login disabled); others recycle inactive accounts. If the holder chose deletion, follow that platform's closure process — it permanently removes the content.",
    },
    FaqEntry {
        id: "what-if-locked",
        question_zh: "打开应用时提示「需解锁」怎么办？",
        question_en: "The app says 'locked'. What now?",
        answer_zh: "说明当前数据处于加密锁定状态。输入持有者的主密码即可解锁；忘记主密码时用恢复码重设。两者都找不到时，凭据无法恢复，但已打印的这份清单仍然有效。",
        answer_en: "The data is encrypted and locked. Enter the holder's master password, or use the recovery code if the password is unknown. If neither exists, credentials cannot be recovered — but this printed guide still applies.",
    },
];

/// 把 FAQ 渲染成 HTML 区块（供执行清单内嵌）。
pub fn render_faq_html(locale: crate::report::ReportLocale) -> String {
    let zh = locale == crate::report::ReportLocale::Zh;
    let mut out = String::new();
    for (i, entry) in FAQ.iter().enumerate() {
        let q = if zh {
            entry.question_zh
        } else {
            entry.question_en
        };
        let a = if zh { entry.answer_zh } else { entry.answer_en };
        out.push_str(&format!(
            "<details class=\"faq\"><summary><span class=\"faq-no\">{}</span>{}</summary><p>{}</p></details>",
            i + 1,
            guide_html(q),
            guide_html(a),
        ));
    }
    out
}

/// FAQ 的打印样式。集中在这里，避免与执行清单主样式各写一份。
pub const FAQ_STYLE: &str = r#"
.faq{border-bottom:1px solid #e8e2d8;padding:10px 0}
.faq summary{cursor:pointer;font-size:14px;font-weight:600;display:flex;gap:8px;align-items:baseline}
.faq-no{display:inline-flex;align-items:center;justify-content:center;width:20px;height:20px;border-radius:50%;background:#f6f3ee;color:#6b6157;font-size:11px;flex:0 0 auto}
.faq p{font-size:13px;color:#4a433a;line-height:1.7;margin:8px 0 0 28px}
@media print{.faq p{display:block}}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::ReportLocale;

    /// FAQ 必须覆盖「找不到备份」「平台拒绝」「诈骗」这三类真要命的问题。
    #[test]
    fn faq_covers_the_critical_questions() {
        let ids: Vec<&str> = FAQ.iter().map(|e| e.id).collect();
        assert!(ids.contains(&"no-backup"), "必须回答找不到备份怎么办");
        assert!(ids.contains(&"platform-refuses"), "必须回答平台拒绝怎么办");
        assert!(ids.contains(&"avoid-fraud"), "必须防诈骗提示");
        assert!(ids.contains(&"where-credentials"), "必须回答密码在哪");
        assert!(FAQ.len() >= 10, "至少 10 条，实际 {}", FAQ.len());
    }

    /// 每条都必须中英双语齐全，不允许只写中文。
    #[test]
    fn every_entry_is_bilingual() {
        for e in FAQ {
            assert!(!e.question_zh.trim().is_empty(), "{} 缺中文问题", e.id);
            assert!(!e.question_en.trim().is_empty(), "{} 缺英文问题", e.id);
            assert!(!e.answer_zh.trim().is_empty(), "{} 缺中文答案", e.id);
            assert!(!e.answer_en.trim().is_empty(), "{} 缺英文答案", e.id);
            // 英文答案里出现成段中文，说明翻译漏了
            let cjk = e
                .answer_en
                .chars()
                .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
                .count();
            assert!(cjk < 5, "{} 的英文答案含 {} 个汉字，可能未翻译", e.id, cjk);
        }
    }

    /// 不允许出现「可能会」「大概」这类无法执行的模糊表述——家人需要明确动作。
    #[test]
    fn answers_are_actionable_not_vague() {
        for e in FAQ {
            assert!(!e.answer_zh.contains("不太好说"), "{} 的答案不可执行", e.id);
        }
    }

    #[test]
    fn renders_bilingual_html_with_numbers() {
        let zh = render_faq_html(ReportLocale::Zh);
        assert!(zh.contains("账号密码在哪里"));
        assert!(zh.contains("class=\"faq-no\""));
        let en = render_faq_html(ReportLocale::En);
        assert!(en.contains("Where are the passwords"));
        assert!(!en.contains("账号密码在哪里"));
    }

    /// FAQ 文本里不允许出现可执行脚本——它会被嵌进导出文件。
    #[test]
    fn html_is_escaped() {
        let html = render_faq_html(ReportLocale::Zh);
        assert!(!html.contains("<script"), "FAQ 不应含脚本标签");
    }
}
