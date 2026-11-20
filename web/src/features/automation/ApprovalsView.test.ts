import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ApprovalPage, Envelope } from "@/platform/types";
import { platformClient } from "@/platform/client";
import { clearQueryCache } from "@/platform/queryCache";
import ApprovalsView from "./ApprovalsView.vue";

const approval = {
  id: "apr_demo_01",
  runId: "run_01",
  tool: "assets.update",
  objectKey: "ast_01",
  diff: '{"before":"active","after":"archived"}',
  status: "pending" as const,
  reason: "",
  createdAt: "2026-11-19T09:31:00+08:00",
  decidedAt: "",
};

function envelope<T>(data: T): Envelope<T> {
  return {
    success: true,
    code: "OK",
    message: "mock",
    data,
    details: null,
    traceId: "test",
  };
}

describe("ApprovalsView", () => {
  beforeEach(() => {
    clearQueryCache();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    clearQueryCache();
  });

  it("blocks a rejection without a reason before calling the API", async () => {
    vi.spyOn(platformClient, "listApprovals").mockResolvedValue(
      envelope<ApprovalPage>({ items: [approval], pending: 1 }),
    );
    const decide = vi.spyOn(platformClient, "decideApproval");
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/automation", name: "automation", component: { template: "<div />" } },
        { path: "/automation/runs/:id", name: "run-detail", component: { template: "<div />" } },
      ],
    });
    await router.push("/automation");
    await router.isReady();

    const wrapper = mount(ApprovalsView, { global: { plugins: [router] } });
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "拒绝")?.trigger("click");
    await flushPromises();

    expect(decide).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("拒绝审批时必须填写理由");
  });
});
