export type Locale = "zh-CN" | "en-US";

export interface Envelope<T> {
  success: boolean;
  code: string;
  message: string;
  data: T | null;
  details: Record<string, unknown> | null;
  traceId: string;
}

export type ModuleStatus = "ready" | "planned" | "degraded";

export interface OverviewModule {
  id: string;
  label: string;
  status: ModuleStatus;
}

export interface OverviewActivity {
  at: string;
  title: string;
  kind: string;
}

export interface Overview {
  workspace: string;
  completion: number;
  assetsTotal: number;
  heirsTotal: number;
  approvalsPending: number;
  alerts: number;
  modules: OverviewModule[];
  activity: OverviewActivity[];
}

export type AssetKind =
  | "account"
  | "file"
  | "device"
  | "domain"
  | "subscription"
  | "physical"
  | "financial"
  | "real_estate"
  | "vehicle"
  | "intellectual_property"
  | "business"
  | "virtual_item"
  | "membership"
  | "identity"
  | "other";
export type AssetStatus = "active" | "archived";
export type AssetValueStatus = "market" | "appraised" | "insured" | "unknown";

export interface AssetDeleteResult {
  id: string;
  deleted: boolean;
}

export interface AssetMutationInput {
  id?: string;
  name: string;
  kind: AssetKind;
  platform?: string;
  value?: string;
  valueStatus?: AssetValueStatus;
  note?: string;
  identifier?: string;
  ownershipShare?: string;
  beneficiary?: string;
  beneficiaryContact?: string;
  credentialLocation?: string;
  expiresAt?: string;
  quantity?: string;
  unit?: string;
  status?: AssetStatus;
}

export interface Asset {
  id: string;
  name: string;
  kind: AssetKind;
  platform: string;
  status: AssetStatus;
  value: string;
  valueStatus: AssetValueStatus;
  note: string;
  identifier: string;
  ownershipShare: string;
  beneficiary: string;
  beneficiaryContact: string;
  credentialLocation: string;
  expiresAt: string;
  quantity: string;
  unit: string;
  updatedAt: string;
}

export interface AssetsPage {
  items: Asset[];
  nextCursor: string | null;
  total: number;
}

export interface DiscoveryCategory {
  id: string;
  label: string;
  labelEn: string;
  count: number;
}

export interface DiscoverySuggestion {
  id: string;
  name: string;
  kind: AssetKind;
  platform: string;
  category: string;
  reason: string;
  reasonEn: string;
  exists: boolean;
}

export interface DiscoveryResult {
  categories: DiscoveryCategory[];
  suggestions: DiscoverySuggestion[];
  total: number;
}

export interface BulkCreateResult {
  created: number;
  skipped: number;
  items: Asset[];
}

export interface Heir {
  id: string;
  name: string;
  role: string;
  email: string;
  phone: string;
}

export interface HeirDraft {
  id?: string;
  name: string;
  role?: string;
  email?: string;
  phone?: string;
  isExecutor?: boolean;
}

export interface CheckinResult {
  checkedInAt: string;
  nextCheckAt: string;
  status: string;
}

export interface Trigger {
  mode: string;
  days: number;
  status: "off" | "armed" | "paused" | "triggered" | string;
  lastCheckIn: string;
  nextCheckAt: string;
  pausedUntil: string;
  triggeredAt: string;
  stage: number;
}

export interface TriggerEvent {
  id: string;
  createdAt: string;
  kind: string;
  stage: number;
  detail: string;
}

export interface TriggerEventPage {
  items: TriggerEvent[];
  nextCursor: string | null;
  total: number;
}

export type NotificationStatus = "pending" | "sent" | "failed" | "blocked";

export interface NotificationItem {
  id: string;
  createdAt: string;
  triggerEventId: number;
  kind: string;
  recipient: string;
  recipientRole: string;
  subject: string;
  body: string;
  status: NotificationStatus;
  attempts: number;
  lastError: string;
  sentAt: string;
}

export interface NotificationPage {
  items: NotificationItem[];
  nextCursor: string | null;
  total: number;
}

export interface NotificationSendResult {
  configured: boolean;
  sent: number;
  failed: number;
  pending: number;
  errors: string[];
}

