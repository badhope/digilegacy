<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import AppIcon from "@/shared/icons/AppIcon.vue";
import type { IconName } from "@/shared/icons/paths";
import AgentDrawer from "./AgentDrawer.vue";
import CommandPalette from "./CommandPalette.vue";
import LockScreen from "./LockScreen.vue";
import { useI18n } from "@/platform/i18n";
import { useTheme } from "@/platform/theme";
import { lockConsole, openAgent, openPalette } from "@/platform/ui";

const route = useRoute();
const router = useRouter();
const { t, locale, toggleLocale } = useI18n();
const { theme, toggleTheme } = useTheme();
const mobileOpen = ref(false);

const navItems: Array<{ path: string; labelKey: string; icon: IconName }> = [
  { path: "/overview", labelKey: "nav.overview", icon: "overview" },
  { path: "/assets", labelKey: "nav.assets", icon: "assets" },
  { path: "/planning", labelKey: "nav.planning", icon: "planning" },
  { path: "/deliverables", labelKey: "nav.deliverables", icon: "deliverables" },
  { path: "/automation", labelKey: "nav.automation", icon: "automation" },
];

const currentTitle = computed(() => t(String(route.meta.titleKey ?? "nav.overview")));

function isActive(path: string) {
  if (path === "/overview") return route.path === "/overview";
  if (path === "/assets") return route.path.startsWith("/assets") || route.path === "/policies";
  return route.path.startsWith(path);
}

function go(path: string) {
  mobileOpen.value = false;
  router.push(path);
}

function onKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    openPalette();
  }
}

watch(
  () => route.fullPath,
  () => {
    mobileOpen.value = false;
  },
);

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div class="shell">
    <a class="skip-link" href="#main">{{ t("nav.overview") }}</a>

    <aside class="rail" :class="{ open: mobileOpen }" aria-label="Primary">
      <div class="brand">
        <span class="brand-mark">DL</span>
        <span class="brand-copy">
          <strong>{{ t("app.name") }}</strong>
          <small>{{ t("app.tagline") }}</small>
        </span>
      </div>

      <div class="rail-section">
        <span class="rail-label">WORKSPACE</span>
        <nav class="rail-nav">
          <button
            v-for="item in navItems"
            :key="item.path"
            type="button"
            class="rail-item"
            :class="{ active: isActive(item.path) }"
            :aria-current="isActive(item.path) ? 'page' : undefined"
            @click="go(item.path)"
          >
            <span class="rail-icon"><AppIcon :name="item.icon" :size="17" /></span>
            <span>{{ t(item.labelKey) }}</span>
            <span v-if="isActive(item.path)" class="active-mark" />
          </button>
        </nav>
      </div>

      <div class="rail-bottom">
        <button type="button" class="workspace-card" @click="go('/settings/workspace')">
          <span class="workspace-dot" />
          <span>
            <strong>{{ t("app.workspace") }}</strong>
            <small>{{ t("app.workspace.note") }}</small>
          </span>
          <AppIcon name="chevron-right" :size="14" />
        </button>

        <button type="button" class="agent-rail" @click="openAgent()">
          <span class="agent-rail-icon"><AppIcon name="agent" :size="16" /></span>
          <span>
            <strong>{{ t("topbar.agent") }}</strong>
            <small>{{ t("overview.agent.online") }} · {{ t("overview.agent.readonly") }}</small>
          </span>
          <span class="online-dot" />
        </button>

        <div class="rail-meta mono">
          <span>v0.7.6</span>
          <span>M3 / LIVE</span>
        </div>
      </div>
    </aside>

    <div v-if="mobileOpen" class="rail-scrim" @click="mobileOpen = false" />

    <div class="frame">
      <header class="topbar">
        <div class="topbar-left">
          <button class="tool-button menu-button" type="button" :aria-label="t('topbar.menu')" @click="mobileOpen = true">
            <AppIcon name="menu" :size="18" />
          </button>
          <div class="breadcrumb">
            <span class="mono">DIGILEGACY</span>
            <span class="breadcrumb-slash">/</span>
            <strong>{{ currentTitle }}</strong>
          </div>
        </div>

        <div class="topbar-tools">
          <button class="search-trigger" type="button" :aria-label="t('topbar.search')" @click="openPalette()">
            <AppIcon name="search" :size="15" />
            <span>{{ t("topbar.search") }}</span>
            <kbd class="mono">⌘K</kbd>
          </button>

          <button class="tool-button agent-trigger" type="button" :aria-label="t('topbar.agent')" @click="openAgent()">
            <AppIcon name="agent" :size="17" />
            <span class="tool-label">{{ t("topbar.agent") }}</span>
            <span class="online-dot" />
          </button>

          <span class="tool-divider" />

          <button class="tool-button" type="button" :aria-label="t('topbar.theme')" @click="toggleTheme()">
            <AppIcon :name="theme === 'light' ? 'moon' : 'sun'" :size="17" />
          </button>

          <button class="locale-button mono" type="button" :aria-label="t('topbar.language')" @click="toggleLocale()">
            {{ locale === "zh-CN" ? "EN" : "中" }}
          </button>

          <button class="tool-button" type="button" :aria-label="t('topbar.lock')" @click="lockConsole()">
            <AppIcon name="lock" :size="16" />
          </button>

          <button class="tool-button" type="button" :aria-label="t('topbar.settings')" @click="go('/settings/workspace')">
            <AppIcon name="settings" :size="17" />
          </button>
        </div>
      </header>

      <main id="main" class="content">
        <RouterView />
      </main>
    </div>

    <AgentDrawer />
    <CommandPalette />
    <LockScreen />
  </div>
