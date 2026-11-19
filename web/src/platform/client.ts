import { COMMANDS, INVOKE_PATH, type CommandName } from "./generated/ipc";
import { mockInvoke } from "./generated/mock";
import type { AgentRuns, AssetsPage, Asset, Envelope, Overview, Plan, SecurityStatus } from "./types";

const USE_MOCK = import.meta.env.VITE_USE_MOCK !== "false";
const API_BASE = import.meta.env.VITE_API_BASE ?? "";

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

  const response = await fetch(`${API_BASE}${INVOKE_PATH}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name, args }),
  });

  const envelope = (await response.json()) as Envelope<T>;
  return envelope;
}

export const platformClient = {
  commands: COMMANDS,
  useMock: USE_MOCK,

  getOverview: () => invoke<Overview>(COMMANDS.appOverview),
  listAssets: (args: Record<string, unknown> = {}) => invoke<AssetsPage>(COMMANDS.assetsList, args),
  getAsset: (id: string) => invoke<Asset>(COMMANDS.assetsGet, { id }),
  getPlan: () => invoke<Plan>(COMMANDS.planGet),
  listRuns: () => invoke<AgentRuns>(COMMANDS.agentRunsList),
  getSecurity: () => invoke<SecurityStatus>(COMMANDS.securityStatus),
};
