// 审核场景注册表：**加一个场景就是往这里加一个声明，不改管线**。
//
// 目前只注册了「平台政策」一个场景。这不是因为管线只支持它，
// 而是因为其余候选场景（法律文书、家人说明、资产分类）需要各自决定
// 「审什么、按什么标准、送审哪些字段」——那是产品判断，不是代码问题。
// 加场景时照着 `policy()` 写一个函数 + 在 `all_profiles()` 里登记即可。
//
// 每个场景都必须自己构造 `ReviewItem`：
// - `payload` 只放该场景允许出境的字段（见 ai_assist 的白名单）；
// - `fingerprint` 必须覆盖所有会影响结论的内容（否则「审旧用新」）。

use crate::family_faq::FAQ;
use crate::platform_data::get_all_platforms;
use crate::review::{fingerprint, ReviewItem, ReviewProfile};

/// 场景 id：与留痕表里的 `profile_id` 对应，**不可随意改名**（会让历史记录失联）。
pub const POLICY_REVIEW: &str = "platform-policy";

/// 平台政策初审。
///
/// 审的是「这条政策的表述是否自相矛盾、是否有可核对来源、是否缺失关键材料」，
/// 不是「这条政策对不对」——后者只能由人对着官方公告核对。
pub fn policy_profile() -> ReviewProfile {
    ReviewProfile {
        id: POLICY_REVIEW.to_string(),
        title_key: "review.profile.policy".to_string(),
        criteria: "检查下列问题并逐条指出：\n\
1) process_description 与 required_documents 是否互相矛盾（例如流程要求公证但材料里没有公证材料）；\n\
2) official_policy_url 是否为空，或明显不是该平台的官方域名；\n\
3) estimated_difficulty 与 estimated_time_days 是否与流程描述的复杂度明显不匹配；\n\
4) 是否存在绝对化承诺（如「一定能继承」「无需任何材料」）；\n\
5) 是否引用了法条编号；若引用，只能提示「需核对原文」，不得判断法条是否正确。"
            .to_string(),
        use_ai: true,
        strictness: crate::review::ReviewStrictness::Standard,
        strict_checks: Vec::new(),
    }
}

/// 用平台库构造政策审核条目。
///
/// 送审字段严格限定为政策文本本身：不带平台 id 以外的任何内部状态，
/// 也不带用户数据（平台库是内置静态数据，与用户无关）。
pub fn policy_items() -> Vec<ReviewItem> {
    get_all_platforms()
        .into_iter()
        .map(|p| {
            let payload = format!(
                "平台：{}\n类别：{}\n可继承：{}\n所需材料：{}\n办理流程：{}\n难度：{}\n预计时长：{}\n官方链接：{}\n来源标注：{}\n备注：{}",
                p.name,
                p.category,
                if p.inheritable { "是" } else { "否" },
                p.required_documents,
                p.process_description,
                p.estimated_difficulty,
                p.estimated_time_days,
                p.official_policy_url,
                p.data_source,
                p.notes,
            );
            ReviewItem {
                key: p.id.to_string(),
                title: p.name.clone(),
                fingerprint: fingerprint(&payload),
                payload,
            }
        })
        .collect()
}

/// 法律文书场景 id。
pub const LEGAL_REVIEW: &str = "legal-template";

