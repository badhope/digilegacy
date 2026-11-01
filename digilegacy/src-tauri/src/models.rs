//! 领域模型：枚举定义、DTO、统一响应封装

use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────
// 领域枚举：用于校验前端传入的合法值
// ──────────────────────────────────────────────

/// 数字资产类别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AssetCategory {
    Property,    // 财产型
    Personality, // 人格型
    Mixed,       // 混合型
}

impl AssetCategory {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "property" => Some(Self::Property),
            "personality" => Some(Self::Personality),
            "mixed" => Some(Self::Mixed),
            _ => None,
        }
    }
}

/// 处置方式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DisposalMethod {
    Inherit,
    Liquidate,
    Memorialize,
    AuthorizeView,
    Delete,
    Donate,
    NoAction,
}

impl DisposalMethod {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "inherit" => Some(Self::Inherit),
            "liquidate" => Some(Self::Liquidate),
            "memorialize" => Some(Self::Memorialize),
            "authorize_view" => Some(Self::AuthorizeView),
            "delete" => Some(Self::Delete),
            "donate" => Some(Self::Donate),
            "no_action" => Some(Self::NoAction),
            _ => None,
        }
    }
}

/// 校验并规范化资产类别
pub fn validate_category(cat: &str) -> Result<String, String> {
    match AssetCategory::from_str(cat) {
        Some(_) => Ok(cat.to_string()),
        None => Err(format!("无效的资产类别: {}", cat)),
    }
}

/// 校验并规范化处置方式
pub fn validate_disposal(method: &str) -> Result<String, String> {
    match DisposalMethod::from_str(method) {
        Some(_) => Ok(method.to_string()),
        None => Err(format!("无效的处置方式: {}", method)),
    }
}

/// 处置意愿的完整性校验：选「继承」必须写明由谁继承。
///
/// 只选了「指定继承」却没填继承人，报告里就会出现一条「继承人：未指定」，
/// 家人拿到后无从执行。前端负责即时提示，后端负责兜底——
/// 前端校验可以被绕过（IPC 直调），这条必须在写库前执行。
pub fn validate_disposal_heir(method: &str, heir_name: Option<&str>) -> Result<(), String> {
    if method == "inherit" {
        let name = heir_name.unwrap_or("").trim();
        if name.is_empty() {
            return Err("处置方式为「指定继承」时必须填写继承人姓名".to_string());
        }
    }
    Ok(())
}

// ──────────────────────────────────────────────
// 数据传输对象（DTO）
// ──────────────────────────────────────────────

/// 平台信息（基础版，用于数据库存储与常规查询）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Platform {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub subtype: String,
    pub has_property: bool,
    pub has_personality: bool,
    pub inheritable: bool,
    pub required_documents: String,
    pub process_description: String,
    pub estimated_difficulty: String,
    pub official_policy_url: String,
    pub estimated_time_days: String,
    pub notes: String,
}

/// 平台扩展信息（包含继承执行工具包所需字段）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformExtended {
    #[serde(flatten)]
    pub base: Platform,
    pub name_en: String,
    pub data_source: String,
    pub inheritance_form_url: String,
    pub monetization_path: String,
    pub legal_template_ids: String,
    pub risk_warnings: String,
    pub tags: String,
    /// 最后人工核对月份（`YYYY-MM`），空 = 从未核对。
    pub last_reviewed: String,
}

/// 数字资产
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DigitalAsset {
    pub id: Option<i64>,
    pub platform_id: i64,
    pub platform_name: String,
    pub account_identifier: String,
    pub account_description: String,
    pub category: String,
    pub estimated_value: Option<f64>,
    pub value_currency: String,
    pub disposal_method: String,
    pub heir_name: Option<String>,
    pub heir_contact: Option<String>,
    pub notes: String,
    pub has_credential: bool,
    pub is_confirmed: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 安全状态
#[derive(Debug, Clone, Serialize)]
pub struct SecurityStatus {
    pub has_master_password: bool,
    pub unlocked: bool,
}

/// 设置主密码的返回：安全状态 + 一次性恢复密钥。
/// 恢复密钥只在此响应中返回一次，UI 必须立即展示给用户抄录。
#[derive(Debug, Clone, Serialize)]
pub struct MasterPasswordSetup {
    pub status: SecurityStatus,
    pub recovery_code: String,
}

/// 创建/更新资产的请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetRequest {
    pub platform_id: i64,
    pub account_identifier: String,
    pub account_description: String,
    pub category: String,
    pub estimated_value: Option<f64>,
    pub disposal_method: String,
    pub heir_name: Option<String>,
    pub heir_contact: Option<String>,
    pub notes: String,
}

/// 用户档案
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserProfile {
    pub id: Option<i64>,
    pub full_name: String,
    pub phone: String,
    pub email: String,
    pub id_last4: String,
    pub emergency_contact_name: String,
    pub emergency_contact_phone: String,
    pub emergency_contact_relation: String,
    pub emergency_contact_email: String,
    pub heir_primary_name: String,
    pub heir_primary_phone: String,
    pub heir_primary_relation: String,
    pub heir_primary_email: String,
    pub check_in_interval_days: i64,
    pub last_check_in: Option<String>,
    pub dms_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 一位继承人 / 执行人。
///
/// 为什么需要多名：真实家庭很少只有一个人接手。旧模型只有一组
/// `heir_primary_*` 字段，第二个孩子、配偶、律师都没有位置，
/// 报告里也只能写一个名字，家人拿到后无法判断谁负责哪一部分。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Heir {
    pub id: Option<i64>,
    pub name: String,
    pub relation: String,
    pub phone: String,
    pub email: String,
    /// 分配说明（如「房产类资产」「仅处理金融账户」）。自由文本，可空。
    pub share_note: String,
    /// 是否为遗产执行人（负责统筹，DMS 第三级通知优先发给他）。
    pub is_executor: bool,
    pub sort_order: i64,
}

