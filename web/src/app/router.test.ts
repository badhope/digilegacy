import { describe, expect, it } from "vitest";
import { router } from "./router";

describe("router contract", () => {
  it("keeps the five primary entrances reachable", () => {
    expect(router.resolve("/overview").name).toBe("overview");
    expect(router.resolve("/assets").name).toBe("assets");
    expect(router.resolve("/planning").name).toBe("planning");
    expect(router.resolve("/deliverables").name).toBe("deliverables");
    expect(router.resolve("/automation").name).toBe("automation");
  });

  it("keeps settings as a global tool route, not a primary entrance", () => {
    expect(router.resolve("/settings").name).toBe("settings");
    expect(router.resolve("/settings/security").name).toBe("settings");
  });

  it("does not resurrect legacy routes", () => {
    for (const path of ["/dashboard", "/discover", "/profile", "/dms", "/legal", "/report", "/heir", "/review"]) {
      expect(router.resolve(path).name, path).toBe("not-found");
    }
  });

  it("resolves detail and scaffold routes", () => {
    expect(router.resolve("/assets/ast_01").name).toBe("asset-detail");
    expect(router.resolve("/deliverables/rep_01").name).toBe("deliverable-detail");
    expect(router.resolve("/automation/runs/run_01").name).toBe("run-detail");
    expect(router.resolve("/automation/tools").name).toBe("tools");
    expect(router.resolve("/automation/approvals").name).toBe("approvals");
  });
});
