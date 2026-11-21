import { mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it } from "vitest";
import AssetTabs from "./AssetTabs.vue";

const router = createRouter({
  history: createMemoryHistory(),
  routes: [
    { path: "/assets", name: "assets", component: { template: "<div />" } },
    { path: "/assets/discover", name: "asset-discover", component: { template: "<div />" } },
    { path: "/assets/:id", name: "asset-detail", component: { template: "<div />" } },
    { path: "/policies", name: "policies", component: { template: "<div />" } },
  ],
});

async function mountAt(path: string) {
  await router.push(path);
  await router.isReady();
  return mount(AssetTabs, { global: { plugins: [router] } });
}

function activeLabels(wrapper: ReturnType<typeof mount>): string[] {
  return wrapper.findAll(".asset-tab.active").map((tab) => tab.text());
}

describe("AssetTabs", () => {
  it("keeps the list tab active on asset detail pages", async () => {
    const wrapper = await mountAt("/assets/ast_01");
    expect(activeLabels(wrapper)).toEqual(["资产清单"]);
  });

  it("moves the active state to bulk add on its own route", async () => {
    const wrapper = await mountAt("/assets/discover");
    expect(activeLabels(wrapper)).toEqual(["批量添加"]);
  });

  it("keeps policies independent from the asset list", async () => {
    const wrapper = await mountAt("/policies");
    expect(activeLabels(wrapper)).toEqual(["平台政策"]);
  });
});
