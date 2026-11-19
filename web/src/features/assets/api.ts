import { platformClient } from "@/platform/client";

export function loadAssets(args: Record<string, unknown> = {}) {
  return platformClient.listAssets(args);
}

export function loadAsset(id: string) {
  return platformClient.getAsset(id);
}
