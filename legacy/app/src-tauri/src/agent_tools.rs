//! 工具注册表与权限分级（harness 层的核心抽象）。
//!
//! 为什么需要它：原来的 agent 工具是「`tool_definitions()` 里一份 JSON、
//! `execute_tool()` 里一个 match」，两边靠人肉保持一致，加一个工具要改两处，
//! 权限边界（只读 / 草稿 / 写入）只写在注释里，无法被程序检查。
//!
//! 这里把「工具是什么」变成数据：
//! - 定义、执行、权限三者绑定在同一条 `ToolSpec` 上，不可能漂移；
//! - 权限是枚举而不是注释，执行入口按权限统一拦截；
//! - 注册表可在测试里枚举，能断言「所有工具都有权限标注」「没有工具能写库」。
//!
//! 边界不变：本模块**不新增任何写路径**。`ToolPermission::Draft` 只产出文本，
//! `Read` 只读库。将来的写工具必须新增权限档位并单独走人工确认，不能复用现有档位。

use serde::Serialize;

use crate::db;

/// 工具权限档位。执行前按此统一拦截，不依赖每个工具自己记得检查。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ToolPermission {
    /// 只读本地数据（资产、统计、政策库）。结果会出境给模型。
    Read,
    /// 只产出文本草稿，不落库。用户在界面上确认后才可能被保存。
    Draft,
}

impl ToolPermission {
    /// 权限的人类可读名（前端展示与审计日志共用）。
    pub fn label(&self) -> &'static str {
        match self {
            ToolPermission::Read => "read",
            ToolPermission::Draft => "draft",
        }
    }

    /// 是否允许该权限的工具在当前 harness 配置下运行。
    ///
    /// 目前两档都允许；保留这个函数是为了让「未来收紧权限」只需改一处，
    /// 而不是回去翻每个工具的执行分支。
    pub fn is_allowed(&self) -> bool {
        matches!(self, ToolPermission::Read | ToolPermission::Draft)
    }
}

/// 一条工具规格：定义 + 权限 + 执行器，三者不可分离。
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema 形式的入参定义（原样拼进 tools 数组）。
    pub parameters: serde_json::Value,
    pub permission: ToolPermission,
    /// 执行器：拿到已解析的参数，返回要回传给模型的文本。
    pub run: fn(&serde_json::Value) -> Result<String, String>,
}

/// 全部已注册工具。新增工具只改这一处，`tool_definitions` 与 `execute` 自动跟随。
pub fn registry() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "query_assets",
            description: "查询用户已登记的数字资产列表（平台名/类别/处置方式/继承人；不含凭据与密码）",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "category": { "type": "string", "description": "可选过滤：property/personality/mixed" }
                }
            }),
            permission: ToolPermission::Read,
            run: run_query_assets,
        },
        ToolSpec {
            name: "query_platform_policy",
            description: "按平台名或关键词查内置继承政策库（所需材料/办理流程/难度/时长/官方链接/来源标注）",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "keyword": { "type": "string", "description": "平台名关键词，如 支付宝" }
                }
            }),
            permission: ToolPermission::Read,
            run: run_query_platform_policy,
        },
        ToolSpec {
            name: "get_planning_stats",
            description: "获取继承规划完成度统计（资产数/处置进度/触发链状态/待办清单）",
            parameters: serde_json::json!({ "type": "object", "properties": {} }),
            permission: ToolPermission::Read,
            run: run_get_planning_stats,
        },
        ToolSpec {
            name: "run_readiness_drill",
            description: "跑一次家人演练（只读元数据）：家人拿到执行包会在哪一步卡住",
            parameters: serde_json::json!({ "type": "object", "properties": {} }),
            permission: ToolPermission::Read,
            run: run_readiness_drill,
        },
        ToolSpec {
            name: "get_trigger_status",
            description: "查询到期提醒（安心传承）当前状态：是否启用、逾期阶段、最近确认时间",
            parameters: serde_json::json!({ "type": "object", "properties": {} }),
            permission: ToolPermission::Read,
            run: run_get_trigger_status,
        },
        ToolSpec {
            name: "draft_heir_note",
            description: "起草一段给继承人的说明文字草稿（只返回草稿文本；由用户确认后自行保存，agent 不会写入任何数据）",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "topic": { "type": "string" },
                    "points": { "type": "string", "description": "要点，分号分隔" }
                },
                "required": ["topic"]
            }),
            permission: ToolPermission::Draft,
            run: run_draft_heir_note,
        },
    ]
}

