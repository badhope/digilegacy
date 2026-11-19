// 通用审核管线：AI 初审 + 人工终审，**场景可插拔**。
//
// 为什么不能把某个具体场景写死在这里：
// 政策库核验、法律文书校对、家人说明改写、资产分类复核……它们共享同一条
// 管线（取候选 → AI 初审 → 人工终审 → 落库 → 门禁），差异只在
// 「审什么、按什么标准审、结果落到哪」。所以差异被抽象成 `ReviewProfile`，
// 管线本身不含任何业务名词。
//
// 五条不可让渡的规则（写进类型与断言，而不是靠注释）：
// 1. **AI 永远不能独自放行**：`AiVerdict::Pass` 只把条目推进到「待人工」，
//    最终状态只能由 `HumanVerdict` 决定。没有一个 API 可以让 AI 直接把
//    条目置为 `Approved`。
// 2. **AI 不可用时必须优雅降级**：没有端点/Key 时管线仍能跑完，只是
//    所有条目直接进入「待人工」。不能因为没配 AI 就让审核功能不可用。
// 3. **判定必须留痕**：每次审核写入一条不可变记录（谁、何时、依据哪一版、
//    AI 说了什么、人说了什么）。没有留痕的「已审核」等于没有审核。
// 4. **AI 只看得见该场景声明的字段**：`ReviewItem::payload` 由场景自己构造，
//    管线不做字段裁剪——但每个场景的 payload 构造必须遵守 ai_assist 的白名单。
// 5. **过期即失效**：条目内容变化后，旧判定自动作废（按内容指纹比对），
//    避免「审的是旧版、用的是新版」。

use serde::{Deserialize, Serialize};

/// AI 初审结论。**注意没有任何「通过」以外的终态**——
/// `Pass` 只表示「AI 没发现问题」，条目仍必须经过人工。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiVerdict {
    /// AI 未发现问题（仍需人工确认）
    Pass,
    /// AI 发现问题，附带理由
    Flag,
    /// AI 无法判断（信息不足、超时、未配置）
    Unknown,
    /// 未运行（没有可用的 AI 端点，或场景声明不需要 AI 初审）
    Skipped,
}

/// 人工终审结论。只有这里能产生「已通过」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanVerdict {
    Approved,
    Rejected,
    /// 退回修改（内容需要更新，不是判定为假）
    NeedsEdit,
}

/// 条目的最终状态。由「AI 初审 + 人工终审」共同决定，
/// 且 `Approved` 只能由 `HumanVerdict::Approved` 产生。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// 尚未审核
    Pending,
    /// AI 初审完成，等待人工
    AwaitingHuman,
    Approved,
    Rejected,
    NeedsEdit,
}

impl ReviewStatus {
    /// 唯一的推进入口。`HumanVerdict` 是必填参数——这是「AI 不能独自放行」
    /// 的类型级保证：没有任何重载能在缺少人工结论时得到 `Approved`。
    pub fn from_verdicts(ai: &AiVerdict, human: Option<&HumanVerdict>) -> Self {
        match human {
            Some(HumanVerdict::Approved) => ReviewStatus::Approved,
            Some(HumanVerdict::Rejected) => ReviewStatus::Rejected,
            Some(HumanVerdict::NeedsEdit) => ReviewStatus::NeedsEdit,
            None => match ai {
                AiVerdict::Skipped => ReviewStatus::Pending,
                _ => ReviewStatus::AwaitingHuman,
            },
        }
    }
}

/// 一条待审条目。管线不关心它是什么，只关心它的**内容指纹**与**送审文本**。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    /// 场景内的稳定标识（如平台 id、文书 id）。
    pub key: String,
    /// 人类可读标题，用于界面与留痕。
    pub title: String,
    /// 送审文本（由场景构造，必须遵守该场景的白名单）。
    pub payload: String,
    /// 内容指纹：内容变化即判定失效。
    pub fingerprint: String,
}

/// 审核严格度。
///
/// 「严格」不是提示词的语气，而是**机器可校验的判据**：
/// - `Strict` 要求 AI 逐项给出结论，缺任何一项直接降级为 `Unknown`；
/// - `Strict` 下 AI 报过的疑点，人工通过时必须写明理由（不能空备注放行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStrictness {
    Standard,
    Strict,
}

