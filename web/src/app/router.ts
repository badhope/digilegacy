import { createRouter, createWebHistory, type RouteRecordRaw } from "vue-router";

declare module "vue-router" {
  interface RouteMeta {
    titleKey?: string;
    eyebrowKey?: string;
    descriptionKey?: string;
    readinessKey?: string;
    modules?: string[];
    commands?: string[];
  }
}

const routes: RouteRecordRaw[] = [
  { path: "/", redirect: "/overview" },
  {
    path: "/overview",
    name: "overview",
    component: () => import("@/features/overview/OverviewView.vue"),
    meta: { titleKey: "nav.overview" },
  },
  {
    path: "/assets",
    name: "assets",
    component: () => import("@/features/assets/AssetsView.vue"),
    meta: { titleKey: "nav.assets" },
  },
  {
    path: "/policies",
    name: "policies",
    component: () => import("@/features/policies/PoliciesView.vue"),
    meta: { titleKey: "nav.policies" },
  },
  {
    path: "/assets/discover",
    name: "asset-discover",
    component: () => import("@/features/assets/AssetsDiscoverView.vue"),
    meta: { titleKey: "assets.discover.title" },
  },
  {
    path: "/assets/:id",
    name: "asset-detail",
    component: () => import("@/features/assets/AssetDetailView.vue"),
    meta: { titleKey: "assets.title" },
  },
  {
    path: "/planning",
    name: "planning",
    component: () => import("@/features/planning/PlanningView.vue"),
    meta: { titleKey: "nav.planning" },
  },
  {
    path: "/deliverables",
    name: "deliverables",
    component: () => import("@/features/deliverables/DeliverablesView.vue"),
    meta: { titleKey: "nav.deliverables" },
  },
  {
    path: "/automation",
    name: "automation",
    component: () => import("@/features/automation/AutomationView.vue"),
    meta: { titleKey: "nav.automation" },
  },
  {
    path: "/automation/runs/new",
    name: "run-start",
    component: () => import("@/features/automation/RunStartView.vue"),
    meta: {
      titleKey: "runs.start.title",
      eyebrowKey: "automation.eyebrow",
      descriptionKey: "runs.start.subtitle",
      modules: ["AGT-01"],
      commands: ["agent.runs.start"],
    },
  },
  {
    path: "/automation/runs/:id",
    name: "run-detail",
    component: () => import("@/features/automation/RunDetailView.vue"),
    meta: {
      titleKey: "automation.runs.title",
      eyebrowKey: "automation.eyebrow",
      descriptionKey: "runs.detail.subtitle",
      modules: ["AGT-01", "WEB-09"],
      commands: ["agent.runs.get", "agent.approvals.list"],
    },
  },
  {
    path: "/automation/tools",
    name: "tools",
    redirect: { path: "/automation", query: { tab: "tools" } },
  },
  {
    path: "/automation/approvals",
    name: "approvals",
    redirect: { path: "/automation", query: { tab: "approvals" } },
  },
  {
    path: "/settings/workspace",
    redirect: "/settings/connections",
  },
  {
    path: "/settings/appearance",
    redirect: "/settings/connections",
  },
  {
    path: "/settings/data",
    redirect: "/settings/security",
  },
  {
    path: "/settings/about",
    redirect: "/settings/connections",
  },
  {
    path: "/settings/audit",
    redirect: "/settings/security",
  },
  {
    path: "/settings/:section?",
    name: "settings",
    component: () => import("@/features/settings/SettingsView.vue"),
    meta: { titleKey: "nav.settings" },
  },
  {
    path: "/:pathMatch(.*)*",
    name: "not-found",
    component: () => import("@/features/notfound/NotFoundView.vue"),
    meta: { titleKey: "notfound.title" },
  },
];

export function prefetchRoute(name: string): void {
  const record = routes.find((route) => route.name === name);
  const component = record?.component;
  if (typeof component === "function") {
    void (component as () => Promise<unknown>)().catch(() => undefined);
  }
}

export const router = createRouter({
  history: createWebHistory(),
  routes,
  scrollBehavior: () => ({ top: 0 }),
});