/// 组装发给模型的 `tools` 数组（OpenAI function-calling 格式）。
pub fn tool_definitions() -> serde_json::Value {
    let tools: Vec<serde_json::Value> = registry()
        .iter()
        .map(|spec| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": spec.name,
                    "description": spec.description,
                    "parameters": spec.parameters,
                }
            })
        })
        .collect();
    serde_json::Value::Array(tools)
}

/// 按名字执行工具。未知工具返回显式错误，让模型自己纠正。
pub fn execute(name: &str, args: &serde_json::Value) -> Result<String, String> {
    let specs = registry();
    let spec = specs
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("未知工具：{}（可用：{}）", name, tool_names().join(", ")))?;

    // 权限统一在这里拦，而不是靠每个执行器自觉。
    if !spec.permission.is_allowed() {
        return Err(format!(
            "工具 {} 的权限档位 {} 当前不允许运行",
            name,
            spec.permission.label()
        ));
    }

    (spec.run)(args)
}

/// 全部工具名（测试与错误提示共用）。
pub fn tool_names() -> Vec<&'static str> {
    registry().iter().map(|s| s.name).collect()
}

// ─────────────────────────── 各工具实现 ───────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{}…（截断）", cut)
    }
}

const TOOL_RESULT_MAX_CHARS: usize = 6_000;

fn run_query_assets(args: &serde_json::Value) -> Result<String, String> {
    let category = args.get("category").and_then(|x| x.as_str()).unwrap_or("");
    let assets = db::get_all_assets_db()?;
    let lines: Vec<String> = assets
        .iter()
        .filter(|a| category.is_empty() || a.category == category)
        .map(|a| {
            // Agent 只需要判断规划状态；账号标识、继承人姓名/联系方式、备注与凭据
            // 都不出境，避免「本地只读工具」变成身份数据外送工具。
            format!(
                "- 平台={}（类别={}）：处置={}，继承人状态={}",
                a.platform_name,
                a.category,
                a.disposal_method,
                if a.heir_name
                    .as_deref()
                    .map(|v| !v.trim().is_empty() && !v.contains("需解锁"))
                    .unwrap_or(false)
                {
                    "已指定"
                } else {
                    "未指定/未知"
                }
            )
        })
        .collect();
    if lines.is_empty() {
        Ok("（没有符合条件的资产）".to_string())
    } else {
        Ok(truncate(&lines.join("\n"), TOOL_RESULT_MAX_CHARS))
    }
}

fn run_query_platform_policy(args: &serde_json::Value) -> Result<String, String> {
    let kw = args
        .get("keyword")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    if kw.is_empty() {
        return Err("keyword 不能为空".to_string());
    }
    if kw.chars().count() > 100 || kw.chars().any(|c| c.is_control()) {
        return Err("keyword 含非法字符或超过 100 字符".to_string());
    }
    let platforms = crate::platform_data::get_all_platforms();
    let hits: Vec<String> = platforms
        .iter()
        .filter(|p| p.name.contains(kw) || p.name_en.to_lowercase().contains(&kw.to_lowercase()))
        .take(5)
        .map(|p| {
            format!(
                "【{}】可继承：{}\n所需材料：{}\n流程：{}\n难度：{}；时长：{}\n来源：{}",
                p.name,
                if p.inheritable { "是" } else { "否" },
                p.required_documents,
                p.process_description,
                p.estimated_difficulty,
                p.estimated_time_days,
                p.data_source,
            )
        })
        .collect();
    if hits.is_empty() {
        Ok(format!("（内置政策库没有匹配「{}」的平台）", kw))
    } else {
        Ok(truncate(&hits.join("\n\n"), TOOL_RESULT_MAX_CHARS))
    }
}

fn run_get_planning_stats(_args: &serde_json::Value) -> Result<String, String> {
    let s = db::get_dashboard_stats_db()?;
    let todos: Vec<String> = s
        .issues
        .iter()
        .map(|i| format!("[{}] {}（去 {}）", i.severity, i.key, i.target))
        .collect();
    Ok(format!(
        "资产总数：{}\n已设定处置：{}\n已指定继承人：{}\n账号标识就绪：{}\n触发链：{}\n完成度：{}%\n待办：{}",
        s.total_assets,
        s.disposal_planned_count,
        s.heir_assigned_count,
        s.identifier_ready_count,
        s.trigger_channel,
        s.completion_percentage,
        if todos.is_empty() {
            "无".to_string()
        } else {
            todos.join("；")
        }
    ))
}

