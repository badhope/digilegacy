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
    path: "/deliverables/:id",
    name: "deliverable-detail",
    component: () => import("@/features/scaffold/ScaffoldView.vue"),
    meta: {
      titleKey: "deliverables.title",
      eyebrowKey: "deliverables.eyebrow",
      descriptionKey: "deliverables.description",
      readinessKey: "deliverables.readiness",
      modules: ["DOM-05", "RND-02"],
      commands: ["deliver.export"],
    },
  },
  {
    path: "/automation",
    name: "automation",
    component: () => import("@/features/automation/AutomationView.vue"),
    meta: { titleKey: "nav.automation" },
  },
  {
    path: "/automation/runs/:id",
    name: "run-detail",
    component: () => import("@/features/scaffold/ScaffoldView.vue"),
    meta: {
      titleKey: "automation.runs.title",
      eyebrowKey: "automation.eyebrow",
      descriptionKey: "automation.description",
      readinessKey: "feature.automation.readiness",
      modules: ["AGT-01", "WEB-09"],
      commands: ["agent.runs.list", "agent.approvals.decide"],
    },
  },
  {
    path: "/automation/tools",
    name: "tools",
    component: () => import("@/features/scaffold/ScaffoldView.vue"),
    meta: {
      titleKey: "automation.tools.title",
      eyebrowKey: "automation.eyebrow",
      descriptionKey: "automation.tools.subtitle",
      readinessKey: "feature.automation.readiness",
      modules: ["AGT-02", "IFC-03"],
      commands: ["assets.list", "assets.get", "plan.get", "agent.runs.list"],
    },
  },
  {
    path: "/automation/approvals",
    name: "approvals",
    component: () => import("@/features/automation/ApprovalsView.vue"),
    meta: {
      titleKey: "automation.approvals.title",
      eyebrowKey: "automation.eyebrow",
      descriptionKey: "automation.approvals.subtitle",
      modules: ["AGT-03", "WEB-09"],
      commands: ["agent.approvals.list", "agent.approvals.decide"],
    },
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

export const router = createRouter({
  history: createWebHistory(),
  routes,
  scrollBehavior: () => ({ top: 0 }),
});
