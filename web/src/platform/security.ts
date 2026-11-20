import type { SecurityStatus } from "./types";

const USE_MOCK = import.meta.env.VITE_USE_MOCK === "true";
const API_BASE = import.meta.env.VITE_API_BASE ?? "";

export interface SecurityReply {
  success: boolean;
  code: string;
  message: string;
  data?: SecurityStatus;
}

const MOCK_STATUS: SecurityStatus = {
  configured: false,
  locked: false,
  recoveryConfigured: false,
  keySource: "unknown",
  lastBackupAt: "",
};

export async function fetchSecurityStatus(): Promise<SecurityStatus | null> {
  if (USE_MOCK) return MOCK_STATUS;
  try {
    const response = await fetch(`${API_BASE}/api/v1/security`);
    const envelope = (await response.json()) as SecurityReply;
    return envelope.success && envelope.data ? envelope.data : null;
  } catch {
    return null;
  }
}

export async function lockVault(): Promise<boolean> {
  if (USE_MOCK) return false;
  try {
    return (await fetch(`${API_BASE}/api/v1/security/lock`, { method: "POST" })).ok;
  } catch {
    return false;
  }
}
