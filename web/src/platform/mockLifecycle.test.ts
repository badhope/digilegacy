import { describe, expect, it } from "vitest";
import { platformClient } from "./client";

describe("mock asset lifecycle", () => {
  it("reflects the requested status in offline mock mode", async () => {
    const result = await platformClient.updateAsset({
      id: "ast_01",
      name: "家庭照片与影像库",
      kind: "file",
      platform: "iCloud",
      value: "2014–2024",
      note: "",
      status: "archived",
    });
    expect(result.success).toBe(true);
    expect(result.data?.status).toBe("archived");
  });
});
