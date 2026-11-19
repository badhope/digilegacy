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

export interface AgentRun {
  id: string;
  status: "queued" | "running" | "waiting_approval" | "succeeded" | "failed" | "cancelled";
  title: string;
  steps: number;
  startedAt: string;
  tokens: number;
}

export interface AgentRuns {
  items: AgentRun[];
}

export interface SecurityStatus {
  locked: boolean;
  recoveryConfigured: boolean;
  keySource: "os" | "passphrase" | "unknown";
  lastBackupAt: string;
}
