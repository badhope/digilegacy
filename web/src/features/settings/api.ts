import { platformClient } from "@/platform/client";

export function loadSecurity() {
  return platformClient.getSecurity();
}