/// 权威来源登记（来自 `src/data/authorities.json`，由 `verify-authorities.mjs` 实测）。
///
/// `reachable` 与 `content_matched` 是两件独立的事实：可达只证明页面存在，
/// 只有内容匹配才证明「这条链接支持这条引用」。界面必须分别展示，
/// 不能合并成一个「已验证」。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Authority {
    pub id: String,
    pub kind: String,
    pub title_zh: String,
    pub title_en: String,
    pub publisher: String,
    pub url: String,
    pub note_zh: String,
    pub note_en: String,
    pub applies_to: Vec<String>,
    #[serde(default)]
    pub reachable: bool,
    #[serde(default)]
    pub content_matched: bool,
    #[serde(default)]
    pub last_checked: String,
    #[serde(default)]
    pub http_status: i64,
    #[serde(default)]
    pub observed_title: String,
    #[serde(default)]
    pub portal_note_zh: String,
    #[serde(default)]
    pub portal_note_en: String,
}

/// 内置权威来源表（编译期嵌入，与前端读同一份 JSON）。
pub fn authorities() -> Vec<Authority> {
    serde_json::from_str(include_str!("../../src/data/authorities.json")).unwrap_or_default()
}

/// 报告用平台政策摘要。
///
/// 为什么单独建结构：导出物必须与界面预览同源。旧实现的导出只有平台名与
/// 处置方式，把政策、所需材料、难度、官方入口全丢了，家人拿到的是一份
/// 「知道有这件事、但不知道怎么办」的清单。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformPolicyBrief {
    pub platform_id: i64,
    pub name: String,
    pub required_documents: String,
    pub process_description: String,
    pub estimated_difficulty: String,
    pub estimated_time_days: String,
    pub official_policy_url: String,
    pub monetization_path: String,
    pub risk_warnings: String,
    pub tags: String,
    /// 是否带来源标注（`platform_data.rs` 的 `data_source` 非空）。
    /// 这不等于「已核验官方原文」——那需要人工逐条比对；界面与报告都按此措辞。
    pub has_source: bool,
    /// 最后人工核对月份（`YYYY-MM`）；空 = 从未核对。
    pub last_reviewed: String,
    /// 真实来源标注（`未核实…` 表示未核实）。界面据此显示溯源状态。
    pub data_source: String,
}

/// 继承报告
#[derive(Debug, Serialize, Default)]
pub struct InheritanceReport {
    pub user: UserProfile,
    pub assets: Vec<DigitalAsset>,
    /// 继承人名单（含执行人标记）。空时回退到 `user.heir_primary_*`。
    pub heirs: Vec<Heir>,
    pub policies: Vec<PlatformPolicyBrief>,
    /// 供家人与审核员查阅的权威来源（只包含与本次资产类别相关的条目）。
    pub authorities: Vec<Authority>,
    pub total_estimated_value: f64,
    pub property_assets_count: i64,
    pub personality_assets_count: i64,
    pub mixed_assets_count: i64,
    pub generated_at: String,
}

/// 仪表板统计
///
/// `completion_percentage` 是四项检查各占 25% 的结果，而不是「处置方式不是待设定」。
/// 旧口径下「选了继承但没填继承人」也算 100%，是典型的自欺指标。
#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub total_assets: i64,
    pub property_count: i64,
    pub personality_count: i64,
    pub mixed_count: i64,
    /// 已设定处置方式（`disposal_method != 'no_action'`）的资产数。
    pub disposal_planned_count: i64,
    /// 处置方式是「继承」且填了继承人姓名的资产数（其他处置视为已满足）。
    pub heir_assigned_count: i64,
    /// 账号标识已填（非空且不是解密占位符）的资产数。
    pub identifier_ready_count: i64,
    /// 触发链是否可用：DMS 已启用，且存在至少一条真正能送达的通道。
    pub trigger_ready: bool,
    /// 当前可用的投递通道描述（`smtp` / `heartbeat` / `none`），供界面如实展示。
    pub trigger_channel: String,
    pub total_estimated_value: f64,
    pub last_check_in: Option<String>,
    pub dms_enabled: bool,
    /// 四项检查各 25% 的加权结果。
    pub completion_percentage: i64,
    /// 待办检查项：每条都是一句可直接执行的动作，不是泛泛的「建议完善」。
    /// 由数据推导，不写死文案——同一个界面语言下，中英文各自渲染。
    pub issues: Vec<CheckIssue>,
}

/// 一条待办检查项。
///
/// 设计理由：完成度百分比只告诉用户「还差多少」，不告诉「差在哪」。
/// 用户看到 75% 无从下手；看到「3 条资产没有继承人」才知道下一步做什么。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckIssue {
    /// 稳定键，前端据此映射文案与跳转目标。
    pub key: String,
    /// 严重度：blocker 表示会导致家人无法执行；warn 表示质量缺陷；info 表示优化建议。
    pub severity: String,
    /// 受影响条数（无条数概念时为 0）。
    pub count: i64,
    /// 建议跳转的页面路径（前端可直接 router.push）。
    pub target: String,
}

// ──────────────────────────────────────────────
// API 响应统一封装
// ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            message: "操作成功".into(),
            data: Some(data),
        }
    }

    pub fn ok_with_msg(data: T, msg: &str) -> Self {
        Self {
            success: true,
            message: msg.into(),
            data: Some(data),
        }
    }

    pub fn fail(msg: &str) -> Self {
        Self {
            success: false,
            message: msg.into(),
            data: None,
        }
    }
}
