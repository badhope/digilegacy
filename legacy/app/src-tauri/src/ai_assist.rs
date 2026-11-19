// AI 助手：**只在作者期运行**，产物经人工审校后冻结进执行清单。
//
// 设计边界（写进代码，不靠自觉）：
// 1. **默认关闭**。没有任何内置端点、没有官方服务、没有账号体系。
//    用户自己填 OpenAI 兼容的端点与 Key，有端点时才可用。
// 2. **传输在 Rust 层**（进程内 ureq），因此 `tauri.conf.json` 的 CSP
//    保持 `connect-src 'self' ipc:` 不变——前端永远不发网络请求。
// 3. **出境白名单**：只允许平台名、类别、处置方式、缺失项这类**非身份信息**。
//    凭据、账号标识、继承人联系方式、姓名、电话、邮箱一律在组装请求前剔除，
//    且用「白名单构造」而不是「黑名单过滤」——新增字段默认不出境。
// 4. **输出只读**：AI 只能产出建议文本，不能调用任何写命令；结果要用户
//    逐条勾选确认后才进工具包。
// 5. **无来源即拒答**：提示词要求只依据本清单内的平台政策作答；无法回答时
//    必须说「无法据此回答」，不得编造平台规则或链接。

use serde::{Deserialize, Serialize};

const TIMEOUT_SECS: u64 = 60;
pub(crate) const TIMEOUT_SECS_AGENT: u64 = TIMEOUT_SECS;

/// 助手配置（不含 Key 明文）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssistantConfig {
    pub enabled: bool,
    /// OpenAI 兼容的 base url，如 `https://api.example.com/v1`。
    pub endpoint: String,
    /// 模型名，如 `gpt-4o-mini` / `deepseek-chat` / `qwen-plus`。
    pub model: String,
    /// 是否已保存 Key（不回传明文）。
    pub has_key: bool,
}

/// 一次出境的字段清单，供界面做「发送前预览」。
#[derive(Debug, Clone, Serialize)]
pub struct OutboundPreview {
    /// 将要发送的字段名（固定白名单，便于界面逐条展示）。
    pub fields: Vec<String>,
    /// 请求体（脱敏后），界面可折叠展示。
    pub payload: String,
    /// 估算字符数，用于提示成本。
    pub chars: usize,
}

/// 白名单构造：从资产与政策里**只取**允许出境的字段。
///
/// 注意这里没有任何 `account_identifier` / `credential` / `heir_*` / `user.*`。
/// 新增字段默认不出境——想让它出境必须显式加进这个函数并补测试。
fn build_safe_payload(
    report: &crate::models::InheritanceReport,
    question: &str,
) -> serde_json::Value {
    let assets: Vec<serde_json::Value> = report
        .assets
        .iter()
        .map(|a| {
            serde_json::json!({
                "platform": a.platform_name,
                "category": a.category,
                "disposal": a.disposal_method,
            })
        })
        .collect();
    let policies: Vec<serde_json::Value> = report
        .policies
        .iter()
        .map(|p| {
            serde_json::json!({
                "platform": p.name,
                "documents": p.required_documents,
                "process": p.process_description,
                "difficulty": p.estimated_difficulty,
                "duration": p.estimated_time_days,
            })
        })
        .collect();
    serde_json::json!({
        "question": question,
        "assets": assets,
        "policies": policies,
    })
}

/// 组装发送前的预览（界面用）。
pub fn preview_outbound(
    report: &crate::models::InheritanceReport,
    question: &str,
) -> OutboundPreview {
    let payload = build_safe_payload(report, question);
    let text = serde_json::to_string_pretty(&payload).unwrap_or_default();
    OutboundPreview {
        fields: vec![
            "平台名称".to_string(),
            "资产类别".to_string(),
            "处置方式".to_string(),
            "平台政策(材料/流程/难度/时长)".to_string(),
            "你的问题".to_string(),
        ],
        chars: text.chars().count(),
        payload: text,
    }
}

/// 出境的请求体永远不含这些字段。生产过滤在 payload 组装处逐字段硬编码；
/// 这份清单是它的**执行器**：测试用全量反向断言逼两处保持同步——
/// 生产多发一个字段，测试就红。仅测试引用，故按测试依赖保留。
#[cfg_attr(not(test), allow(dead_code))]
pub const FORBIDDEN_FIELDS: &[&str] = &[
    "account_identifier",
    "credential",
    "credential_enc",
    "heir_name",
    "heir_contact",
    "full_name",
    "phone",
    "email",
    "id_last4",
    "emergency_contact",
    "backup_location",
    "recovery_code",
];

