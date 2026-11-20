import { nextTick, type Component } from "vue";
import { mount, type VueWrapper } from "@vue/test-utils";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import AppShell from "./AppShell.vue";
import AgentDrawer from "./AgentDrawer.vue";
import CommandPalette from "./CommandPalette.vue";
import LockScreen from "./LockScreen.vue";
import { ui } from "@/platform/ui";

type AuditResult = {
  violations: Array<{ id: string; help: string }>;
};

const routes = [
  { path: "/overview", name: "overview", component: { template: "<div>Overview</div>" } },
  { path: "/assets", name: "assets", component: { template: "<div>Assets</div>" } },
  { path: "/settings/workspace", name: "settings", component: { template: "<div>Settings</div>" } },
  { path: "/:pathMatch(.*)*", name: "not-found", component: { template: "<div>Not found</div>" } },
];

function testRouter(): Router {
  return createRouter({ history: createMemoryHistory(), routes });
}

async function mountWithRouter(component: Component, router: Router, attach = true) {
  await router.push("/overview");
  await router.isReady();
  return mount(component, {
    attachTo: attach ? document.body : undefined,
    global: { plugins: [router] },
  });
}

async function audit(): Promise<AuditResult> {
  await nextTick();
  return axe.run(document.body, {
    runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"] },
    rules: { "color-contrast": { enabled: false } },
  });
}

function violations(result: AuditResult): string[] {
  return result.violations.map((violation) => `${violation.id}: ${violation.help}`);
}

let wrappers: VueWrapper[] = [];

beforeEach(() => {
  ui.agentOpen = false;
  ui.paletteOpen = false;
  ui.locked = false;
  document.body.innerHTML = "";
});

afterEach(() => {
  for (const wrapper of wrappers) wrapper.unmount();
  wrappers = [];
  document.body.innerHTML = "";
});

describe("WCAG AA axe gate", () => {
  it("keeps the app shell free of WCAG A/AA violations", async () => {
    const router = testRouter();
    const wrapper = await mountWithRouter(AppShell, router);
    wrappers.push(wrapper);
    const result = await audit();
    expect(violations(result)).toEqual([]);
  });

  it("keeps the command palette free of WCAG A/AA violations", async () => {
    const router = testRouter();
    ui.paletteOpen = true;
    const wrapper = await mountWithRouter(CommandPalette, router);
    wrappers.push(wrapper);
    const result = await audit();
    expect(violations(result)).toEqual([]);
  });

  it("keeps the agent drawer free of WCAG A/AA violations", async () => {
    const router = testRouter();
    ui.agentOpen = true;
    const wrapper = await mountWithRouter(AgentDrawer, router);
    wrappers.push(wrapper);
    const result = await audit();
    expect(violations(result)).toEqual([]);
  });

  it("keeps the locked console free of WCAG A/AA violations", async () => {
    const router = testRouter();
    ui.locked = true;
    const wrapper = await mountWithRouter(LockScreen, router);
    wrappers.push(wrapper);
    const result = await audit();
    expect(violations(result)).toEqual([]);
  });
});

describe("mobile navigation", () => {
  it("labels the skip target and keeps the closed rail out of modal semantics", async () => {
    const router = testRouter();
    const wrapper = await mountWithRouter(AppShell, router);
    wrappers.push(wrapper);
    expect(wrapper.find(".skip-link").text()).toBe("跳到主要内容");
    expect(wrapper.find(".rail").attributes("role")).toBeUndefined();
    expect(wrapper.find(".rail").attributes("aria-modal")).toBeUndefined();
  });

  it("acts as a modal drawer and restores focus to the menu button on Escape", async () => {
    const router = testRouter();
    const wrapper = await mountWithRouter(AppShell, router);
    wrappers.push(wrapper);
    const menu = wrapper.find(".menu-button");
    (menu.element as HTMLElement).focus();
    expect(menu.attributes("aria-expanded")).toBe("false");

    await menu.trigger("click");
    await nextTick();
    await nextTick();

    expect(menu.attributes("aria-expanded")).toBe("true");
    expect(wrapper.find(".rail").attributes("role")).toBe("dialog");
    expect(wrapper.find(".rail").attributes("aria-modal")).toBe("true");
    expect(wrapper.find(".frame").attributes("inert")).toBeDefined();
    expect(document.activeElement).toBe(wrapper.find(".rail-close").element);

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await nextTick();
    await nextTick();

    expect(menu.attributes("aria-expanded")).toBe("false");
    expect(wrapper.find(".rail").attributes("role")).toBeUndefined();
    expect(wrapper.find(".frame").attributes("inert")).toBeUndefined();
    expect(document.activeElement).toBe(menu.element);
  });
});
