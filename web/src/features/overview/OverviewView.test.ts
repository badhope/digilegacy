import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";
import OverviewView from "./OverviewView.vue";

const router = createRouter({
  history: createMemoryHistory(),
  routes: [
    { path: "/overview", name: "overview", component: { template: "<div />" } },
    { path: "/:pathMatch(.*)*", name: "not-found", component: { template: "<div />" } },
  ],
});

describe("OverviewView", () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    await router.push("/overview");
    await router.isReady();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the workspace overview from the mock contract", async () => {
    const wrapper = mount(OverviewView, { global: { plugins: [router] } });
    expect(wrapper.text()).toContain("PLATFORM / OVERVIEW");

    await vi.advanceTimersByTimeAsync(400);
    await flushPromises();

    expect(wrapper.text()).toContain("林家数字遗产");
    expect(wrapper.text()).toContain("已登记资产");
    expect(wrapper.text()).toContain("18");
    expect(wrapper.text()).toContain("62");
  });

  it("uses a skeleton instead of a full-page spinner while loading", async () => {
    const wrapper = mount(OverviewView, { global: { plugins: [router] } });
    expect(wrapper.findAll(".skeleton").length).toBeGreaterThan(0);
    expect(wrapper.text()).toContain("PLATFORM / OVERVIEW");
  });
});