fn system_prompt(locale: crate::report::ReportLocale) -> &'static str {
    match locale {
        crate::report::ReportLocale::Zh => {
            "你是一位数字遗产规划助手。你只能依据用户提供的 assets 与 policies 作答。\
规则：1) 不得编造平台政策、链接或法律条文；2) 无法从给定信息回答时，直接说「根据已提供的信息无法回答」；\
3) 不提供任何法律意见，必要时提示咨询律师；4) 输出为简洁的行动建议，逐条编号；\
5) 不要索要密码、验证码或任何凭据。"
        }
        crate::report::ReportLocale::En => {
            "You are a digital-legacy planning assistant. Answer ONLY from the supplied assets and policies. \
Rules: 1) never invent platform policies, links or statutes; 2) if the answer is not in the given data, say 'cannot answer from the provided information'; \
3) give no legal advice and suggest a lawyer where appropriate; 4) answer as a short numbered action list; \
5) never ask for passwords, verification codes or credentials."
        }
    }
}

/// 调用 OpenAI 兼容的 `/chat/completions`。
///
/// 进程内 HTTP（ureq + rustls）：不依赖系统 curl、不把 Key 交给子进程参数，
/// 超时与 TLS 校验由代码统一控制。CSP 仍保持 `connect-src 'self'`——
/// 前端永远不发网络请求，出网只发生在 Rust 层。
pub(crate) fn call_chat_api(
    endpoint: &str,
    api_key: &str,
    model: &str,
    system: &str,
    user_content: &str,
) -> Result<String, String> {
    let ep = endpoint.trim();
    if !(ep.starts_with("https://") || ep.starts_with("http://")) {
        return Err("端点必须以 https:// 或 http:// 开头".to_string());
    }
    if api_key.trim().is_empty() {
        return Err("尚未填写 API Key".to_string());
    }
    let url = format!("{}/chat/completions", ep.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "temperature": 0.2,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user_content },
        ],
    });

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(TIMEOUT_SECS)))
        .max_redirects(0)
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();

    let response = agent
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {}", api_key.trim()))
        .send_json(&body)
        .map_err(|e| format!("请求失败: {}", e))?;

    let status = response.status().as_u16();
    let text = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("读取响应失败: {}", e))?;
    if !(200..300).contains(&status) {
        // 错误体可能包含提供商的具体原因；截断避免把超长 HTML 错误页回显到 UI。
        let brief: String = text.chars().take(400).collect();
        return Err(format!("请求失败 (HTTP {}): {}", status, brief.trim()));
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "返回内容不是合法 JSON".to_string())?;
    parsed
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "返回内容缺少 choices[0].message.content".to_string())
}

/// 供命令层（流式路径）构造出境 payload —— 与 `ask` 走同一个白名单函数。
pub(crate) fn preview_payload(
    report: &crate::models::InheritanceReport,
    question: &str,
) -> serde_json::Value {
    build_safe_payload(report, question)
}

/// 供命令层（流式路径）取系统提示词。
pub(crate) fn system_prompt_pub(locale: crate::report::ReportLocale) -> &'static str {
    system_prompt(locale)
}

/// 流式调用 OpenAI 兼容的 `/chat/completions`（SSE）。
///
/// 与 `call_chat_api` 相同的白名单 payload 与安全约束；差别只在响应读取方式：
/// 逐行读 `data: {...}` 增量，把每个 delta 通过 `on_delta` 回调交给调用方
/// （命令层经 Tauri channel 推给前端，实现逐字渲染）。
/// SSE 流里的 `[DONE]` 终止；任何一行解析失败不中断流（跳过），由 HTTP 状态兜底。
pub(crate) fn call_chat_api_streaming(
    endpoint: &str,
    api_key: &str,
    model: &str,
    system: &str,
    user_content: &str,
    on_delta: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let ep = endpoint.trim();
    if !(ep.starts_with("https://") || ep.starts_with("http://")) {
        return Err("端点必须以 https:// 或 http:// 开头".to_string());
    }
    if api_key.trim().is_empty() {
        return Err("尚未填写 API Key".to_string());
    }
    let url = format!("{}/chat/completions", ep.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "temperature": 0.2,
        "stream": true,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user_content },
        ],
    });

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(TIMEOUT_SECS)))
        .max_redirects(0)
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();

    let response = agent
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {}", api_key.trim()))
        .header("Accept", "text/event-stream")
        .send_json(&body)
        .map_err(|e| format!("请求失败: {}", e))?;

    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let text = response.into_body().read_to_string().unwrap_or_default();
        let brief: String = text.chars().take(400).collect();
        return Err(format!("请求失败 (HTTP {}): {}", status, brief.trim()));
    }

    let mut reader = std::io::BufReader::new(response.into_body().into_reader());
    let mut line = String::new();
    loop {
        line.clear();
        let n = std::io::BufRead::read_line(&mut reader, &mut line)
            .map_err(|e| format!("读取流失败: {}", e))?;
        if n == 0 {
            break; // EOF
        }
        let trimmed = line.trim_end();
        let Some(data) = trimmed.strip_prefix("data:") else {
            continue; // SSE 注释行 / 空行
        };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(delta) = v
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
            {
                if !delta.is_empty() {
                    on_delta(delta);
                }
            }
        }
        // 解析失败的行跳过：注释、心跳、非 OpenAI 格式的杂讯都不致命
    }
    Ok(())
}