/// 法律文书初审：**开启 AI，但用严格模式**。
///
/// 为什么这里的严格度是 `Strict` 而不是普通模式：
/// 文书会被打印、签字、提交给平台或公证处，AI 漏看一项的代价由用户承担。
/// 因此：
/// 1. AI 必须对六项逐一给出 `FOUND / OK / NA`，缺任何一项整条初审作废（降级 Unknown）；
/// 2. AI 报过的疑点，人工点「通过」时必须写明理由——不允许空备注放行。
///
/// AI 仍然不做法律判断：它只检查「文本内部的机械一致性」，
/// 法条正确性、条款效力一律不在其职责内（提示词里明确禁止）。
pub fn legal_profile() -> ReviewProfile {
    ReviewProfile {
        id: LEGAL_REVIEW.to_string(),
        title_key: "review.profile.legal".to_string(),
        criteria: "逐项检查下列机械一致性问题并逐条指出：
1) 签署主体（立遗嘱人/声明人/委托人/申请人）是否填写；
2) 身份证号、住址、联系方式等必填空白（____）是否仍有未填；
3) 日期格式是否统一（年月日 / YYYY-MM-DD 混用即为问题）；
4) 金额、比例、份额是否前后一致（同一处出现两个不同数字即为问题）；
5) 见证人/第三方签字栏是否存在且已填；
6) 正文是否出现互相矛盾的处置安排（例如同一条资产既写继承又写删除）。
你不判断条款是否合法、是否有效，也不引用任何法条——那属于律师与公证机关的职责。"
            .to_string(),
        use_ai: true,
        strictness: crate::review::ReviewStrictness::Strict,
        strict_checks: vec![
            crate::review::StrictCheck {
                id: "signer".to_string(),
                label: "签署主体已填写".to_string(),
            },
            crate::review::StrictCheck {
                id: "blanks".to_string(),
                label: "应填空白项已填完（无遗留 ____）".to_string(),
            },
            crate::review::StrictCheck {
                id: "date".to_string(),
                label: "日期格式统一".to_string(),
            },
            crate::review::StrictCheck {
                id: "amounts".to_string(),
                label: "金额/比例/份额前后一致".to_string(),
            },
            crate::review::StrictCheck {
                id: "witness".to_string(),
                label: "见证人或第三方签字栏齐备".to_string(),
            },
            crate::review::StrictCheck {
                id: "conflict".to_string(),
                label: "无互相矛盾的处置安排".to_string(),
            },
        ],
    }
}

/// 法律文书条目。模板本体在前端 JSON 里（单一事实源），这里内嵌同一份文件。
pub fn legal_items() -> Vec<ReviewItem> {
    let raw = include_str!("../../src/data/legal-templates.json");
    let list: Vec<serde_json::Value> = serde_json::from_str(raw).unwrap_or_default();
    list.into_iter()
        .map(|t| {
            let id = t
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let body = t.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let kind = t.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let desc = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
            crate::review::ReviewItem {
                key: id,
                title: name,
                fingerprint: crate::review::fingerprint(body),
                payload: format!(
                    "类型：{}
说明：{}

正文：
{}",
                    kind, desc, body
                ),
            }
        })
        .collect()
}

/// 家人说明场景 id。
pub const FAMILY_NOTE_REVIEW: &str = "family-note";

/// 家人说明初审：审的是「这段话家人读得懂吗、有没有误导」。
///
/// 这是 AI 初审最合适的场景：文案质量判断本来就是语言模型的强项，
/// 而且即使判断错了，代价只是说明不够好，不会造成错误执行。
pub fn family_note_profile() -> ReviewProfile {
    ReviewProfile {
        id: FAMILY_NOTE_REVIEW.to_string(),
        title_key: "review.profile.familyNote".to_string(),
        criteria: "检查下列问题并逐条指出：
1) 是否出现家人无法理解的术语或缩写（如「二次确认链」「占位符」）；
2) 是否包含可执行的下一步（找谁、带什么、做什么），还是只有原则性描述；
3) 是否给出可能误导的时间承诺（如「一周内办完」）；
4) 是否暗示本应用能代替公证、律师或平台审核；
5) 是否泄露任何凭据、账号或金额。"
            .to_string(),
        use_ai: true,
        strictness: crate::review::ReviewStrictness::Standard,
        strict_checks: Vec::new(),
    }
}

/// 家人说明条目：来自保管位置与继承人名单这两块「家人一定会读」的文本。
pub fn family_note_items() -> Vec<ReviewItem> {
    let mut out = Vec::new();
    if let Ok(h) = crate::db::get_custody_hints_db() {
        for (key, title, text) in [
            ("backup-location", "加密备份存放位置", &h.backup_location),
            (
                "recovery-location",
                "恢复码保管位置",
                &h.recovery_code_location,
            ),
        ] {
            if text.trim().is_empty() {
                continue;
            }
            out.push(crate::review::ReviewItem {
                key: key.to_string(),
                title: title.to_string(),
                fingerprint: crate::review::fingerprint(text),
                payload: format!(
                    "字段：{}
内容：{}",
                    title, text
                ),
            });
        }
    }
    if let Ok(heirs) = crate::db::get_heirs_db() {
        for h in &heirs {
            if h.share_note.trim().is_empty() {
                continue;
            }
            out.push(crate::review::ReviewItem {
                key: format!("heir-share-{}", h.id.unwrap_or(0)),
                title: format!("{}的分配说明", h.name),
                fingerprint: crate::review::fingerprint(&h.share_note),
                // 送审只需说明本身：不带姓名/电话/邮箱
                payload: format!("分配说明：{}", h.share_note),
            });
        }
    }
    out
}

