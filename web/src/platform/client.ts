import { COMMAND_PERMISSION, COMMANDS, INVOKE_PATH, type CommandName } from "./generated/ipc";
import { mockInvoke } from "./generated/mock";
import { cachedQuery, clearQueryCache, invalidateQuery, queryCacheKey } from "./queryCache";
import type { UsageSummary, AgentRuns, ApprovalPage, Approval, AssetsPage, Asset, DiscoveryResult, BulkCreateResult, AuditPage, CheckinResult, DeliverableExport, Envelope, JobDetail, NotificationPage, NotificationSendResult, Overview, Plan, PoliciesPage, Policy, RunDetail, RunPlan, SecurityStatus, Trigger, TriggerEventPage } from "./types";

const USE_MOCK = import.meta.env.VITE_USE_MOCK === "true";
const API_BASE = import.meta.env.VITE_API_BASE ?? "";
const WRITE_TOKEN_KEY = "digilegacy.writeToken";

function readWriteToken(): string {
  if (typeof window === "undefined") return "";
  return window.sessionStorage.getItem(WRITE_TOKEN_KEY) ?? "";
}

function persistWriteToken(token: string): void {
  if (typeof window === "undefined") return;
  if (token) {
    window.sessionStorage.setItem(WRITE_TOKEN_KEY, token);
  } else {
    window.sessionStorage.removeItem(WRITE_TOKEN_KEY);
  }
}

function traceId(): string {
  return `tr_web_${Date.now().toString(36)}_${Math.random().toString(16).slice(2, 8)}`;
}

function mockEnvelope<T>(name: CommandName): Envelope<T> {
  return {
    success: true,
    code: "OK",
    message: "mock",
    data: mockInvoke(name) as T,
    details: null,
    traceId: traceId(),
  };
}

async function invoke<T>(name: CommandName, args: Record<string, unknown> = {}): Promise<Envelope<T>> {
  if (USE_MOCK) {
    await new Promise((resolve) => setTimeout(resolve, name === COMMANDS.appOverview ? 320 : 180));
    return mockEnvelope<T>(name);
  }

  const token = COMMAND_PERMISSION[name] === "write" ? readWriteToken() : "";
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers.Authorization = `Bearer ${token}`;

  const response = await fetch(`${API_BASE}${INVOKE_PATH}`, {
    method: "POST",
    headers,
    body: JSON.stringify({ name, args }),
  });

  const envelope = (await response.json()) as Envelope<T>;
  if (!response.ok && envelope.success) {
    return {
      ...envelope,
      success: false,
      code: `HTTP_${response.status}`,
      message: `Platform API 返回 HTTP ${response.status}`,
    };
  }
  return envelope;
}

async function read<T>(name: CommandName, args: Record<string, unknown>, scope: string): Promise<Envelope<T>> {
  return cachedQuery<T>(`${scope}:${queryCacheKey(name, args)}`, () => invoke<T>(name, args));
}

async function write<T>(
  name: CommandName,
  args: Record<string, unknown>,
  scopes: string[],
): Promise<Envelope<T>> {
  const envelope = await invoke<T>(name, args);
  if (envelope.success) {
    for (const scope of scopes) invalidateQuery(scope);
    invalidateQuery("audit");
  }
  return envelope;
}

