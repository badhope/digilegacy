import type { SecurityReply } from "./security";

const USE_MOCK = import.meta.env.VITE_USE_MOCK === "true";
const API_BASE = import.meta.env.VITE_API_BASE ?? "";

export async function unlockVault(password: string): Promise<SecurityReply> {
  if (USE_MOCK) return { success: true, code: "OK", message: "mock" };
  try {
    const response = await fetch(`${API_BASE}/api/v1/security/unlock`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ password }),
    });
    return (await response.json()) as SecurityReply;
  } catch {
    return { success: false, code: "NETWORK", message: "本地 API 不可达" };
  }
}
