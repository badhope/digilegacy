import { platformClient } from "@/platform/client";

export function loadSecurity() {
  return platformClient.getSecurity();
}

export function loadAudit(args: Record<string, unknown> = {}) {
  return platformClient.listAudit(args);
}

export function loadConnections() {
  return platformClient.listConnections();
}
