import { describe, expect, it } from "vitest";
import { router } from "./router";

describe("router contract", () => {
  it("keeps the five primary entrances reachable", () => {
    expect(router.resolve("/overview").name).toBe("overview");
    expect(router.resolve("/assets").name).toBe("assets");
    expect(router.resolve("/policies").name).toBe("policies");
    expect(router.resolve("/planning").name).toBe("planning");
    expect(router.resolve("/deliverables").name).toBe("deliverables");
    expect(router.resolve("/automation").name).toBe("automation");
  });

  it("keeps settings as a global tool route, not a primary entrance", () => {
    expect(router.resolve("/settings").name).toBe("settings");
    expect(router.resolve("/settings/security").name).toBe("settings");
  });

  it("redirects retired settings sections to the two remaining ones", async () => {
    for (const [legacy, target] of [
      ["/settings/workspace", "/settings/connections"],
      ["/settings/appearance", "/settings/connections"],
      ["/settings/about", "/settings/connections"],
      ["/settings/data", "/settings/security"],
      ["/settings/audit", "/settings/security"],
    ] as const) {
      await router.push(legacy);
      expect(router.currentRoute.value.fullPath, legacy).toBe(target);
      expect(router.currentRoute.value.name, legacy).toBe("settings");
    }
  });

  it("does not resurrect legacy routes", () => {
    for (const path of ["/dashboard", "/discover", "/profile", "/dms", "/legal", "/report", "/heir", "/review", "/deliverables/rep_01"]) {
      expect(router.resolve(path).name, path).toBe("not-found");
    }
  });

  it("resolves implemented detail routes", () => {
    expect(router.resolve("/assets/discover").name).toBe("asset-discover");
    expect(router.resolve("/assets/ast_01").name).toBe("asset-detail");
    expect(router.resolve("/automation/runs/run_01").name).toBe("run-detail");
    expect(router.resolve("/automation/tools").name).toBe("tools");
    expect(router.resolve("/automation/approvals").name).toBe("approvals");
  });

  it("keeps automation tools and approvals as deep-link aliases", async () => {
    await router.push("/automation/tools");
    expect(router.currentRoute.value.name).toBe("automation");
    expect(router.currentRoute.value.query.tab).toBe("tools");

    await router.push("/automation/approvals");
    expect(router.currentRoute.value.name).toBe("automation");
    expect(router.currentRoute.value.query.tab).toBe("approvals");
  });
});