</template>

<style scoped>
.shell {
  display: grid;
  grid-template-columns: var(--rail-w) minmax(0, 1fr);
  min-height: 100vh;
}

.skip-link {
  position: fixed;
  top: 12px;
  left: 12px;
  z-index: 100;
  padding: 8px 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--ink);
  transform: translateY(-120%);
}

.skip-link:focus {
  transform: translateY(0);
}

.rail {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  width: var(--rail-w);
  height: 100vh;
  padding: 26px 18px 18px;
  overflow: hidden;
  color: #e9eee3;
  background:
    radial-gradient(circle at 10% 0, rgba(145, 173, 134, 0.16), transparent 18rem),
    #1b211c;
  border-right: 1px solid rgba(255, 255, 255, 0.06);
}

.brand {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 0 8px 32px;
}

.brand-mark {
  display: grid;
  width: 34px;
  height: 34px;
  border: 1px solid rgba(233, 238, 227, 0.22);
  border-radius: 8px;
  color: #dce7d3;
  font-family: var(--font-display);
  font-size: 13px;
  letter-spacing: -0.03em;
  place-items: center;
  background: rgba(255, 255, 255, 0.06);
}

.brand-copy {
  display: flex;
  flex-direction: column;
}

.brand-copy strong {
  color: #f2f5ec;
  font-family: var(--font-display);
  font-size: 17px;
  font-weight: 500;
  letter-spacing: -0.02em;
}

.brand-copy small {
  margin-top: 1px;
  color: rgba(233, 238, 227, 0.5);
  font-family: var(--font-mono);
  font-size: 8px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.rail-section {
  flex: 1;
}

.rail-label {
  display: block;
  padding: 0 10px 9px;
  color: rgba(233, 238, 227, 0.38);
  font-family: var(--font-mono);
  font-size: 8px;
  letter-spacing: 0.14em;
}

.rail-nav {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.rail-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 10px 10px;
  border-radius: 8px;
  color: rgba(233, 238, 227, 0.62);
  font-size: 12px;
  text-align: left;
  transition:
    color var(--fast) var(--ease),
    background var(--fast) var(--ease);
}

.rail-item:hover {
  background: rgba(255, 255, 255, 0.055);
  color: #edf2e9;
}

.rail-item.active {
  background: rgba(145, 173, 134, 0.16);
  color: #f2f6ed;
  font-weight: 600;
}

.rail-icon {
  display: grid;
  width: 20px;
  height: 20px;
  color: rgba(233, 238, 227, 0.5);
  place-items: center;
}

.rail-item.active .rail-icon {
  color: #b5cba8;
}

.active-mark {
  width: 5px;
  height: 5px;
  margin-left: auto;
  border-radius: 50%;
  background: #b5cba8;
}

.rail-bottom {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-top: 16px;
}

.workspace-card,
.agent-rail {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 9px;
  text-align: left;
  background: rgba(255, 255, 255, 0.035);
  transition: background var(--fast) var(--ease);
}

.workspace-card:hover,
.agent-rail:hover {
  background: rgba(255, 255, 255, 0.07);
}

.workspace-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #b5cba8;
}