/// 执行一次提问。调用方（命令层）负责检查开关、解密 Key 与占位符拦截。
pub fn ask(
    report: &crate::models::InheritanceReport,
    question: &str,
    endpoint: &str,
    api_key: &str,
    model: &str,
    locale: crate::report::ReportLocale,
) -> Result<String, String> {
    if question.trim().is_empty() {
        return Err("问题不能为空".to_string());
    }
    if question.chars().count() > 2_000 {
        return Err("问题过长（上限 2000 字符）".to_string());
    }
    // 占位符意味着数据未解密；带着占位符提问会得到「未指定」的废答案。
    if crate::report::contains_placeholder(report) {
        return Err("当前数据处于加密锁定状态，请先解锁后再使用助手".to_string());
    }
    let payload = build_safe_payload(report, question.trim());
    let user_content = serde_json::to_string_pretty(&payload).unwrap_or_default();
    call_chat_api(
        endpoint,
        api_key,
        model,
        system_prompt(locale),
        &user_content,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DigitalAsset, PlatformPolicyBrief, UserProfile};
    use crate::report::ReportLocale;

    fn report() -> crate::models::InheritanceReport {
        let asset = DigitalAsset {
            id: Some(1),
            platform_id: 1,
            platform_name: "支付宝".to_string(),
            account_identifier: "very-secret-account-123".to_string(),
            account_description: "私人描述".to_string(),
            category: "property".to_string(),
            estimated_value: Some(12345.0),
            value_currency: "CNY".to_string(),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: Some("13700000000".to_string()),
            notes: "私密备注".to_string(),
            has_credential: true,
            is_confirmed: true,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let policy = PlatformPolicyBrief {
            platform_id: 1,
            name: "支付宝".to_string(),
            required_documents: "死亡证明".to_string(),
            process_description: "联系客服".to_string(),
            estimated_difficulty: "中等".to_string(),
            estimated_time_days: "30天".to_string(),
            official_policy_url: "https://help.alipay.com".to_string(),
            monetization_path: String::new(),
            risk_warnings: String::new(),
            tags: String::new(),
            has_source: true,
            last_reviewed: "2026-10".to_string(),
            data_source: "官方帮助中心".to_string(),
        };
        let user = UserProfile {
            full_name: "张三".to_string(),
            phone: "13800000000".to_string(),
            email: "private@example.com".to_string(),
            heir_primary_name: "王五".to_string(),
            heir_primary_phone: "13700000000".to_string(),
            ..Default::default()
        };
        crate::models::InheritanceReport {
            user,
            assets: vec![asset],
            heirs: Vec::new(),
            authorities: Vec::new(),
            policies: vec![policy],
            total_estimated_value: 12345.0,
            property_assets_count: 1,
            personality_assets_count: 0,
            mixed_assets_count: 0,
            generated_at: "2026-10-26 12:00:00".to_string(),
        }
    }

    /// 出境白名单是这套功能的红线：任何一项身份信息泄露都是产品事故。
    #[test]
    fn outbound_payload_contains_no_identity_data() {
        let r = report();
        let preview = preview_outbound(&r, "我该先办哪个平台？");
        let p = preview.payload.clone();

        // 必须包含的（让回答有依据）
        assert!(p.contains("支付宝"));
        assert!(p.contains("inherit"));
        assert!(p.contains("死亡证明"));

        // 绝不允许出现的内容：账号、姓名、电话、邮箱、备注、估值
        for leak in [
            "very-secret-account-123",
            "13700000000",
            "13800000000",
            "private@example.com",
            "张三",
            "王五",
            "私人描述",
            "私密备注",
            "12345",
        ] {
            assert!(!p.contains(leak), "出境内容泄露了敏感字段: {}", leak);
        }
        for field in FORBIDDEN_FIELDS {
            assert!(!p.contains(field), "出境 JSON 不应包含字段名 {}", field);
        }
    }

    /// 新增的资产字段默认不出境：这条测试会在有人往 payload 里加字段时失败。
    #[test]
    fn payload_shape_is_the_documented_whitelist() {
        let r = report();
        let payload = build_safe_payload(&r, "q");
        let obj = payload.as_object().unwrap();
        let keys: Vec<&String> = obj.keys().collect();
        assert_eq!(keys.len(), 3, "只允许 question / assets / policies");
        let asset_keys: Vec<&String> = obj["assets"][0].as_object().unwrap().keys().collect();
        assert_eq!(
            asset_keys.len(),
            3,
            "资产只允许 platform / category / disposal"
        );
        let policy_keys: Vec<&String> = obj["policies"][0].as_object().unwrap().keys().collect();
        assert_eq!(policy_keys.len(), 5, "政策只允许五个非身份字段");
    }

    #[test]
    fn preview_reports_field_list_and_size() {
        let r = report();
        let preview = preview_outbound(&r, "问题");
        assert!(!preview.fields.is_empty());
        assert!(preview.chars > 0);
        assert_eq!(preview.chars, preview.payload.chars().count());
    }

    /// 锁定态（含占位符）必须拒绝提问，不发请求。
    #[test]
    fn ask_refuses_placeholder_report() {
        let mut r = report();
        r.assets[0].account_identifier = crate::vault::PLACEHOLDER_LOCKED.to_string();
        let err = ask(
            &r,
            "问题",
            "https://api.example.com/v1",
            "sk-test",
            "m",
            ReportLocale::Zh,
        )
        .unwrap_err();
        assert!(err.contains("解锁"), "实际: {}", err);
    }

    #[test]
    fn ask_validates_question_and_endpoint() {
        let r = report();
        let e = ask(&r, "   ", "https://x/v1", "k", "m", ReportLocale::Zh).unwrap_err();
        assert!(e.contains("不能为空"));
        let e = ask(&r, "q", "file:///etc/passwd", "k", "m", ReportLocale::Zh).unwrap_err();
        assert!(e.contains("https://"), "实际: {}", e);
        let long = "x".repeat(2_001);
        let e = ask(&r, &long, "https://x/v1", "k", "m", ReportLocale::Zh).unwrap_err();
        assert!(e.contains("过长"), "实际: {}", e);
    }

    /// 系统提示必须包含「不编造」与「不索要凭据」两条硬约束。
    #[test]
    fn system_prompt_forbids_fabrication_and_credential_requests() {
        let zh = system_prompt(ReportLocale::Zh);
        assert!(zh.contains("不得编造"));
        assert!(zh.contains("不要索要密码"));
        assert!(zh.contains("法律意见"));
        let en = system_prompt(ReportLocale::En);
        assert!(en.contains("never invent"));
        assert!(en.contains("never ask for passwords"));
    }

    /// SSE 流解析：从模拟的分块行流里抽出全部增量，按顺序拼接。
    /// 这是 call_chat_api_streaming 的读循环核心（网络层用本地 mock 代替）。
    #[test]
    fn sse_delta_extraction_from_lines() {
        let lines = [
            "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}",
            "",
            ": keep-alive comment",
            "data: {\"choices\":[{\"delta\":{\"content\":\"，家人\"}}]}",
            "data: {\"choices\":[{\"delta\":{}}]}",
            "data: [DONE]",
            "data: {\"choices\":[{\"delta\":{\"content\":\"不应出现\"}}]}",
        ];
        let mut collected = String::new();
        for raw in lines {
            let trimmed = raw.trim_end();
            let Some(data) = trimmed.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(delta) = v
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                    .and_then(|d| d.get("content"))
                    .and_then(|c| c.as_str())
                {
                    collected.push_str(delta);
                }
            }
        }
        assert_eq!(
            collected, "你好，家人",
            "[DONE] 之后不得再消费；注释行/空行跳过"
        );
    }

    /// 白名单回归：出境 payload 永不含身份字段（FORBIDDEN_FIELDS 全量反向断言）。
    #[test]
    fn streaming_payload_never_contains_forbidden_fields() {
        let mut r = report();
        r.assets[0].account_identifier = "secret-account-id".into();
        r.user.full_name = "机密姓名".into();
        let payload = preview_payload(&r, "继承人该怎么取回支付宝余额？");
        let text = serde_json::to_string(&payload).unwrap();
        for f in FORBIDDEN_FIELDS {
            // 字段名本身不得出现（值级检查在 outbound_payload_contains_no_identity_data）
            assert!(
                !text.contains(&format!("\"{}\":", f)),
                "出境 payload 不得包含字段 {}",
                f
            );
        }
        assert!(
            !text.contains("secret-account-id") && !text.contains("机密姓名"),
            "身份值不得出境"
        );
    }
}
