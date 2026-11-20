import { COMMAND_PERMISSION, COMMANDS, INVOKE_PATH, type CommandName } from "./generated/ipc";
import { mockInvoke } from "./generated/mock";
import { cachedQuery, clearQueryCache, invalidateQuery, queryCacheKey } from "./queryCache";
import type { HeirDraft, UsageSummary, AgentRuns, ApprovalPage, Approval, AssetsPage, Asset, LocalModelsPayload, ConnectionKind,
  ConnectionItem,
  ConnectionUpsertInput,
  ConnectionDeleteResult, ConnectionPage, ConnectionTestResult, DiscoveryResult, BulkCreateResult, AuditPage, CheckinResult, DeliverableExport, Envelope, JobDetail, NotificationPage, NotificationSendResult, Overview, Plan, PoliciesPage, Policy, PolicyCandidate, PolicyCandidatePage, PolicySyncResult, RunDetail, RunPlan, SecurityStatus, Trigger, TriggerEventPage } from "./types";

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

function mockEnvelope<T>(name: CommandName, args: Record<string, unknown> = {}): Envelope<T> {
  const fixture = mockInvoke(name);
  const data =
    name === COMMANDS.assetsUpdate || name === COMMANDS.assetsCreate
      ? { ...(fixture as Record<string, unknown>), ...args }
      : fixture;
  return {
    success: true,
    code: "OK",
    message: "mock",
    data: data as T,
    details: null,
    traceId: traceId(),
  };
}

function networkFailure<T>(message: string): Envelope<T> {
  return {
    success: false,
    code: "NETWORK",
    message,
    data: null as T,
    details: null,
    traceId: traceId(),
  };
}

async function invoke<T>(name: CommandName, args: Record<string, unknown> = {}): Promise<Envelope<T>> {
  if (USE_MOCK) {
    await new Promise((resolve) => setTimeout(resolve, name === COMMANDS.appOverview ? 320 : 180));
    return mockEnvelope<T>(name, args);
  }

  const token = COMMAND_PERMISSION[name] === "write" ? readWriteToken() : "";
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers.Authorization = `Bearer ${token}`;

  let response: Response;
  try {
    response = await fetch(`${API_BASE}${INVOKE_PATH}`, {
      method: "POST",
      headers,
      body: JSON.stringify({ name, args }),
    });
  } catch {
    return networkFailure<T>("无法连接平台 API，请确认服务已启动。");
  }

  let envelope: Envelope<T>;
  try {
    envelope = (await response.json()) as Envelope<T>;
  } catch {
    return networkFailure<T>(
      response.status >= 500
        ? `平台 API 暂不可用（HTTP ${response.status}）。`
        : `平台 API 返回了无法解析的响应（HTTP ${response.status}）。`,
    );
  }
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

const MOCK_LOCAL_MODELS: LocalModelsPayload = {
  items: [
    {
      id: "minicpm5-1b",
      name: "MiniCPM5-1B",
      params: "1.08B 稠密",
      context: 131072,
      sizeBytes: 688065920,
      sha256: "81b64d05a23b17b34c475f42b3e72fbde62d4b92cc34541f7a8031d0752deafa",
      license: "Apache-2.0",
      description: "1B 级开源 SOTA，128K 上下文，标准 Llama 架构；默认推荐。",
      recommended: true,
      strictUnder2b: true,
      runtime: "llama.cpp",
      fileName: "MiniCPM5-1B-Q4_K_M.gguf",
      downloadUrl: "https://modelscope.cn/models/OpenBMB/MiniCPM5-1B-GGUF",
      modelId: "MiniCPM5-1B-Q4_K_M",
      downloadedBytes: 688065920,
      verified: true,
      status: "ready",
      progress: 1,
      error: null,
      active: false,
    },
  ],
  root: "data/models",
  active: null,
  runtime: { running: false },
};

async function localModelRequest<T>(path: string, method: "GET" | "POST"): Promise<Envelope<T>> {
  if (USE_MOCK) {
    return {
      success: true,
      code: "OK",
      message: "mock",
      data: MOCK_LOCAL_MODELS as T,
      details: null,
      traceId: traceId(),
    };
  }
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const token = readWriteToken();
  if (token) headers.Authorization = `Bearer ${token}`;
  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, { method, headers });
  } catch {
    return networkFailure<T>("无法连接本地模型服务，请确认平台 API 已启动。");
  }
  let envelope: Envelope<T>;
  try {
    envelope = (await response.json()) as Envelope<T>;
  } catch {
    return networkFailure<T>(
      response.status >= 500
        ? `本地模型服务暂不可用（HTTP ${response.status}）。`
        : `本地模型服务返回了无法解析的响应（HTTP ${response.status}）。`,
    );
  }
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