/// 严格模式下的必检项：`id` 用于校验 AI 是否逐项作答，`label` 用于提示词。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrictCheck {
    pub id: String,
    pub label: String,
}

/// 场景声明。加一个新场景 = 实现一个 profile，**不需要改管线**。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewProfile {
    /// 场景标识（如 `policy`、`legal-template`、`family-note`）。
    pub id: String,
    /// 界面显示名（i18n 键）。
    pub title_key: String,
    /// 审核标准：会作为 system prompt 的一部分发送。
    /// 必须写「发现什么问题要报什么」，而不是「请审查一下」。
    pub criteria: String,
    /// 该场景是否需要 AI 初审。`false` 表示纯人工场景，
    /// 管线会直接把条目置为 `Pending` 而不是伪造一个 `Skipped`。
    pub use_ai: bool,
    /// 严格度。严格模式要求逐项作答，且疑点必须人工说明理由后才能放行。
    #[serde(default = "default_strictness")]
    pub strictness: ReviewStrictness,
    /// 严格模式的必检项（为空则表示不需要逐项校验）。
    #[serde(default)]
    pub strict_checks: Vec<StrictCheck>,
}

fn default_strictness() -> ReviewStrictness {
    ReviewStrictness::Standard
}

/// 一次审核动作的留痕记录。**不可变**：写进去就不再修改。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub profile_id: String,
    pub item_key: String,
    /// 审核时该条目的内容指纹：内容再变则本记录作废。
    pub fingerprint: String,
    pub ai_verdict: AiVerdict,
    /// AI 给出的理由（`Flag`/`Unknown` 时尤其重要）。
    pub ai_note: String,
    pub human_verdict: Option<HumanVerdict>,
    pub human_note: String,
    /// `human` / `ai-only`（后者只可能出现在 `AwaitingHuman`）
    pub decided_by: String,
    pub created_at: String,
}

/// AI 初审提示词。刻意要求「只依据给定内容、必须给出可核对的位置」，
/// 并明确禁止它做最终判断——否则模型会倾向输出「通过」。
///
/// 严格模式下额外追加「逐项作答」要求：模型必须对每个必检项单独给结论，
/// 缺项由 `validate_strict_answer` 检出并作废本次初审。
pub fn ai_review_prompt(profile: &ReviewProfile, item: &ReviewItem) -> String {
    let strict_block = if profile.strictness == ReviewStrictness::Strict
        && !profile.strict_checks.is_empty()
    {
        let mut lines =
            String::from("\n\n严格模式：必须对下列每一项单独作答，缺任何一项本次初审作废。\n");
        for c in &profile.strict_checks {
            lines.push_str(&format!("- [{}] {}\n", c.id, c.label));
        }
        lines.push_str(
            "作答格式：先给出总判定行 PASS/FLAG/UNKNOWN，再逐项一行 `CHECK <id>: <FOUND|OK|NA> <一句话依据>`。",
        );
        lines
    } else {
        String::new()
    };

    format!(
        "你在为一个本地优先应用做内容初审。你**不是最终决策者**：你的结论只会用于提示人工复核。\n\
\n\
审核标准：\n{criteria}\n\
\n\
待审条目：《{title}》\n\
内容：\n{payload}\n\
\n\
输出要求（严格遵守）：\n\
1) 只依据上面给出的内容判断，不得引入外部知识或编造来源；\n\
2) 第一行只输出 PASS 或 FLAG 或 UNKNOWN 之一；\n\
3) 从第二行起，逐条说明依据，每条注明它对应原文的哪一句；\n\
4) 若内容不足以判断，输出 UNKNOWN 并说明缺什么；\n\
5) 不要给出法律意见，不要复述密码或任何凭据。{strict}",
        criteria = profile.criteria,
        title = item.title,
        payload = item.payload,
        strict = strict_block,
    )
}