export const platformClient = {
  commands: COMMANDS,
  useMock: USE_MOCK,
  getWriteToken: readWriteToken,
  setWriteToken: (token: string) => {
    persistWriteToken(token);
    clearQueryCache();
  },

  getOverview: () => read<Overview>(COMMANDS.appOverview, {}, "overview"),
  listAssets: (args: Record<string, unknown> = {}) => read<AssetsPage>(COMMANDS.assetsList, args, "assets"),
  getAsset: (id: string) => read<Asset>(COMMANDS.assetsGet, { id }, "assets"),
  createAsset: (args: { name: string; kind: string; platform?: string; value?: string; note?: string }) =>
    write<Asset>(COMMANDS.assetsCreate, args, ["assets", "overview"]),
  updateAsset: (args: { id: string; name: string; kind: string; platform?: string; value?: string; note?: string }) =>
    write<Asset>(COMMANDS.assetsUpdate, args, ["assets", "overview"]),
  discoverAssets: (args: { category?: string; q?: string } = {}) =>
    read<DiscoveryResult>(COMMANDS.assetsDiscover, args, "assets.discover"),
  bulkCreateAssets: (items: Array<{ name: string; kind: string; platform?: string; value?: string; note?: string }>) =>
    write<BulkCreateResult>(COMMANDS.assetsBulkCreate, { items }, ["assets", "overview", "assets.discover"]),
  getPlan: () => read<Plan>(COMMANDS.planGet, {}, "plan"),
  configureTrigger: (args: { enabled?: boolean; days?: number; pauseDays?: number }) =>
    write<Trigger>(COMMANDS.triggerConfigure, args, ["plan", "trigger.events", "overview"]),
  evaluateTrigger: () => write<Trigger>(COMMANDS.triggerEvaluate, {}, ["plan", "trigger.events", "overview"]),
  triggerEvents: (args: Record<string, unknown> = {}) => read<TriggerEventPage>(COMMANDS.triggerEventsList, args, "trigger.events"),
  listNotifications: (args: Record<string, unknown> = {}) => read<NotificationPage>(COMMANDS.notificationsList, args, "notifications"),
  sendNotifications: (args: Record<string, unknown> = {}) => write<NotificationSendResult>(COMMANDS.notificationsSend, args, ["notifications"]),
  checkin: () => write<CheckinResult>(COMMANDS.triggerCheckin, {}, ["plan", "trigger.events", "overview", "audit"]),
  listPolicies: (args: Record<string, unknown> = {}) => read<PoliciesPage>(COMMANDS.policiesList, args, "policies"),
  getPolicy: (id: string) => read<Policy>(COMMANDS.policiesGet, { id }, "policies"),
  setPolicyProvenance: (args: { id: string; status: string; sourceUrl?: string; note?: string }) =>
    write<Policy>(COMMANDS.policiesProvenanceSet, args, ["policies", "overview"]),
  exportDeliverable: (kind: string, locale = "zh-CN") =>
    write<DeliverableExport>(COMMANDS.deliverExport, { kind, locale, wait: false }, ["jobs", "overview"]),
  // Job/run detail is polled or streamed; never cache volatile progress.
  getJob: (id: string) => invoke<JobDetail>(COMMANDS.jobsGet, { id }),
  cancelJob: (id: string) => write<JobDetail>(COMMANDS.jobsCancel, { id }, ["jobs"]),
  exportUrl: (path: string) => `${API_BASE}${path}`,
  listRuns: (args: Record<string, unknown> = {}) => read<AgentRuns>(COMMANDS.agentRunsList, args, "agent.runs"),
  getRun: (id: string) => invoke<RunDetail>(COMMANDS.agentRunsGet, { id }),
  usageSummary: (days = 30) => read<UsageSummary>(COMMANDS.agentUsageSummary, { days }, "agent.usage"),
  planRun: (task: string, maxSteps = 20) =>
    write<RunPlan>(COMMANDS.agentRunsPlan, { task, maxSteps }, ["agent.usage", "agent.runs"]),
  startRun: (task: string, steps: Array<{ tool: string; args: Record<string, unknown> }>, maxSteps = 20, maxWriteSteps = 5, planId = "") =>
    write<RunDetail>(COMMANDS.agentRunsStart, { task, steps, maxSteps, maxWriteSteps, ...(planId ? { planId } : {}) }, ["agent.runs", "agent.usage", "overview", "agent.approvals"]),
  cancelRun: (id: string) => write<RunDetail>(COMMANDS.agentRunsCancel, { id }, ["agent.runs", "overview", "agent.approvals"]),
  retryRun: (id: string) => write<RunDetail>(COMMANDS.agentRunsRetry, { id }, ["agent.runs", "overview", "agent.approvals"]),
  getSecurity: () => read<SecurityStatus>(COMMANDS.securityStatus, {}, "security"),
  listAudit: (args: Record<string, unknown> = {}) => read<AuditPage>(COMMANDS.auditList, args, "audit"),
  listApprovals: (args: Record<string, unknown> = {}) => read<ApprovalPage>(COMMANDS.agentApprovalsList, args, "agent.approvals"),
  decideApproval: (id: string, decision: "approve" | "reject", reason = "") =>
    write<Approval>(COMMANDS.agentApprovalsDecide, { id, decision, reason }, ["agent.approvals", "agent.runs", "overview"]),
};
