import { platformClient } from "@/platform/client";

export function loadOverview() {
  return platformClient.getOverview();
}