/// 严格模式校验：逐项作答是否齐全、哪些项被判为 FOUND。
///
/// 返回 `(缺失项, 被标记项)`。**只要缺项，整条判定就必须降级为 Unknown**：
/// 模型跳过某一项往往正说明它没看，而不是「没问题」。
pub fn validate_strict_answer(profile: &ReviewProfile, answer: &str) -> (Vec<String>, Vec<String>) {
    if profile.strictness != ReviewStrictness::Strict || profile.strict_checks.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let upper = answer.to_uppercase();
    let mut missing = Vec::new();
    let mut flagged = Vec::new();
    for c in &profile.strict_checks {
        let tag = format!("CHECK {}:", c.id.to_uppercase());
        match upper.find(&tag) {
            None => missing.push(c.id.clone()),
            Some(pos) => {
                let rest = &upper[pos + tag.len()..];
                let line = rest.lines().next().unwrap_or("");
                if line.trim_start().starts_with("FOUND") {
                    flagged.push(c.id.clone());
                }
            }
        }
    }
    (missing, flagged)
}

/// 解析 AI 输出。宽容处理前缀噪声，但**宁可 Unknown 也不猜 Pass**：
/// 解析不出明确结论时一律 Unknown，避免「模型跑偏→被当成通过」。
pub fn parse_ai_verdict(raw: &str) -> (AiVerdict, String) {
    let trimmed = raw.trim();
    let first_line = trimmed.lines().next().unwrap_or("").trim().to_uppercase();
    let note = trimmed
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    // 只认首行的首个词，避免正文里出现「PASS」被误判。
    // 其余一切（含显式 UNKNOWN）都归为 Unknown：宁可让人多看一眼，
    // 也不能把无法解析的输出当成「通过」。
    let verdict = if first_line.starts_with("PASS") {
        AiVerdict::Pass
    } else if first_line.starts_with("FLAG") {
        AiVerdict::Flag
    } else {
        AiVerdict::Unknown
    };
    // 首行之外没有依据时，把首行剩余内容当作说明
    let note = if note.is_empty() {
        trimmed
            .strip_prefix(trimmed.lines().next().unwrap_or(""))
            .unwrap_or("")
            .trim()
            .to_string()
    } else {
        note
    };
    (verdict, note)
}

/// 判定是否仍然有效：内容指纹必须一致。
pub fn is_record_current(record: &ReviewRecord, current_fingerprint: &str) -> bool {
    record.fingerprint == current_fingerprint
}

