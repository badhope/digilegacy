import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Asset, AssetsPage, Envelope } from "@/platform/types";
import { platformClient } from "@/platform/client";
import { clearQueryCache } from "@/platform/queryCache";
import AssetsView from "./AssetsView.vue";
import AssetDetailView from "./AssetDetailView.vue";

const asset: Asset = {
  id: "ast_01",
  name: "家庭照片与影像库",
  kind: "file",
  platform: "iCloud",
  status: "active",
  value: "2014–2024",
  valueStatus: "market",
  note: "",
  identifier: "",
  ownershipShare: "",
  beneficiary: "",
  beneficiaryContact: "",
  credentialLocation: "",
  expiresAt: "",
  quantity: "",
  unit: "",
  updatedAt: "2026-11-18T09:20:00+08:00",
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

function router() {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/assets", name: "assets", component: { template: "<div />" } },
      { path: "/assets/:id", name: "asset-detail", component: { template: "<div />" } },
      { path: "/policies", name: "policies", component: { template: "<div />" } },
      { path: "/:pathMatch(.*)*", name: "not-found", component: { template: "<div />" } },
    ],
  });
}

describe("asset lifecycle", () => {
  beforeEach(() => {
    clearQueryCache();
    window.sessionStorage.clear();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    clearQueryCache();
    vi.useRealTimers();
  });

  it("shows an add action when the asset list is truly empty", async () => {
    vi.spyOn(platformClient, "listAssets").mockResolvedValue(
      envelope<AssetsPage>({ items: [], nextCursor: null, total: 0 }),
    );
    const testRouter = router();
    await testRouter.push("/assets");
    await testRouter.isReady();

    const wrapper = mount(AssetsView, { global: { plugins: [testRouter] } });
    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();

    expect(wrapper.text()).toContain("还没有资产");
    await wrapper.find(".empty-action button").trigger("click");

    expect(wrapper.text()).toContain("新增数字资产");
  });

  it("archives an asset and exposes the restore action", async () => {
    const archived = { ...asset, status: "archived" as const };
    const update = vi.spyOn(platformClient, "updateAsset").mockResolvedValue(envelope(archived));
    const testRouter = router();
    await testRouter.push("/assets/ast_01");
    await testRouter.isReady();

    const wrapper = mount(AssetDetailView, { global: { plugins: [testRouter] } });
    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "归档资产")?.trigger("click");
    await flushPromises();

    expect(update).toHaveBeenCalledWith(expect.objectContaining({ status: "archived" }));
    expect(wrapper.text()).toContain("已归档，可随时恢复");
    expect(wrapper.text()).toContain("恢复使用");
  });

  it("filters the list by asset kind", async () => {
    const list = vi.spyOn(platformClient, "listAssets").mockResolvedValue(
      envelope<AssetsPage>({ items: [asset], nextCursor: null, total: 1 }),
    );
    const testRouter = router();
    await testRouter.push("/assets");
    await testRouter.isReady();

    const wrapper = mount(AssetsView, { global: { plugins: [testRouter] } });
    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();

    await wrapper.findAll("select")[1].setValue("real_estate");
    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();

    expect(list.mock.calls.at(-1)?.[0]).toMatchObject({ kind: "real_estate" });
  });

  it("permanently deletes an archived asset after confirmation", async () => {
    const archived = { ...asset, status: "archived" as const };
    vi.spyOn(platformClient, "getAsset").mockResolvedValue(envelope(archived));
    const remove = vi
      .spyOn(platformClient, "deleteAsset")
      .mockResolvedValue(envelope({ id: archived.id, deleted: true }));
    const testRouter = router();
    await testRouter.push("/assets/ast_01");
    await testRouter.isReady();

    const wrapper = mount(AssetDetailView, { global: { plugins: [testRouter] } });
    await vi.advanceTimersByTimeAsync(300);
    await flushPromises();

    await wrapper.findAll("button").find((button) => button.text() === "删除")?.trigger("click");
    expect(wrapper.find(".delete-confirm").exists()).toBe(true);

    await wrapper.findAll("button").find((button) => button.text() === "确认永久删除")?.trigger("click");
    await flushPromises();

    expect(remove).toHaveBeenCalledWith("ast_01", true);
    expect(testRouter.currentRoute.value.path).toBe("/assets");
  });
});
