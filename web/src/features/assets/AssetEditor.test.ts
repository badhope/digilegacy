import { mount, type VueWrapper } from "@vue/test-utils";
import { beforeEach, describe, expect, it } from "vitest";
import AssetEditor from "./AssetEditor.vue";
import { draftFromAsset } from "./model";
import { readAssetDraft, writeAssetDraft } from "./draftStorage";

function buttonByText(wrapper: VueWrapper, text: string) {
  return wrapper.findAll("button").find((button) => button.text().includes(text));
}

describe("AssetEditor", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
  });

  it("submits deep fields and supports save-and-continue", async () => {
    const wrapper = mount(AssetEditor, { props: { mode: "create" } });
    await wrapper.find(".more-toggle").trigger("click");

    const inputs = wrapper.findAll("input");
    await inputs[0].setValue("鼓浪屿老宅");
    await inputs[1].setValue("不动产登记中心");
    await inputs[2].setValue("¥3,200");
    await inputs[3].setValue("闽(2026)第0001号");
    await inputs[4].setValue("50%");
    await inputs[5].setValue("林小雨");
    await inputs[6].setValue("lin@example.com");
    await inputs[7].setValue("保险柜 A-03");
    await inputs[8].setValue("2056-12-31");
    await inputs[9].setValue("1");
    await inputs[10].setValue("套");
    await wrapper.findAll("select")[0].setValue("real_estate");

    await buttonByText(wrapper, "保存并继续")?.trigger("click");
    const submitted = wrapper.emitted("submit")?.[0] as unknown[] | undefined;
    expect(submitted?.[0]).toMatchObject({
      name: "鼓浪屿老宅",
      kind: "real_estate",
      platform: "不动产登记中心",
      value: "3200",
      valueStatus: "market",
      identifier: "闽(2026)第0001号",
      ownershipShare: "50%",
      beneficiary: "林小雨",
      beneficiaryContact: "lin@example.com",
      credentialLocation: "保险柜 A-03",
      expiresAt: "2056-12-31",
      quantity: "1",
      unit: "套",
    });
    expect(submitted?.[1]).toEqual({ continue: true });
  });

  it("clears the amount when the value status is not assessed", async () => {
    const wrapper = mount(AssetEditor, { props: { mode: "create" } });
    const inputs = wrapper.findAll("input");
    await inputs[0].setValue("老家宅基地");
    await inputs[2].setValue("50万");
    await wrapper.findAll("select")[1].setValue("unknown");
    expect(inputs[2].attributes("disabled")).toBeDefined();
    await buttonByText(wrapper, "保存")?.trigger("click");
    const submitted = wrapper.emitted("submit")?.[0] as unknown[] | undefined;
    expect(submitted?.[0]).toMatchObject({ value: "", valueStatus: "unknown" });
  });

  it("restores an unsaved draft after leaving for settings", async () => {
    writeAssetDraft({
      ...draftFromAsset(null),
      name: "待填的银行卡",
      kind: "financial",
      platform: "招商银行",
    });
    const wrapper = mount(AssetEditor, { props: { mode: "create" } });
    await wrapper.vm.$nextTick();

    expect(wrapper.find(".draft-note").text()).toContain("已恢复上次未保存的草稿");
    expect((wrapper.findAll("input")[0].element as HTMLInputElement).value).toBe("待填的银行卡");
  });

  it("keeps the type and valuation status when resetting for the next entry", async () => {
    const wrapper = mount(AssetEditor, { props: { mode: "create" } });
    await wrapper.findAll("input")[0].setValue("家庭寿险保单");
    await wrapper.findAll("select")[0].setValue("financial");
    await wrapper.findAll("select")[1].setValue("appraised");

    await (wrapper.vm as unknown as { resetDraft: (previous: ReturnType<typeof draftFromAsset>) => void }).resetDraft({
      ...draftFromAsset(null),
      kind: "financial",
      valueStatus: "appraised",
    });

    const inputs = wrapper.findAll("input");
    expect((inputs[0].element as HTMLInputElement).value).toBe("");
    expect((wrapper.findAll("select")[0].element as HTMLSelectElement).value).toBe("financial");
    expect((wrapper.findAll("select")[1].element as HTMLSelectElement).value).toBe("appraised");
  });

  it("warns before discarding a dirty draft", async () => {
    const wrapper = mount(AssetEditor, { props: { mode: "create" } });
    await wrapper.findAll("input")[0].setValue("还没填完");

    await wrapper.find(".close-button").trigger("click");
    expect(wrapper.find(".close-confirm").exists()).toBe(true);

    await buttonByText(wrapper, "放弃草稿")?.trigger("click");
    expect(wrapper.emitted("cancel")).toHaveLength(1);
    expect(readAssetDraft()).toBeNull();
  });
});