/// 内容指纹：FNV-1a 64 位。用自实现而不是引入 sha 依赖——
/// 它只需要「内容变了就能发现」，不是安全用途。
pub fn fingerprint(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, use_ai: bool) -> ReviewProfile {
        ReviewProfile {
            id: id.to_string(),
            title_key: "review.title".to_string(),
            criteria: "检查是否编造出处".to_string(),
            use_ai,
            strictness: ReviewStrictness::Standard,
            strict_checks: Vec::new(),
        }
    }

    fn item(payload: &str) -> ReviewItem {
        ReviewItem {
            key: "k1".to_string(),
            title: "条目".to_string(),
            payload: payload.to_string(),
            fingerprint: fingerprint(payload),
        }
    }

    /// 最关键的一条：**AI 说 Pass 也不能变成 Approved**。
    #[test]
    fn ai_pass_alone_never_approves() {
        assert_eq!(
            ReviewStatus::from_verdicts(&AiVerdict::Pass, None),
            ReviewStatus::AwaitingHuman,
            "AI 未发现问题也只应进入待人工"
        );
        assert_eq!(
            ReviewStatus::from_verdicts(&AiVerdict::Pass, Some(&HumanVerdict::Approved)),
            ReviewStatus::Approved
        );
    }

    #[test]
    fn human_verdict_overrides_ai() {
        // AI 报了问题，人判断是误报 → 允许通过（人有权推翻机器）
        assert_eq!(
            ReviewStatus::from_verdicts(&AiVerdict::Flag, Some(&HumanVerdict::Approved)),
            ReviewStatus::Approved
        );
        // AI 说没问题，人判断有问题 → 拒绝
        assert_eq!(
            ReviewStatus::from_verdicts(&AiVerdict::Pass, Some(&HumanVerdict::Rejected)),
            ReviewStatus::Rejected
        );
    }

    #[test]
    fn skipped_ai_stays_pending_until_human() {
        assert_eq!(
            ReviewStatus::from_verdicts(&AiVerdict::Skipped, None),
            ReviewStatus::Pending
        );
        // 纯人工场景：人审通过即通过
        assert_eq!(
            ReviewStatus::from_verdicts(&AiVerdict::Skipped, Some(&HumanVerdict::Approved)),
            ReviewStatus::Approved
        );
    }

    /// 解析必须保守：任何不明确的输出都是 Unknown，绝不当成 Pass。
    #[test]
    fn parsing_is_conservative() {
        assert_eq!(parse_ai_verdict("PASS\n引用了第一段").0, AiVerdict::Pass);
        assert_eq!(parse_ai_verdict("FLAG\n来源为空").0, AiVerdict::Flag);
        assert_eq!(parse_ai_verdict("UNKNOWN\n缺少链接").0, AiVerdict::Unknown);
        // 大小写不敏感是有意的：模型经常输出 "pass"。
        assert_eq!(
            parse_ai_verdict(
                "pass
理由"
            )
            .0,
            AiVerdict::Pass
        );
        for garbage in ["", "   ", "好的，我看了一下", "我无法确定"] {
            assert_eq!(
                parse_ai_verdict(garbage).0,
                AiVerdict::Unknown,
                "无法解析时必须 Unknown：{:?}",
                garbage
            );
        }
        // 正文中出现 PASS 但首行不是结论 → 不能算通过
        assert_eq!(
            parse_ai_verdict("说明：这段内容写着 PASS").0,
            AiVerdict::Unknown
        );
    }

    #[test]
    fn parsing_keeps_reasons() {
        let (v, note) = parse_ai_verdict("FLAG\n1) 第 2 段提到的链接打不开");
        assert_eq!(v, AiVerdict::Flag);
        assert!(note.contains("链接打不开"));
    }

    /// 内容变化必须让旧判定失效，否则会出现「审的是旧版、用的是新版」。
    #[test]
    fn record_expires_when_content_changes() {
        let old = item("旧内容");
        let record = ReviewRecord {
            profile_id: "policy".to_string(),
            item_key: old.key.clone(),
            fingerprint: old.fingerprint.clone(),
            ai_verdict: AiVerdict::Pass,
            ai_note: String::new(),
            human_verdict: Some(HumanVerdict::Approved),
            human_note: String::new(),
            decided_by: "human".to_string(),
            created_at: "2026-10-26".to_string(),
        };
        assert!(is_record_current(&record, &old.fingerprint));
        let new = item("新内容");
        assert!(
            !is_record_current(&record, &new.fingerprint),
            "内容变化后旧审核必须失效"
        );
    }

    #[test]
    fn fingerprint_is_stable_and_sensitive() {
        assert_eq!(fingerprint("abc"), fingerprint("abc"));
        assert_ne!(fingerprint("abc"), fingerprint("abd"));
        assert_ne!(fingerprint(""), fingerprint(" "));
        assert_eq!(fingerprint("abc").len(), 16);
    }

    /// 提示词必须写明「不是最终决策者」与「不得编造」，
    /// 否则模型会顺着「帮他通过」的倾向输出。
    #[test]
    fn prompt_states_it_is_not_the_decider() {
        let p = ai_review_prompt(&profile("policy", true), &item("内容"));
        assert!(p.contains("不是最终决策者"));
        assert!(p.contains("不得引入外部知识或编造来源"));
        assert!(p.contains("只依据上面给出的内容"));
        assert!(p.contains("不要给出法律意见"));
    }

    /// 场景可插拔：同一套管线，两个 profile 只需给出不同的标准。
    #[test]
    fn profiles_are_data_not_code() {
        let a = profile("policy", true);
        let b = profile("legal-template", false);
        assert_ne!(a.id, b.id);
        assert!(a.use_ai && !b.use_ai, "纯人工场景应可声明不跑 AI");
        // 管线本身不出现任何业务名词
        let prompt = ai_review_prompt(&a, &item("内容"));
        assert!(prompt.contains(&a.criteria));
    }

    #[test]
    fn review_record_serialises_for_audit_trail() {
        let r = ReviewRecord {
            profile_id: "policy".to_string(),
            item_key: "1001".to_string(),
            fingerprint: "deadbeef".to_string(),
            ai_verdict: AiVerdict::Flag,
            ai_note: "缺来源".to_string(),
            human_verdict: Some(HumanVerdict::NeedsEdit),
            human_note: "补官方链接".to_string(),
            decided_by: "human".to_string(),
            created_at: "2026-10-26 10:00:00".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("needs_edit"));
        let back: ReviewRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.human_note, "补官方链接");
    }
}

