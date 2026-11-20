import { flushPromises, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import LockScreen from "./LockScreen.vue";
import { ui } from "@/platform/ui";

const unlockVault = vi.hoisted(() => vi.fn());
vi.mock("@/platform/securityUnlock", () => ({ unlockVault }));

async function submit(password: string) {
  const input = document.body.querySelector<HTMLInputElement>("input[type=password]");
  expect(input).not.toBeNull();
  input!.value = password;
  input!.dispatchEvent(new Event("input"));
  await nextTick();
  document.body.querySelector("form")!.dispatchEvent(new Event("submit"));
  await flushPromises();
}

let wrapper: ReturnType<typeof mount> | null = null;

beforeEach(() => {
  ui.locked = true;
  ui.securityReady = true;
  ui.securityConfigured = true;
  unlockVault.mockReset();
  document.body.innerHTML = "";
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = null;
  document.body.innerHTML = "";
});

describe("LockScreen", () => {
  it("stays locked and explains a wrong password", async () => {
    unlockVault.mockResolvedValue({ success: false, code: "INVALID_ARGS", message: "主密码不正确" });
    wrapper = mount(LockScreen, { attachTo: document.body });
    await submit("wrong-password");
    expect(ui.locked).toBe(true);
    expect(document.body.textContent).toContain("主密码不正确");
    expect(unlockVault).toHaveBeenCalledWith("wrong-password");
    expect(document.activeElement).toBe(document.body.querySelector("input[type=password]"));
  });

  it("only unlocks after the server accepts the password and moves focus to main", async () => {
    unlockVault.mockResolvedValue({ success: true, code: "OK", data: { locked: false } });
    const opener = document.createElement("button");
    const main = document.createElement("main");
    main.id = "main";
    main.tabIndex = -1;
    document.body.append(opener, main);
    opener.focus();
    wrapper = mount(LockScreen, { attachTo: document.body });
    await submit("correct horse battery");
    expect(ui.locked).toBe(false);
    await nextTick();
    expect(document.activeElement).toBe(main);
  });
});