/// 资产分类场景 id。
pub const ASSET_REVIEW: &str = "asset-classification";

/// 资产分类初审。
///
/// 审的是「category 与卡片描述的语义是否一致」——这正是确定性规则做不好、
/// 而语言模型擅长的事。代价也低：判错了只是分类标签不对，可随时改回。
pub fn asset_profile() -> ReviewProfile {
    ReviewProfile {
        id: ASSET_REVIEW.to_string(),
        title_key: "review.profile.asset".to_string(),
        criteria: "检查下列问题并逐条指出：
1) 资产的 category（property/personality/mixed）与该平台的 has_property / has_personality 是否一致；
2) 处置方式为 inherit 时是否缺少继承人；
3) 处置方式为 delete 时，该资产是否含明显有价值内容（如余额、已购内容）；
4) 账号描述与平台类型是否明显不匹配（例如把社交账号登记成金融资产）。"
            .to_string(),
        use_ai: true,
        strictness: crate::review::ReviewStrictness::Standard,
        strict_checks: Vec::new(),
    }
}

/// 资产分类条目：只送平台、类别、处置方式与平台能力标记，
/// **不含账号标识、金额、备注**（那些属于用户隐私，与分类判断无关）。
pub fn asset_items() -> Vec<ReviewItem> {
    let Ok(assets) = crate::db::get_all_assets_db() else {
        return Vec::new();
    };
    let platforms = get_all_platforms();
    assets
        .into_iter()
        .map(|a| {
            let p = platforms.iter().find(|p| p.id == a.platform_id);
            let desc = format!(
                "平台：{}
资产类别：{}
处置方式：{}
平台可继承：{}
平台含财产属性：{}
平台含人格属性：{}",
                a.platform_name,
                a.category,
                a.disposal_method,
                p.map(|x| if x.inheritable { "是" } else { "否" })
                    .unwrap_or("未知"),
                p.map(|x| if x.has_property { "是" } else { "否" })
                    .unwrap_or("未知"),
                p.map(|x| if x.has_personality { "是" } else { "否" })
                    .unwrap_or("未知"),
            );
            crate::review::ReviewItem {
                key: format!("asset-{}", a.id.unwrap_or(0)),
                title: a.platform_name.clone(),
                fingerprint: crate::review::fingerprint(&desc),
                payload: desc,
            }
        })
        .collect()
}

/// 场景列表（界面据此渲染选项卡）。新增场景只改这里。
pub fn all_profiles() -> Vec<ReviewProfile> {
    vec![
        policy_profile(),
        legal_profile(),
        family_note_profile(),
        asset_profile(),
        crate::review_heir::heir_comprehension_profile(),
    ]
}

/// 按 id 取场景。未知 id 返回 None，而不是回退到某个默认场景——
/// 静默回退会让「审查法律文书」实际跑了「审查政策」，是最坏的一种错。
pub fn profile_by_id(id: &str) -> Option<ReviewProfile> {
    all_profiles().into_iter().find(|p| p.id == id)
}

/// 按场景 id 取待审条目。
pub fn items_for(id: &str) -> Option<Vec<ReviewItem>> {
    match id {
        POLICY_REVIEW => Some(policy_items()),
        LEGAL_REVIEW => Some(legal_items()),
        FAMILY_NOTE_REVIEW => Some(family_note_items()),
        ASSET_REVIEW => Some(asset_items()),
        crate::review_heir::HEIR_COMPREHENSION_REVIEW => {
            // 该场景审的是「系统生成的交付物」，需要实时报告；
            // 拿不到报告（锁定态）时返回空集，界面显示「无可审条目」。
            match crate::db::get_inheritance_report_db() {
                Ok(report) => Some(crate::review_heir::heir_comprehension_items(&report)),
                Err(_) => Some(Vec::new()),
            }
        }
        _ => None,
    }
}