#[cfg(test)]
mod strict_tests {
    use super::*;

    fn strict_profile() -> ReviewProfile {
        ReviewProfile {
            id: "legal".to_string(),
            title_key: "review.profile.legal".to_string(),
            criteria: "检查机械一致性".to_string(),
            use_ai: true,
            strictness: ReviewStrictness::Strict,
            strict_checks: vec![
                StrictCheck {
                    id: "signer".into(),
                    label: "签署主体".into(),
                },
                StrictCheck {
                    id: "blanks".into(),
                    label: "空白项".into(),
                },
                StrictCheck {
                    id: "conflict".into(),
                    label: "处置冲突".into(),
                },
            ],
        }
    }

    /// 提示词必须列出每一项，并要求 `CHECK <id>:` 格式。
    #[test]
    fn strict_prompt_requires_per_item_answers() {
        let p = ai_review_prompt(
            &strict_profile(),
            &ReviewItem {
                key: "1".into(),
                title: "遗嘱".into(),
                payload: "正文".into(),
                fingerprint: "f".into(),
            },
        );
        assert!(p.contains("严格模式"));
        assert!(p.contains("[signer] 签署主体"));
        assert!(p.contains("[blanks] 空白项"));
        assert!(p.contains("CHECK <id>"));
        assert!(p.contains("缺任何一项本次初审作废"));
    }

    /// 非严格场景不得出现严格模式说明（避免无谓的提示词膨胀）。
    #[test]
    fn standard_prompt_has_no_strict_block() {
        let mut prof = strict_profile();
        prof.strictness = ReviewStrictness::Standard;
        let p = ai_review_prompt(
            &prof,
            &ReviewItem {
                key: "1".into(),
                title: "t".into(),
                payload: "x".into(),
                fingerprint: "f".into(),
            },
        );
        assert!(!p.contains("严格模式"));
    }

    /// 缺项必须被检出——这是严格模式的核心。
    #[test]
    fn missing_items_are_detected() {
        let prof = strict_profile();
        let answer = "PASS\nCHECK signer: OK 已填\nCHECK blanks: OK 无遗留";
        let (missing, flagged) = validate_strict_answer(&prof, answer);
        assert_eq!(missing, vec!["conflict"], "必须指出缺少 conflict");
        assert!(flagged.is_empty());
    }

    /// 全部作答且无 FOUND → 无缺失、无标记。
    #[test]
    fn complete_clean_answer_passes_validation() {
        let prof = strict_profile();
        let answer = "PASS\nCHECK signer: OK\nCHECK blanks: OK\nCHECK conflict: OK";
        let (missing, flagged) = validate_strict_answer(&prof, answer);
        assert!(missing.is_empty());
        assert!(flagged.is_empty());
    }

    /// 某一项 FOUND 时，即使总判定写 PASS 也必须被单独列出。
    #[test]
    fn found_items_are_reported() {
        let prof = strict_profile();
        let answer =
            "PASS\nCHECK signer: OK\nCHECK blanks: FOUND 第 3 段仍有 ____\nCHECK conflict: OK";
        let (missing, flagged) = validate_strict_answer(&prof, answer);
        assert!(missing.is_empty());
        assert_eq!(flagged, vec!["blanks"]);
    }

    /// 大小写与空白不应导致误判（模型输出格式并不稳定）。
    #[test]
    fn validation_tolerates_case_and_spacing() {
        let prof = strict_profile();
        let answer = "pass\n  check signer: ok\nCheck Blanks: Found 有空白\nCHECK CONFLICT: NA";
        let (missing, flagged) = validate_strict_answer(&prof, answer);
        assert!(missing.is_empty(), "不应误报缺失: {:?}", missing);
        assert_eq!(flagged, vec!["blanks"]);
    }

    /// 标准场景不做逐项校验（空清单不应误报）。
    #[test]
    fn standard_profile_skips_validation() {
        let mut prof = strict_profile();
        prof.strictness = ReviewStrictness::Standard;
        let (missing, flagged) = validate_strict_answer(&prof, "PASS\n随便写");
        assert!(missing.is_empty());
        assert!(flagged.is_empty());
    }
}