export const platformClient = {
  commands: COMMANDS,
  useMock: USE_MOCK,
  getWriteToken: readWriteToken,
  setWriteToken: (token: string) => {
    persistWriteToken(token);
    clearQueryCache();
    if (typeof window !== "undefined") {
      window.dispatchEvent(new Event("digilegacy:write-token"));
    }
  },

  getOverview: () => read<Overview>(COMMANDS.appOverview, {}, "overview"),
  listAssets: (args: Record<string, unknown> = {}) => read<AssetsPage>(COMMANDS.assetsList, args, "assets"),
  getAsset: (id: string) => read<Asset>(COMMANDS.assetsGet, { id }, "assets"),
  createAsset: (args: { name: string; kind: string; platform?: string; value?: string; note?: string }) =>
    write<Asset>(COMMANDS.assetsCreate, args, ["assets", "overview"]),
  updateAsset: (args: {
    id: string;
    name: string;
    kind: string;
    platform?: string;
    value?: string;
    note?: string;
    status?: Asset["status"];
  }) => write<Asset>(COMMANDS.assetsUpdate, args, ["assets", "overview"]),
  discoverAssets: (args: { category?: string; q?: string } = {}) =>
    read<DiscoveryResult>(COMMANDS.assetsDiscover, args, "assets.discover"),
  bulkCreateAssets: (items: Array<{ name: string; kind: string; platform?: string; value?: string; note?: string }>) =>
    write<BulkCreateResult>(COMMANDS.assetsBulkCreate, { items }, ["assets", "overview", "assets.discover"]),
  getPlan: () => read<Plan>(COMMANDS.planGet, {}, "plan"),
  setPlanProfile: (name: string) =>
    write<Plan>(COMMANDS.planProfileSet, { name }, ["plan", "overview"]),
  setPlanHeirs: (heirs: HeirDraft[]) =>
    write<Plan>(COMMANDS.planHeirsSet, { heirs }, ["plan", "overview"]),
  configureTrigger: (args: { enabled?: boolean; days?: number; pauseDays?: number }) =>
    write<Trigger>(COMMANDS.triggerConfigure, args, ["plan", "trigger.events", "overview"]),
  evaluateTrigger: () => write<Trigger>(COMMANDS.triggerEvaluate, {}, ["plan", "trigger.events", "overview"]),
  triggerEvents: (args: Record<string, unknown> = {}) => read<TriggerEventPage>(COMMANDS.triggerEventsList, args, "trigger.events"),
  listConnections: (args: Record<string, unknown> = {}) => read<ConnectionPage>(COMMANDS.connectionsList, args, "connections"),
  testConnection: (args: { kind: ConnectionKind; id?: number; label?: string; endpoint?: string }) =>
    write<ConnectionTestResult>(COMMANDS.connectionsTest, args, ["connections"]),
  saveConnection: (args: ConnectionUpsertInput) =>
    write<ConnectionItem>(COMMANDS.connectionsUpsert, { ...args } as Record<string, unknown>, ["connections", "overview"]),
  deleteConnection: (id: string) =>
    write<ConnectionDeleteResult>(COMMANDS.connectionsDelete, { id }, ["connections", "overview"]),
  listNotifications: (args: Record<string, unknown> = {}) => read<NotificationPage>(COMMANDS.notificationsList, args, "notifications"),
  sendNotifications: (args: Record<string, unknown> = {}) => write<NotificationSendResult>(COMMANDS.notificationsSend, args, ["notifications"]),
  checkin: () => write<CheckinResult>(COMMANDS.triggerCheckin, {}, ["plan", "trigger.events", "overview", "audit"]),
  listPolicies: (args: Record<string, unknown> = {}) => read<PoliciesPage>(COMMANDS.policiesList, args, "policies"),
  getPolicy: (id: string) => read<Policy>(COMMANDS.policiesGet, { id }, "policies"),
  setPolicyProvenance: (args: { id: string; status: string; sourceUrl?: string; note?: string }) =>
    write<Policy>(COMMANDS.policiesProvenanceSet, args, ["policies", "overview"]),
  syncPolicies: (since = "") =>
    write<PolicySyncResult>(COMMANDS.policiesSync, { since }, ["policy.candidates", "audit", "overview"]),
  listPolicyCandidates: (args: Record<string, unknown> = {}) =>
    read<PolicyCandidatePage>(COMMANDS.policiesCandidatesList, args, "policy.candidates"),
  decidePolicyCandidate: (args: { id: string; action: "approve" | "reject"; note?: string }) =>
    write<PolicyCandidate>(COMMANDS.policiesCandidateDecide, args, ["policies", "policy.candidates", "audit", "overview"]),
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
  listLocalModels: () => localModelRequest<LocalModelsPayload>("/api/v1/local-models", "GET"),
  downloadLocalModel: (id: string) => localModelRequest<Record<string, unknown>>(`/api/v1/local-models/${encodeURIComponent(id)}/download`, "POST"),
  activateLocalModel: (id: string) => localModelRequest<LocalModelsPayload>(`/api/v1/local-models/${encodeURIComponent(id)}/activate`, "POST"),
  stopLocalModel: () => localModelRequest<Record<string, unknown>>("/api/v1/local-models/stop", "POST"),
  getSecurity: () => read<SecurityStatus>(COMMANDS.securityStatus, {}, "security"),
  listAudit: (args: Record<string, unknown> = {}) => read<AuditPage>(COMMANDS.auditList, args, "audit"),
  listApprovals: (args: Record<string, unknown> = {}) => read<ApprovalPage>(COMMANDS.agentApprovalsList, args, "agent.approvals"),
  decideApproval: (id: string, decision: "approve" | "reject", reason = "") =>
    write<Approval>(COMMANDS.agentApprovalsDecide, { id, decision, reason }, ["agent.approvals", "agent.runs", "overview"]),
};