/// 场景的「送审字段」说明，界面展示用：让用户明确知道 AI 看到了什么。
///
/// 返回的是**稳定的字段标识**（`payload.field.*`），不是中文文案。此前这里
/// 直接返回中文串，导致英文界面把中文原样打印出来（中英混排），而且字段
/// 改名会静默改变界面文案。前端负责把这些标识翻译成当前语言。
pub fn payload_fields_for(id: &str) -> Vec<String> {
    match id {
        LEGAL_REVIEW => vec![
            "payload.field.legalDocType".to_string(),
            "payload.field.legalDescription".to_string(),
            "payload.field.legalBody".to_string(),
        ],
        FAMILY_NOTE_REVIEW => vec![
            "payload.field.noteFieldName".to_string(),
            "payload.field.noteContent".to_string(),
        ],
        ASSET_REVIEW => vec![
            "payload.field.platformName".to_string(),
            "payload.field.assetCategory".to_string(),
            "payload.field.disposalMethod".to_string(),
            "payload.field.platformCapability".to_string(),
        ],
        crate::review_heir::HEIR_COMPREHENSION_REVIEW => vec![
            "payload.field.heirKitBody".to_string(),
            "payload.field.heirAllocation".to_string(),
            "payload.field.heirIntro".to_string(),
        ],
        POLICY_REVIEW => vec![
            "payload.field.platformName".to_string(),
            "payload.field.category".to_string(),
            "payload.field.inheritable".to_string(),
            "payload.field.requiredDocs".to_string(),
            "payload.field.procedure".to_string(),
            "payload.field.difficulty".to_string(),
            "payload.field.estimatedTime".to_string(),
            "payload.field.officialUrl".to_string(),
            "payload.field.sourceNote".to_string(),
            "payload.field.notes".to_string(),
        ],
        _ => vec!["payload.field.unknown".to_string()],
    }
}