export type ConnectionKind = "llm" | "policy" | "smtp" | "imap" | "heartbeat" | "webhook" | "feishu" | "wecom" | "mcp";
export type ConnectionStatus = "unconfigured" | "ok" | "degraded" | "error" | "disabled";
export type ConnectionSource = "env" | "stored";

export interface ConnectionItem {
  id: string;
  kind: ConnectionKind;
  label: string;
  model?: string;
  enabled: boolean;
  lastCheckAt: string;
  lastStatus: ConnectionStatus;
  lastError: string;
  updatedAt: string;
  source: ConnectionSource;
}

export interface ConnectionPage {
  items: ConnectionItem[];
  total: number;
}

export interface ConnectionTestResult {
  ok: boolean;
  status: ConnectionStatus;
  latencyMs: number;
  message: string;
  connectionId?: string;
  checkedAt?: string;
  models?: string[];
}

export interface ConnectionUpsertInput {
  id?: string;
  kind: ConnectionKind;
  label?: string;
  endpoint: string;
  model?: string;
  token?: string;
  username?: string;
  from?: string;
  security?: "tls" | "starttls" | "none";
  enabled?: boolean;
}

export interface ConnectionDeleteResult {
  id: string;
  kind: string;
  label: string;
}

export interface Plan {
  holder: { name: string; updatedAt: string };
  heirs: Heir[];
  trigger: Trigger;
}

export type RunStatus = "queued" | "running" | "cancelling" | "waiting_approval" | "succeeded" | "failed" | "rejected" | "cancelled";

export interface AgentRun {
  id: string;
  status: RunStatus;
  title: string;
  steps: number;
  startedAt: string;
  durationMs: number;
  tokens: number;
}

export interface RunStep {
  tool: string;
  args: Record<string, unknown>;
  status: "queued" | "pending_approval" | "done" | "failed" | "rejected" | "cancelled";
  result?: unknown;
  error: string;
  at: string;
}

export interface RunDetail {
  id: string;
  status: RunStatus;
  task: string;
  steps: RunStep[];
  toolsUsed: string;
  rounds: number;
  startedAt: string;
  durationMs: number;
  finalText: string;
  tokens: number;
  tokensIn: number;
  tokensOut: number;
  costMicros: number;
  usagePriced: boolean;
  planId: string;
}

export interface RunPlanStep {
  tool: string;
  args: Record<string, unknown>;
}

export interface RunPlanUsage {
  tokensIn: number;
  tokensOut: number;
  costMicros: number;
  priced: boolean;
}

export interface RunPlan {
  planId: string;
  steps: RunPlanStep[];
  model: string;
  usage: RunPlanUsage;
}

export interface AgentRuns {
  items: AgentRun[];
}

export interface UsageTotals {
  plans: number;
  runs: number;
  tokensIn: number;
  tokensOut: number;
  tokens: number;
  costMicros: number;
  pricedPlans: number;
  unpricedPlans: number;
}

export interface UsageDay {
  day: string;
  plans: number;
  runs: number;
  tokensIn: number;
  tokensOut: number;
  tokens: number;
  costMicros: number;
  unpricedPlans: number;
}

export interface UsageModel {
  model: string;
  plans: number;
  tokensIn: number;
  tokensOut: number;
  tokens: number;
  costMicros: number;
  unpricedPlans: number;
}

export interface UsageSummary {
  since: string;
  days: number;
  totals: UsageTotals;
  byDay: UsageDay[];
  byModel: UsageModel[];
}

export type ProvenanceStatus = "unverified" | "source_cited" | "verified" | "disputed";

export interface PolicyProvenance {
  status: ProvenanceStatus;
  sourceUrl: string;
  verifiedAt: string;
  note: string;
}

export interface Policy {
  id: string;
  name: string;
  nameEn: string;
  category: string;
  subtype: string;
  hasProperty: boolean;
  hasPersonality: boolean;
  inheritable: boolean;
  requiredDocuments: string;
  processDescription: string;
  estimatedDifficulty: string;
  officialPolicyUrl: string;
  estimatedTimeDays: string;
  notes: string;
  dataSource: string;
  inheritanceFormUrl: string;
  monetizationPath: string;
  legalTemplateIds: string;
  riskWarnings: string;
  tags: string;
  provenance: PolicyProvenance;
}

export interface PoliciesPage {
  items: Policy[];
  nextCursor: string | null;
  total: number;
  categories: string[];
}

