import { describe, expect, it } from "vitest";
import { ASSET_KINDS, ASSET_KIND_GROUPS, ASSET_VALUE_STATUSES, assetValueStatusLabel, draftFromAsset, formatAssetValue, normalizeValuation } from "./model";

describe("asset kind groups", () => {
  it("places every kind in exactly one group", () => {
    const grouped = ASSET_KIND_GROUPS.flatMap((group) => group.kinds);
    expect(grouped).toHaveLength(ASSET_KINDS.length);
    expect(new Set(grouped)).toEqual(new Set(ASSET_KINDS));
  });

  it("normalizes human valuations and rejects prose", () => {
    expect(normalizeValuation("")).toBe("");
    expect(normalizeValuation("¥3,200")).toBe("3200");
    expect(normalizeValuation("1.2万")).toBe("12000");
    expect(normalizeValuation("50万")).toBe("500000");
    expect(normalizeValuation("120万")).toBe("1200000");
    expect(normalizeValuation("约 50 万元")).toBe("500000");
    expect(normalizeValuation("3200元")).toBe("3200");
    expect(normalizeValuation("待评估")).toBeNull();
    expect(normalizeValuation("保额50万")).toBeNull();
    expect(normalizeValuation("-1")).toBeNull();
  });

  it("formats stored CNY values for display", () => {
    expect(formatAssetValue("")).toBe("");
    expect(formatAssetValue("3200", "zh-CN")).toContain("3,200");
    expect(formatAssetValue("3200", "zh-CN")).not.toContain(".00");
    expect(formatAssetValue("1200000", "zh-CN")).toContain("1,200,000");
  });

  it("keeps every legacy kind available", () => {
    for (const kind of ["account", "file", "device", "domain", "subscription", "physical", "other"]) {
      expect(ASSET_KINDS).toContain(kind);
    }
  });

  it("keeps the draft and the asset schema aligned", () => {
    const draft = draftFromAsset({
      id: "7",
      name: "老宅",
      kind: "real_estate",
      platform: "不动产登记中心",
      valueStatus: "appraised",
      identifier: "闽(2026)第0001号",
      ownershipShare: "50%",
      beneficiary: "林小雨",
      beneficiaryContact: "lin@example.com",
      credentialLocation: "保险柜 A-03",
      expiresAt: "2056-12-31",
      quantity: "1",
      unit: "套",
    });
    expect(draft).toMatchObject({
      name: "老宅",
      kind: "real_estate",
      platform: "不动产登记中心",
      valueStatus: "appraised",
      identifier: "闽(2026)第0001号",
      ownershipShare: "50%",
      beneficiary: "林小雨",
      beneficiaryContact: "lin@example.com",
      credentialLocation: "保险柜 A-03",
      expiresAt: "2056-12-31",
      quantity: "1",
      unit: "套",
    });
  });

  it("labels every value status in both locales", () => {
    for (const status of ASSET_VALUE_STATUSES) {
      expect(assetValueStatusLabel(status, "zh-CN").length).toBeGreaterThan(0);
      expect(assetValueStatusLabel(status, "en-US").length).toBeGreaterThan(0);
    }
    expect(assetValueStatusLabel("unknown", "zh-CN")).toBe("待评估");
    expect(assetValueStatusLabel("market", "en-US")).toBe("Market value");
  });
});
