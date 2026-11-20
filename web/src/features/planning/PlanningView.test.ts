import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Envelope, NotificationPage, Plan, TriggerEventPage } from "@/platform/types";
import { platformClient } from "@/platform/client";
import { clearQueryCache } from "@/platform/queryCache";
import PlanningView from "./PlanningView.vue";

const plan: Plan = {
  holder: { name: "林知远", updatedAt: "2026-11-18T09:00:00+08:00" },
  heirs: [],
  trigger: {
    mode: "dms",
    days: 30,
    status: "armed",
    lastCheckIn: "2026-11-01T00:00:00+08:00",
    nextCheckAt: "2026-12-01T00:00:00+08:00",
    pausedUntil: "",
    triggeredAt: "",
    stage: 0,
  },
};

function envelope<T>(data: T): Envelope<T> {
  return { success: true, code: "OK", message: "mock", data, details: null, traceId: "test" };
}

function router() {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/planning", name: "planning", component: { template: "<div />" } },
      { path: "/deliverables", name: "deliverables", component: { template: "<div />" } },
      { path: "/:pathMatch(.*)*", name: "not-found", component: { template: "<div />" } },
    ],
  });
}

async function mountView() {
  const testRouter = router();
  await testRouter.push("/planning");
  await testRouter.isReady();
  const wrapper = mount(PlanningView, { global: { plugins: [testRouter] } });
  await vi.advanceTimersByTimeAsync(300);
  await flushPromises();
  return wrapper;
}

describe("planning CRUD", () => {
  beforeEach(() => {
    clearQueryCache();
    vi.useFakeTimers();
    vi.spyOn(platformClient, "getPlan").mockResolvedValue(envelope(plan));
    vi.spyOn(platformClient, "triggerEvents").mockResolvedValue(
      envelope<TriggerEventPage>({ items: [], nextCursor: null, total: 0 }),
    );
    vi.spyOn(platformClient, "listNotifications").mockResolvedValue(
      envelope<NotificationPage>({ items: [], nextCursor: null, total: 0 }),
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
    clearQueryCache();
    vi.useRealTimers();
  });

  it("saves the holder name from the inline form", async () => {
    const update = vi.spyOn(platformClient, "setPlanProfile").mockResolvedValue(
      envelope({ ...plan, holder: { ...plan.holder, name: "林知远（新）" } }),
    );
    const wrapper = await mountView();

    await wrapper.findAll("button").find((button) => button.text() === "编辑")?.trigger("click");
    await wrapper.find(".edit-form input").setValue("林知远（新）");
    await wrapper.find(".edit-form .form-actions button").trigger("click");
    await flushPromises();

    expect(update).toHaveBeenCalledWith("林知远（新）");
    expect(wrapper.text()).toContain("持有人已保存");
    expect(wrapper.text()).toContain("林知远（新）");
  });

  it("adds and saves a new heir", async () => {
    const update = vi.spyOn(platformClient, "setPlanHeirs").mockResolvedValue(
      envelope({
        ...plan,
        heirs: [{ id: "heir_01", name: "林若岚", role: "executor" }],
      }),
    );
    const wrapper = await mountView();

    await wrapper.findAll("button").find((button) => button.text() === "添加继承人")?.trigger("click");
    const inputs = wrapper.findAll(".heir-edit-row .form-field input");
    await inputs[0].setValue("林若岚");
    await inputs[1].setValue("配偶");
    await wrapper.find(".heir-edit-form .executor-toggle input").setValue(true);
    await wrapper.find(".heir-edit-form .form-actions button").trigger("click");
    await flushPromises();

    expect(update).toHaveBeenCalledWith([
      { name: "林若岚", role: "配偶", isExecutor: true },
    ]);
    expect(wrapper.text()).toContain("继承人已保存");
    expect(wrapper.text()).toContain("林若岚");
  });
});
