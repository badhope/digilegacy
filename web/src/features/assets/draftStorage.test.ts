import { beforeEach, describe, expect, it } from "vitest";
import { draftFromAsset } from "./model";
import { clearAssetDraft, hasMeaningfulDraft, readAssetDraft, writeAssetDraft } from "./draftStorage";

describe("asset draft storage", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
  });

  it("ignores a blank draft", () => {
    expect(hasMeaningfulDraft(draftFromAsset(null))).toBe(false);
    writeAssetDraft(draftFromAsset(null));
    expect(readAssetDraft()).toBeNull();
  });

  it("does not keep a type-only reset as a draft", () => {
    const draft = { ...draftFromAsset(null), kind: "financial" as const, valueStatus: "appraised" as const };
    expect(hasMeaningfulDraft(draft)).toBe(false);
    writeAssetDraft(draft);
    expect(readAssetDraft()).toBeNull();
  });

  it("round-trips deep fields through sessionStorage", () => {
    const draft = {
      ...draftFromAsset(null),
      name: "老宅",
      kind: "real_estate" as const,
      ownershipShare: "50%",
      credentialLocation: "保险柜 A-03",
      expiresAt: "2056-12-31",
    };
    writeAssetDraft(draft);
    expect(readAssetDraft()).toMatchObject({
      name: "老宅",
      kind: "real_estate",
      ownershipShare: "50%",
      credentialLocation: "保险柜 A-03",
      expiresAt: "2056-12-31",
    });
    clearAssetDraft();
    expect(readAssetDraft()).toBeNull();
  });
});