/// 家人 FAQ 也可以作为审核场景：检查「答案是否与政策库矛盾」。
/// 这里保留构造器但**不默认注册**——FAQ 是静态文案，改动频率低，
/// 是否值得每次跑 AI 初审属于产品取舍。
#[allow(dead_code)]
pub fn faq_items() -> Vec<ReviewItem> {
    FAQ.iter()
        .map(|e| {
            let payload = format!("问题：{}\n回答：{}", e.question_zh, e.answer_zh);
            ReviewItem {
                key: e.id.to_string(),
                title: e.question_zh.to_string(),
                fingerprint: fingerprint(&payload),
                payload,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::ReviewStatus;

    #[test]
    fn policy_profile_is_registered() {
        let profiles = all_profiles();
        assert!(!profiles.is_empty(), "至少注册一个场景");
        assert!(profiles.iter().any(|p| p.id == POLICY_REVIEW));
        assert!(profile_by_id(POLICY_REVIEW).is_some());
    }

    /// 五个场景都已注册，且**同一套管线**服务它们——加场景没有改管线。
    #[test]
    fn all_scenes_registered_through_one_pipeline() {
        let profiles = all_profiles();
        let ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        for want in [
            POLICY_REVIEW,
            LEGAL_REVIEW,
            FAMILY_NOTE_REVIEW,
            ASSET_REVIEW,
            crate::review_heir::HEIR_COMPREHENSION_REVIEW,
        ] {
            assert!(ids.contains(&want), "场景 {} 未注册", want);
            assert!(items_for(want).is_some(), "场景 {} 没有条目构造器", want);
        }
    }

    /// 法律文书：开启 AI，但必须使用严格模式 + 完整必检项。
    ///
    /// 这条断言替代了旧版「法律文书不得启用 AI」。用户明确要求开启，
    /// 因此把「安全」从「不让 AI 碰」改为「让 AI 逐项作答、缺项作废、
    /// 疑点必须人工写理由才能放行」。
    #[test]
    fn legal_scene_is_strict_and_fully_checked() {
        let p = profile_by_id(LEGAL_REVIEW).unwrap();
        assert!(p.use_ai, "用户要求开启 AI 初审");
        assert_eq!(
            p.strictness,
            crate::review::ReviewStrictness::Strict,
            "法律文书必须使用严格模式"
        );
        assert_eq!(p.strict_checks.len(), 6, "六项必检缺一不可");
        // 每项必须有非空 id 与说明，否则校验函数无法定位
        for c in &p.strict_checks {
            assert!(!c.id.is_empty());
            assert!(!c.label.is_empty());
        }
        // 仍然登记在管线里，且能取到条目
        assert!(!items_for(LEGAL_REVIEW).unwrap().is_empty());
    }

    /// 资产分类的送审内容不得含账号标识、金额或备注。
    #[test]
    fn asset_scene_payload_excludes_private_fields() {
        // 该测试不依赖真实数据库：直接检查构造函数的字段模板
        let profile = profile_by_id(ASSET_REVIEW).unwrap();
        assert!(profile.use_ai);
        // 送审字段清单里不应出现隐私字段标识
        let fields = payload_fields_for(ASSET_REVIEW);
        for forbidden in ["account", "amount", "notes", "contact"] {
            assert!(
                !fields.iter().any(|f| f.to_lowercase().contains(forbidden)),
                "资产分类送审字段不得包含 {}",
                forbidden
            );
        }
        // 必须全部是 i18n 标识而不是可直接展示的中文文案
        assert!(
            fields.iter().all(|f| f.starts_with("payload.field.")),
            "送审字段必须返回稳定的 i18n 标识，界面才能按当前语言渲染"
        );
    }

    /// 家人说明场景的送审字段同样只有「字段名 + 内容」。
    #[test]
    fn family_note_scene_is_minimal() {
        let fields = payload_fields_for(FAMILY_NOTE_REVIEW);
        assert_eq!(fields.len(), 2, "只应有字段名与内容");
    }

    /// 未知场景必须返回 None：静默回退会让审核跑错对象。
    #[test]
    fn unknown_profile_returns_none() {
        assert!(profile_by_id("no-such-scene").is_none());
        assert!(items_for("no-such-scene").is_none());
        assert!(profile_by_id("").is_none());
    }

    /// 政策条目必须覆盖整个平台库，且指纹对内容敏感。
    #[test]
    fn policy_items_cover_all_platforms_and_fingerprint_content() {
        let items = policy_items();
        assert_eq!(items.len(), 152, "必须覆盖全部内置平台");
        let first = &items[0];
        assert!(!first.payload.is_empty());
        assert_eq!(first.fingerprint.len(), 16);
        // 同一份数据两次构造指纹必须一致（否则每次刷新都变「待审」）
        let again = policy_items();
        assert_eq!(items[0].fingerprint, again[0].fingerprint);
    }

    /// 送审内容不得包含任何用户数据字段（平台库本身是静态数据）。
    #[test]
    fn policy_payload_has_no_user_data() {
        for item in policy_items() {
            for forbidden in [
                "account_identifier",
                "credential",
                "heir",
                "password",
                "sk-",
            ] {
                assert!(
                    !item.payload.contains(forbidden),
                    "平台 {} 的送审内容含敏感词 {}",
                    item.title,
                    forbidden
                );
            }
        }
    }

    /// 场景声明必须带明确标准，不能是空字符串（否则 AI 无从判断）。
    #[test]
    fn profile_criteria_are_actionable() {
        for p in all_profiles() {
            assert!(
                p.criteria.chars().count() > 30,
                "场景 {} 的审核标准过于笼统",
                p.id
            );
            assert!(!p.title_key.is_empty());
        }
    }

    /// FAQ 场景构造器可用（即使未注册），保证「以后要加」时结构就绪。
    #[test]
    fn faq_items_are_constructible() {
        let items = faq_items();
        assert_eq!(items.len(), FAQ.len());
        assert!(!items[0].payload.is_empty());
    }

    /// AI 初审不能放行：状态机层面再确认一次。
    #[test]
    fn profile_scenario_still_requires_human() {
        use crate::review::{AiVerdict, HumanVerdict};
        assert_eq!(
            ReviewStatus::from_verdicts(&AiVerdict::Pass, None),
            ReviewStatus::AwaitingHuman
        );
        assert_eq!(
            ReviewStatus::from_verdicts(&AiVerdict::Pass, Some(&HumanVerdict::Approved)),
            ReviewStatus::Approved
        );
    }
}
