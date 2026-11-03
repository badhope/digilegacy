// 前端共享类型定义（与后端 Rust model 对齐）

export interface Platform {
  id: number;
  name: string;
  category: string;
  subtype: string;
  has_property: boolean;
  has_personality: boolean;
  inheritable: boolean;
  required_documents: string;
  process_description: string;
  estimated_difficulty: string;
  official_policy_url: string;
  estimated_time_days: string;
  notes: string;
}

export interface PlatformExtended extends Platform {
  name_en: string;
  data_source: string;
  inheritance_form_url: string;
  monetization_path: string;
  legal_template_ids: string;
  risk_warnings: string;
  tags: string;
  /** 最后人工核对月份（YYYY-MM）；空 = 从未核对 */
  last_reviewed: string;
}

export interface DigitalAsset {
  id?: number;
  platform_id: number;
  platform_name: string;
  account_identifier: string;
  account_description: string;
  category: 'property' | 'personality' | 'mixed';
  estimated_value: number | null;
  value_currency: string;
  disposal_method: string;
  heir_name: string | null;
  heir_contact: string | null;
  notes: string;
  has_credential: boolean;
  is_confirmed: boolean;
  created_at: string;
  updated_at: string;
}

export interface SecurityStatus {
  has_master_password: boolean;
  unlocked: boolean;
}

export interface AssetRequest {
  platform_id: number;
  account_identifier: string;
  account_description: string;
  category: string;
  estimated_value: number | null;
  disposal_method: string;
  heir_name?: string;
  heir_contact?: string;
  notes?: string;
}

export interface UserProfile {
  id?: number;
  full_name: string;
  phone: string;
  email: string;
  id_last4: string;
  emergency_contact_name: string;
  emergency_contact_phone: string;
  emergency_contact_relation: string;
  emergency_contact_email: string;
  heir_primary_name: string;
  heir_primary_phone: string;
  heir_primary_relation: string;
  heir_primary_email: string;
  check_in_interval_days: number;
  last_check_in: string | null;
  dms_enabled: boolean;
  created_at: string;
  updated_at: string;
}

/** 报告里的平台政策摘要（后端 `get_policy_briefs_db` 产出，导出与预览同源）。 */
export interface PlatformPolicyBrief {
  platform_id: number;
  name: string;
  required_documents: string;
  process_description: string;
  estimated_difficulty: string;
  estimated_time_days: string;
  official_policy_url: string;
  monetization_path: string;
  risk_warnings: string;
  tags: string;
  has_source: boolean;
}

/** 权威来源（与 src/data/authorities.json 对应，由脚本实测）。 */
export interface Authority {
  id: string;
  kind: string;
  title_zh: string;
  title_en: string;
  publisher: string;
  url: string;
  note_zh: string;
  note_en: string;
  applies_to: string[];
  reachable: boolean;
  content_matched: boolean;
  last_checked: string;
  http_status: number;
  observed_title: string;
  portal_note_zh: string;
  portal_note_en: string;
}

/** 通用审核场景声明（后端注册表下发，界面不写死业务名）。 */
export interface ReviewProfile {
  id: string;
  title_key: string;
  criteria: string;
  use_ai: boolean;
  /** standard | strict：严格模式要求逐项作答，疑点需人工说明理由才能放行 */
  strictness: 'standard' | 'strict';
  /** 严格模式的必检项（界面展示「AI 必须逐项回答哪些问题」） */
  strict_checks: Array<{ id: string; label: string }>;
}

/** 条目当前审核状态。approved 只能由人工终审产生。 */
export type ReviewStatus = 'pending' | 'awaiting_human' | 'approved' | 'rejected' | 'needs_edit';
export type AiVerdict = 'pass' | 'flag' | 'unknown' | 'skipped';
export type HumanVerdict = 'approved' | 'rejected' | 'needs_edit';

export interface ReviewItemState {
  key: string;
  title: string;
  /** 待审内容摘要（审核员据此判断要不要细看） */
  preview: string;
  status: ReviewStatus;
  ai_verdict: AiVerdict | null;
  ai_note: string;
  human_note: string;
  reviewed_at: string | null;
  stale: boolean;
}

export interface ReviewRecord {
  profile_id: string;
  item_key: string;
  fingerprint: string;
  ai_verdict: AiVerdict;
  ai_note: string;
  human_verdict: HumanVerdict | null;
  human_note: string;
  decided_by: string;
  created_at: string;
}

/** 一位继承人 / 执行人。 */
export interface Heir {
  id?: number;
  name: string;
  relation: string;
  phone: string;
  email: string;
  share_note: string;
  is_executor: boolean;
  sort_order: number;
}

/** AI 助手配置。默认关闭；不内置任何端点，用户自填。 */
export interface AssistantConfig {
  enabled: boolean;
  endpoint: string;
  model: string;
  has_key: boolean;
}

/** 出境预览：提问前逐项展示将发送的非身份字段。 */
export interface OutboundPreview {
  fields: string[];
  payload: string;
  chars: number;
}

/** 保管位置：备份文件与恢复码放在哪里。 */
export interface CustodyHints {
  backup_location: string;
  recovery_code_location: string;
}

/** 家人演练结果。 */
export interface DrillCheck {
  key: string;
  ok: boolean;
  count: number;
}

export interface DrillReport {
  platforms: number;
  assets: number;
  assets_without_heir: number;
  assets_without_policy: number;
  unverified_platforms: number;
  trigger_ready: boolean;
  trigger_channel: string;
  checks: DrillCheck[];
}

export interface DashboardStats {
  total_assets: number;
  property_count: number;
  personality_count: number;
  mixed_count: number;
  /** 已设定处置方式的资产数 */
  disposal_planned_count: number;
  /** 处置为「继承」且已填继承人的资产数（其他处置视为满足） */
  heir_assigned_count: number;
  /** 账号标识已填且不是解密占位符的资产数 */
  identifier_ready_count: number;
  /** 触发链是否可用：DMS 启用且至少有一条可送达通道 */
  trigger_ready: boolean;
  /** 实际可用通道：smtp / heartbeat / none */
  trigger_channel: 'smtp' | 'heartbeat' | 'none';
  total_estimated_value: number;
  last_check_in: string | null;
  dms_enabled: boolean;
  /** 四项检查各 25% 的加权结果 */
  completion_percentage: number;
  /** 待办检查项：每条都带跳转目标，点一下就去做 */
  issues: CheckIssue[];
}

/** 一条待办检查项。severity: blocker 会导致家人无法执行；warn 质量缺陷；info 优化建议。 */
export interface CheckIssue {
  key: string;
  severity: 'blocker' | 'warn' | 'info';
  count: number;
  target: string;
}

export interface ApiResponse<T> {
  success: boolean;
  message: string;
  data: T | null;
}

export interface CheckInStatus {
  last_check_in: string | null;
  interval_days: number;
  dms_enabled: boolean;
  next_check_in_due: string | null;
  days_until_due: number | null;
  /** 最近 5 次确认时间（新的在前） */
  recent_check_ins?: string[];
}

export interface DmsNotification {
  stage: number;
  recipient: string;
  subject: string;
  body: string;
  sent_at: string;
  channel: string;
}

export interface DmsEvaluation {
  enabled: boolean;
  overdue: boolean;
  current_stage: number;
  days_overdue: number;
  notifications: DmsNotification[];
}

/** 只读 DMS 状态：界面渲染用，不触发投递。 */
export interface DmsStatus {
  enabled: boolean;
  overdue: boolean;
  current_stage: number;
  days_overdue: number;
}