export type PolicyCandidateStatus = "pending" | "conflict" | "approved" | "rejected";

export interface PolicyCandidateDiff {
  field: string;
  before: string;
  after: string;
}

export interface PolicyCandidate {
  id: string;
  platformId: string;
  name: string;
  fetchedAt: string;
  source: string;
  status: PolicyCandidateStatus;
  hash: string;
  changedFields: string[];
  diff: PolicyCandidateDiff[];
  reviewedAt: string;
  reviewedBy: string;
}

export interface PolicyCandidatePage {
  items: PolicyCandidate[];
  nextCursor: string | null;
  total: number;
  lastSyncAt: string;
  lastStatus: string;
}

export interface PolicySyncResult {
  configured: boolean;
  source: string;
  fetched: number;
  changed: number;
  queued: number;
  conflict: number;
  skipped: number;
  notModified: boolean;
  cursor: string;
  lastSyncAt: string;
  message: string;
}

export interface DeliverableExport {
  jobId: string;
  status: string;
  progress: number;
  fileName?: string;
  downloadUrl?: string;
  generatedAt?: string;
}

export interface JobDetail {
  jobId: string;
  kind: string;
  status: string;
  progress: number;
  payload: Record<string, unknown>;
  result: Record<string, unknown>;
  errorCode: string;
  createdAt: string;
  updatedAt: string;
  finishedAt: string;
}

export interface SecurityStatus {
  configured: boolean;
  locked: boolean;
  recoveryConfigured: boolean;
  keySource: "os" | "passphrase" | "unknown";
  lastBackupAt: string;
}

export interface AuditItem {
  id: string;
  at: string;
  kind: string;
  subject: string;
  outcome: string;
  hash: string;
  prevHash: string;
}

export interface AuditPage {
  items: AuditItem[];
  nextCursor: string | null;
  total: number;
}

export interface Approval {
  id: string;
  runId: string;
  tool: string;
  objectKey: string;
  diff: string;
  status: "pending" | "approved" | "rejected" | "expired";
  reason: string;
  createdAt: string;
  decidedAt: string;
  jobId?: string;
}

export interface ApprovalPage {
  items: Approval[];
  pending: number;
}

export interface LocalModelItem {
  id: string;
  name: string;
  params: string;
  context: number;
  sizeBytes: number;
  sha256: string;
  license: string;
  description: string;
  runtime: "llama.cpp" | "experimental" | string;
  fileName: string;
  downloadUrl: string;
  modelId: string;
  downloadedBytes: number;
  verified: boolean;
  status: "missing" | "partial" | "downloading" | "ready" | "failed";
  progress: number;
  error: string | null;
  active: boolean;
}

export interface LocalModelRuntime {
  running: boolean;
  id?: string;
  port?: number;
  baseUrl?: string;
}

export interface LocalModelsPayload {
  items: LocalModelItem[];
  root: string;
  active: { id: string; model: string; baseUrl: string; apiKey: string; path: string } | null;
  runtime: LocalModelRuntime;
}

export type PlannerSource = "stored" | "env" | "local" | "none";

export interface BuiltinMcpEntry {
  id: string;
  name: string;
  source: "bundled" | "official" | "vendor";
  transport: "stdio" | "http";
  command: string;
  description: string;
  docsUrl: string;
}

export interface HarnessStatus {
  planner: {
    configured: boolean;
    source: PlannerSource;
    model: string;
    endpointHost: string;
    timeoutSecs: number;
  };
  runs: { total: number; running: number; waitingApproval: number; failed: number };
  approvals: { pending: number };
  usage: { tokensIn: number; tokensOut: number; costMicros: number; priced: boolean };
  tools: { total: number; mcp: number; write: number; draft: number };
  mcp: { servers: number; enabled: number };
  skills: { total: number; enabled: number; builtin: number };
  builtinMcp: BuiltinMcpEntry[];
}

export interface AgentSkill {
  id: string;
  name: string;
  description: string;
  instructions: string;
  enabled: boolean;
  builtin: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface AgentSkillPage {
  items: AgentSkill[];
  total: number;
}

export interface SkillUpsertInput {
  id?: string;
  name: string;
  description?: string;
  instructions: string;
  enabled?: boolean;
}

export interface SkillDeleteResult {
  id: string;
  name: string;
}
