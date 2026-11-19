import { platformClient } from "@/platform/client";

export function loadAssets() {
  return platformClient.listAssets();
}

export function loadAsset(id: string) {
  return platformClient.getAsset(id);
}
