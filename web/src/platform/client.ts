import { COMMAND_PERMISSION, COMMANDS, INVOKE_PATH, type CommandName } from "./generated/ipc";
import { mockInvoke } from "./generated/mock";
import type { AgentRuns, ApprovalPage, Approval, AssetsPage, Asset, AuditPage, DeliverableExport, Envelope, Overview, Plan, PoliciesPage, Policy, RunDetail, RunPlan, SecurityStatus } from "./types";

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

export const platformClient = {
  commands: COMMANDS,
  useMock: USE_MOCK,
  getWriteToken: readWriteToken,
  setWriteToken: persistWriteToken,

  getOverview: () => invoke<Overview>(COMMANDS.appOverview),
  listAssets: (args: Record<string, unknown> = {}) => invoke<AssetsPage>(COMMANDS.assetsList, args),
  getAsset: (id: string) => invoke<Asset>(COMMANDS.assetsGet, { id }),
  createAsset: (args: Record<string, unknown>) => invoke<Asset>(COMMANDS.assetsCreate, args),
  updateAsset: (args: Record<string, unknown>) => invoke<Asset>(COMMANDS.assetsUpdate, args),
  getPlan: () => invoke<Plan>(COMMANDS.planGet),
  listPolicies: (args: Record<string, unknown> = {}) => invoke<PoliciesPage>(COMMANDS.policiesList, args),
  getPolicy: (id: string) => invoke<Policy>(COMMANDS.policiesGet, { id }),
  setPolicyProvenance: (args: { id: string; status: string; sourceUrl?: string; note?: string }) =>
    invoke<Policy>(COMMANDS.policiesProvenanceSet, args),
  exportDeliverable: (kind: string, locale = "zh-CN") =>
    invoke<DeliverableExport>(COMMANDS.deliverExport, { kind, locale }),
  exportUrl: (path: string) => `${API_BASE}${path}`,
  listRuns: (args: Record<string, unknown> = {}) => invoke<AgentRuns>(COMMANDS.agentRunsList, args),
  getRun: (id: string) => invoke<RunDetail>(COMMANDS.agentRunsGet, { id }),
  planRun: (task: string, maxSteps = 20) =>
    invoke<RunPlan>(COMMANDS.agentRunsPlan, { task, maxSteps }),
  startRun: (task: string, steps: Array<{ tool: string; args: Record<string, unknown> }>, maxSteps = 20, maxWriteSteps = 5) =>
    invoke<RunDetail>(COMMANDS.agentRunsStart, { task, steps, maxSteps, maxWriteSteps }),
  cancelRun: (id: string) => invoke<RunDetail>(COMMANDS.agentRunsCancel, { id }),
  retryRun: (id: string) => invoke<RunDetail>(COMMANDS.agentRunsRetry, { id }),
  getSecurity: () => invoke<SecurityStatus>(COMMANDS.securityStatus),
  listAudit: (args: Record<string, unknown> = {}) => invoke<AuditPage>(COMMANDS.auditList, args),
  listApprovals: (args: Record<string, unknown> = {}) => invoke<ApprovalPage>(COMMANDS.agentApprovalsList, args),
  decideApproval: (id: string, decision: "approve" | "reject", reason = "") =>
    invoke<Approval>(COMMANDS.agentApprovalsDecide, { id, decision, reason }),
};
