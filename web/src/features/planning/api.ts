import { platformClient } from "@/platform/client";

export function loadPlan() {
  return platformClient.getPlan();
}
