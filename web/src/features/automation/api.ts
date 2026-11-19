import { platformClient } from "@/platform/client";

export function loadRuns() {
  return platformClient.listRuns();
}
