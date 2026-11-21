import { describe, expect, it } from "vitest";
import { ASSET_KINDS, ASSET_KIND_GROUPS } from "./model";

describe("asset kind groups", () => {
  it("places every kind in exactly one group", () => {
    const grouped = ASSET_KIND_GROUPS.flatMap((group) => group.kinds);
    expect(grouped).toHaveLength(ASSET_KINDS.length);
    expect(new Set(grouped)).toEqual(new Set(ASSET_KINDS));
  });

  it("keeps every legacy kind available", () => {
    for (const kind of ["account", "file", "device", "domain", "subscription", "physical", "other"]) {
      expect(ASSET_KINDS).toContain(kind);
    }
  });
});
