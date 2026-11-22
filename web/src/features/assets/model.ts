import type { Asset, AssetKind, Locale } from "@/platform/types";
import { t } from "@/platform/i18n";

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
  "financial",
  "real_estate",
  "vehicle",
  "intellectual_property",
  "business",
  "virtual_item",
  "membership",
  "identity",
  "other",
];

export interface AssetKindGroup {
  label: string;
  kinds: AssetKind[];
}

const KIND_LABELS: Partial<Record<AssetKind, { zh: string; en: string }>> = {
  financial: { zh: "金融资产", en: "Financial" },
  real_estate: { zh: "不动产", en: "Real estate" },
  vehicle: { zh: "车辆/船舶", en: "Vehicle / vessel" },
  intellectual_property: { zh: "知识产权", en: "Intellectual property" },
  business: { zh: "企业经营", en: "Business interest" },
  virtual_item: { zh: "虚拟物品", en: "Virtual item" },
  membership: { zh: "会员/积分", en: "Membership / points" },
  identity: { zh: "身份/权益", en: "Identity / rights" },
};

const GROUP_LABELS: Record<string, { zh: string; en: string }> = {
  "asset.group.finance": { zh: "金融资产", en: "Financial" },
  "asset.group.property": { zh: "不动产与车辆", en: "Real estate & vehicles" },
  "asset.group.physical": { zh: "实物与收藏", en: "Physical & collectibles" },
  "asset.group.ip": { zh: "知识产权", en: "Intellectual property" },
  "asset.group.business": { zh: "企业经营", en: "Business" },
  "asset.group.digital": { zh: "数字账号", en: "Digital accounts" },
  "asset.group.virtual": { zh: "虚拟物品", en: "Virtual items" },
  "asset.group.infra": { zh: "文件、设备与域名", en: "Files, devices & domains" },
  "asset.group.subscription": { zh: "订阅与会员", en: "Subscriptions & memberships" },
  "asset.group.identity": { zh: "身份与权益", en: "Identity & rights" },
  "asset.group.other": { zh: "其他", en: "Other" },
};


/** Normalize human-typed valuations (CNY, thousands separators and 万/亿 units). */
export function normalizeValuation(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  let text = trimmed.replace(/[¥￥,，\s]/g, "").replace(/^约/, "").replace(/元$/, "");
  let factor = 1;
  if (text.endsWith("万元")) {
    text = text.slice(0, -2);
    factor = 10_000;
  } else if (text.endsWith("万")) {
    text = text.slice(0, -1);
    factor = 10_000;
  } else if (text.endsWith("亿")) {
    text = text.slice(0, -1);
    factor = 100_000_000;
  }
  const parsed = Number(text);
  if (!Number.isFinite(parsed) || parsed < 0) return null;
  const scaled = parsed * factor;
  if (!Number.isFinite(scaled)) return null;
  return Number.isInteger(scaled) ? String(scaled) : String(Number(scaled.toFixed(2)));
}

/** Format a stored CNY value for display without exposing raw floats. */
export function formatAssetValue(raw: string, locale = "zh-CN"): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed)) return trimmed;
  return new Intl.NumberFormat(locale, {
    style: "currency",
    currency: "CNY",
    currencyDisplay: "narrowSymbol",
    maximumFractionDigits: 2,
    minimumFractionDigits: 0,
  }).format(parsed);
}

export const ASSET_KIND_GROUPS: AssetKindGroup[] = [
  { label: "asset.group.finance", kinds: ["financial"] },
  { label: "asset.group.property", kinds: ["real_estate", "vehicle"] },
  { label: "asset.group.physical", kinds: ["physical"] },
  { label: "asset.group.ip", kinds: ["intellectual_property"] },
  { label: "asset.group.business", kinds: ["business"] },
  { label: "asset.group.digital", kinds: ["account"] },
  { label: "asset.group.virtual", kinds: ["virtual_item"] },
  { label: "asset.group.infra", kinds: ["file", "device", "domain"] },
  { label: "asset.group.subscription", kinds: ["subscription", "membership"] },
  { label: "asset.group.identity", kinds: ["identity"] },
  { label: "asset.group.other", kinds: ["other"] },
];

export function assetKindKey(kind: AssetKind): string {
  return `asset.kind.${kind}`;
}

export function assetKindLabel(kind: AssetKind, locale: Locale): string {
  const label = KIND_LABELS[kind];
  return label ? label[locale === "en-US" ? "en" : "zh"] : t(assetKindKey(kind));
}

export function assetGroupLabel(label: string, locale: Locale): string {
  const group = GROUP_LABELS[label];
  return group ? group[locale === "en-US" ? "en" : "zh"] : t(label);
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