.workspace-card > span:nth-child(2),
.agent-rail > span:nth-child(2) {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.workspace-card strong,
.agent-rail strong {
  color: #edf2e9;
  font-size: 11px;
  font-weight: 500;
}

.workspace-card small,
.agent-rail small {
  margin-top: 2px;
  overflow: hidden;
  color: rgba(233, 238, 227, 0.45);
  font-size: 8px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.workspace-card > :last-child {
  color: rgba(233, 238, 227, 0.32);
}

.agent-rail {
  border-color: rgba(145, 173, 134, 0.24);
  background: rgba(145, 173, 134, 0.09);
}

.agent-rail-icon {
  display: grid;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  color: #c6d9ba;
  place-items: center;
  background: rgba(145, 173, 134, 0.18);
}

.online-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #a9c49b;
  box-shadow: 0 0 0 3px rgba(169, 196, 155, 0.16);
}

.rail-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 2px 0;
  color: rgba(233, 238, 227, 0.3);
  font-size: 7px;
  letter-spacing: 0.08em;
}

.frame {
  min-width: 0;
}

.topbar {
  position: sticky;
  top: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  height: var(--topbar-h);
  padding: 0 var(--gutter);
  border-bottom: 1px solid var(--line);
  background: color-mix(in srgb, var(--bg) 82%, transparent);
  backdrop-filter: blur(14px);
}

.topbar-left {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.breadcrumb {
  display: flex;
  align-items: center;
  gap: 9px;
  min-width: 0;
  color: var(--muted);
  font-size: 11px;
}

.breadcrumb .mono {
  color: var(--muted);
  font-size: 9px;
  letter-spacing: 0.08em;
}

.breadcrumb strong {
  overflow: hidden;
  color: var(--ink);
  font-size: 12px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.breadcrumb-slash {
  color: var(--line-strong);
}

.topbar-tools {
  display: flex;
  align-items: center;
  gap: 4px;
}

.search-trigger {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 32px;
  margin-right: 4px;
  padding: 0 9px;
  border: 1px solid var(--line);
  border-radius: 999px;
  color: var(--muted);
  font-size: 10px;
  background: var(--surface);
}

.search-trigger:hover {
  border-color: var(--line-strong);
  color: var(--ink-soft);
}

kbd {
  padding: 2px 5px;
  border: 1px solid var(--line);
  border-radius: 4px;
  color: var(--muted);
  font-size: 8px;
}

.tool-button {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 32px;
  min-height: 32px;
  padding: 0 7px;
  border-radius: 7px;
  color: var(--muted-strong);
  font-size: 10px;
}

.tool-button:hover {
  background: var(--surface-soft);
  color: var(--ink);
}

.agent-trigger {
  color: var(--accent-strong);
}

.tool-label {
  display: none;
}

.tool-divider {
  width: 1px;
  height: 18px;
  margin: 0 4px;
  background: var(--line);
}

.locale-button {
  min-width: 30px;
  min-height: 30px;
  border-radius: 7px;
  color: var(--muted);
  font-size: 9px;
  letter-spacing: 0.08em;
}

.locale-button:hover {
  background: var(--surface-soft);
  color: var(--ink);
}

.menu-button {
  display: none;
}

.content {
  max-width: 1600px;
  min-height: calc(100vh - var(--topbar-h));
  margin: 0 auto;
  padding: clamp(24px, 3.4vw, 46px) var(--gutter) 70px;
}

.rail-scrim {
  display: none;
}

@media (min-width: 1300px) {
  .tool-label {
    display: inline;
  }
}

@media (max-width: 1100px) {
  .shell {
    grid-template-columns: 1fr;
  }

  .rail {
    position: fixed;
    left: 0;
    top: 0;
    z-index: 45;
    transform: translateX(-102%);
    transition: transform var(--normal) var(--ease);
    box-shadow: var(--shadow-float);
  }

  .rail.open {
    transform: translateX(0);
  }

  .rail-scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: block;
    background: rgba(18, 22, 17, 0.28);
  }

  .menu-button {
    display: inline-flex;
  }
}

@media (max-width: 760px) {
  .search-trigger span,
  .search-trigger kbd {
    display: none;
  }

  .search-trigger {
    min-width: 32px;
    justify-content: center;
    padding: 0;
  }

  .breadcrumb .mono,
  .breadcrumb-slash {
    display: none;
  }

  .topbar {
    gap: 8px;
  }

  .tool-divider {
    display: none;
  }
}
</style>
