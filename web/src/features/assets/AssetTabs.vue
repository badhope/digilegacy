<script setup lang="ts">
import { useRoute } from "vue-router";
import { useI18n } from "@/platform/i18n";

const route = useRoute();
const { t } = useI18n();
const tabs = [
  { path: "/assets", key: "assets.tab.assets" },
  { path: "/policies", key: "assets.tab.policies" },
];
const isActive = (path: string) => route.path === path || route.path.startsWith(`${path}/`);
</script>

<template>
  <nav class="asset-tabs" aria-label="Assets sections">
    <RouterLink
      v-for="tab in tabs"
      :key="tab.path"
      :to="tab.path"
      class="asset-tab"
      :class="{ active: isActive(tab.path) }"
    >
      {{ t(tab.key) }}
    </RouterLink>
  </nav>
</template>

<style scoped>
.asset-tabs {
  display: inline-flex;
  gap: 4px;
  margin-bottom: 16px;
  padding: 3px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface);
}
.asset-tab {
  padding: 7px 14px;
  border-radius: calc(var(--radius-sm) - 2px);
  color: var(--muted);
  font-size: 11px;
  text-decoration: none;
}
.asset-tab:hover { color: var(--ink); }
.asset-tab.active { background: var(--accent-soft); color: var(--accent-strong); font-weight: 600; }
</style>
