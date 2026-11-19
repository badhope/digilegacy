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

export type AssetKind = "account" | "file" | "device" | "domain" | "subscription" | "physical" | "other";
export type AssetStatus = "active" | "archived";

export interface Asset {
  id: string;
  name: string;
  kind: AssetKind;
  platform: string;
  status: AssetStatus;
  value: string;
  note: string;
  updatedAt: string;
}

export interface AssetsPage {
  items: Asset[];
  nextCursor: string | null;
  total: number;
}

export interface Heir {
  id: string;
  name: string;
  role: string;
}

export interface Plan {
  holder: { name: string; updatedAt: string };
  heirs: Heir[];
  trigger: { mode: string; days: number; status: string };
}

export type RunStatus = "queued" | "running" | "waiting_approval" | "succeeded" | "failed" | "rejected" | "cancelled";

export interface AgentRun {
  id: string;
  status: RunStatus;
  title: string;
  steps: number;
  startedAt: string;
  tokens: number;
}

export interface RunStep {
  tool: string;
  args: Record<string, unknown>;
  status: "queued" | "pending_approval" | "done" | "failed" | "rejected";
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
  finalText: string;
}

export interface AgentRuns {
  items: AgentRun[];
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

export interface DeliverableExport {
  jobId: string;
  status: string;
  progress: number;
  fileName: string;
  downloadUrl: string;
  generatedAt: string;
}

export interface SecurityStatus {
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
}

export interface ApprovalPage {
  items: Approval[];
  pending: number;
}