fn run_readiness_drill(_args: &serde_json::Value) -> Result<String, String> {
    let report = db::get_inheritance_report_db()?;
    let stats = db::get_dashboard_stats_db()?;
    let drill =
        crate::toolkit::evaluate_drill(&report, stats.trigger_ready, &stats.trigger_channel);
    let lines: Vec<String> = drill
        .checks
        .iter()
        .map(|c| {
            format!(
                "{} {}（{} 项）",
                if c.ok { "✓" } else { "✗" },
                c.key,
                c.count
            )
        })
        .collect();
    Ok(format!(
        "家人演练结果（仅元数据，不读凭据）：\n{}",
        lines.join("\n")
    ))
}

fn run_get_trigger_status(_args: &serde_json::Value) -> Result<String, String> {
    let s = db::get_check_in_status_db()?;
    Ok(format!(
        "到期提醒启用：{}\n间隔天数：{}\n最近确认：{}\n距下次到期：{}",
        if s.dms_enabled { "是" } else { "否" },
        s.interval_days,
        s.last_check_in.as_deref().unwrap_or("从未"),
        s.days_until_due
            .map(|d| format!("{} 天", d))
            .unwrap_or_else(|| "未知".to_string())
    ))
}

fn run_draft_heir_note(args: &serde_json::Value) -> Result<String, String> {
    let topic = args
        .get("topic")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    if topic.is_empty() {
        return Err("topic 不能为空".to_string());
    }
    let points = args.get("points").and_then(|x| x.as_str()).unwrap_or("");
    // 只产出骨架，不代写具体承诺：内容必须由持有人确认（可能涉及法律效力）。
    Ok(format!(
        "【草稿·待用户确认】\n主题：{}\n要点：{}\n\n建议结构：\n1. 这批账号是什么、为什么重要\n2. 你希望家人怎么处理（逐项对应处置方式）\n3. 需要先准备的材料与联系人\n4. 遇到困难时可以找谁\n\n注意：本段是 agent 生成的草稿，未经你确认前不会写入任何数据。",
        topic,
        if points.trim().is_empty() { "（未提供）" } else { points }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个工具都必须有权限标注，且当前只允许 read / draft 两档。
    /// 这是「agent 不能写库」这条边界的可执行版本。
    #[test]
    fn every_tool_has_permission_and_none_can_write() {
        let specs = registry();
        assert!(!specs.is_empty());
        for spec in &specs {
            assert!(
                spec.permission.is_allowed(),
                "工具 {} 权限不被允许",
                spec.name
            );
            assert!(
                matches!(
                    spec.permission,
                    ToolPermission::Read | ToolPermission::Draft
                ),
                "工具 {} 声明了未受支持的权限档位",
                spec.name
            );
        }
    }

    /// 定义数组与注册表必须严格一致：历史上两边手写，容易漏。
    #[test]
    fn definitions_match_registry_exactly() {
        let defs = tool_definitions();
        let arr = defs.as_array().expect("tools 必须是数组");
        assert_eq!(arr.len(), registry().len());
        let def_names: Vec<String> = arr
            .iter()
            .filter_map(|d| d.pointer("/function/name").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .collect();
        let reg_names: Vec<String> = tool_names().iter().map(|s| s.to_string()).collect();
        assert_eq!(def_names, reg_names);
    }

    #[test]
    fn unknown_tool_reports_available_names() {
        let err = execute("no_such_tool", &serde_json::json!({})).unwrap_err();
        assert!(err.contains("未知工具"));
        assert!(err.contains("query_assets"), "错误信息应列出可用工具");
    }

    #[test]
    fn draft_tool_marks_output_as_unconfirmed() {
        let out = execute(
            "draft_heir_note",
            &serde_json::json!({"topic": "支付宝余额"}),
        )
        .unwrap();
        assert!(out.contains("待用户确认"));
        assert!(out.contains("不会写入任何数据"));
    }

    #[test]
    fn policy_tool_rejects_control_chars() {
        let err = execute(
            "query_platform_policy",
            &serde_json::json!({"keyword": "支付宝\nrm -rf"}),
        )
        .unwrap_err();
        assert!(err.contains("非法字符"));
    }
}
