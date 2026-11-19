import type { Asset, AssetKind } from "@/platform/types";

export interface AssetDraft {
  name: string;
  kind: AssetKind;
  platform: string;
  value: string;
  note: string;
}

export const ASSET_KINDS: AssetKind[] = [
  "account",
  "file",
  "device",
  "domain",
  "subscription",
  "physical",
  "other",
];

export function assetKindKey(kind: AssetKind): string {
  return `asset.kind.${kind}`;
}

export function draftFromAsset(asset?: Partial<Asset> | null): AssetDraft {
  return {
    name: asset?.name ?? "",
    kind: asset?.kind ?? "account",
    platform: asset?.platform ?? "",
    value: asset?.value ?? "",
    note: asset?.note ?? "",
  };
}
