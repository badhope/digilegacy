import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createMemoryHistory, createRouter, type Router } from "vue-router";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Component } from "vue";
import OverviewView from "@/features/overview/OverviewView.vue";
import AssetsView from "@/features/assets/AssetsView.vue";
import PoliciesView from "@/features/policies/PoliciesView.vue";
import AssetsDiscoverView from "@/features/assets/AssetsDiscoverView.vue";
import AssetDetailView from "@/features/assets/AssetDetailView.vue";
import PlanningView from "@/features/planning/PlanningView.vue";
import DeliverablesView from "@/features/deliverables/DeliverablesView.vue";
import AutomationView from "@/features/automation/AutomationView.vue";
import RunStartView from "@/features/automation/RunStartView.vue";
import RunDetailView from "@/features/automation/RunDetailView.vue";
import ToolsView from "@/features/automation/ToolsView.vue";
import ApprovalsView from "@/features/automation/ApprovalsView.vue";
import SettingsView from "@/features/settings/SettingsView.vue";
import NotFoundView from "@/features/notfound/NotFoundView.vue";

type AuditResult = {
  violations: Array<{ id: string; help: string }>;
};

const stub = { template: "<div />" };

const routes = [
  { path: "/overview", name: "overview", component: stub },
  { path: "/assets", name: "assets", component: stub },
  { path: "/policies", name: "policies", component: stub },
  { path: "/assets/discover", name: "asset-discover", component: stub },
  { path: "/assets/:id", name: "asset-detail", component: stub },
  { path: "/planning", name: "planning", component: stub },
  { path: "/deliverables", name: "deliverables", component: stub },
  { path: "/automation", name: "automation", component: stub },
  { path: "/automation/runs/new", name: "run-start", component: stub },
  { path: "/automation/runs/:id", name: "run-detail", component: stub },
  { path: "/automation/tools", name: "tools", component: stub },
  { path: "/automation/approvals", name: "approvals", component: stub },
  { path: "/settings/:section?", name: "settings", component: stub },
  { path: "/:pathMatch(.*)*", name: "not-found", component: stub },
];

function testRouter(): Router {
  return createRouter({ history: createMemoryHistory(), routes });
}

async function render(component: Component, path: string) {
  const router = testRouter();
  await router.push(path);
  await router.isReady();
  vi.useFakeTimers();
  const wrapper = mount(component, {
    attachTo: document.body,
    global: { plugins: [router] },
  });
  await vi.advanceTimersByTimeAsync(1200);
  await flushPromises();
  vi.useRealTimers();
  return wrapper;
}

async function audit(): Promise<AuditResult> {
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
  document.body.innerHTML = "";
});

afterEach(() => {
  for (const wrapper of wrappers) wrapper.unmount();
  wrappers = [];
  vi.useRealTimers();
  document.body.innerHTML = "";
});

const cases: Array<[string, Component, string]> = [
  ["overview", OverviewView, "/overview"],
  ["assets", AssetsView, "/assets"],
  ["policies", PoliciesView, "/policies"],
  ["asset discover", AssetsDiscoverView, "/assets/discover"],
  ["asset detail", AssetDetailView, "/assets/ast_01"],
  ["planning", PlanningView, "/planning"],
  ["deliverables", DeliverablesView, "/deliverables"],
  ["automation", AutomationView, "/automation"],
  ["run start", RunStartView, "/automation/runs/new"],
  ["run detail", RunDetailView, "/automation/runs/run_01"],
  ["tools", ToolsView, "/automation/tools"],
  ["approvals", ApprovalsView, "/automation/approvals"],
  ["settings workspace", SettingsView, "/settings/workspace"],
  ["settings security", SettingsView, "/settings/security"],
  ["not found", NotFoundView, "/missing"],
];

describe("route WCAG AA axe gate", () => {
  it.each(cases)("%s", async (_name, component, path) => {
    const wrapper = await render(component, path);
    wrappers.push(wrapper);
    const result = await audit();
    expect(violations(result)).toEqual([]);
  });
});
