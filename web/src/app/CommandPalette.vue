<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import AppIcon from "@/shared/icons/AppIcon.vue";
import type { IconName } from "@/shared/icons/paths";
import { lockVault } from "@/platform/security";
import { closePalette, markLocked, ui } from "@/platform/ui";
import { useI18n } from "@/platform/i18n";
import { useTheme } from "@/platform/theme";
import { useDialogFocus } from "./dialogFocus";

const router = useRouter();
const { t, locale, toggleLocale } = useI18n();
const { theme, toggleTheme } = useTheme();
const panel = ref<HTMLElement | null>(null);
const query = ref("");

const navItems: Array<{ path: string; labelKey: string; icon: IconName }> = [
  { path: "/overview", labelKey: "nav.overview", icon: "overview" },
  { path: "/assets", labelKey: "nav.assets", icon: "assets" },
  { path: "/planning", labelKey: "nav.planning", icon: "planning" },
  { path: "/deliverables", labelKey: "nav.deliverables", icon: "deliverables" },
  { path: "/automation", labelKey: "nav.automation", icon: "automation" },
  { path: "/settings/connections", labelKey: "nav.settings", icon: "settings" },
];

const actions = computed<Array<{ id: string; label: string; icon: IconName; run: () => void }>>(() => [
  {
    id: "new-run",
    label: t("command.action.newRun"),
    icon: "agent",
    run: () => go("/automation/runs/new"),
  },
  {
    id: "approvals",
    label: t("command.action.approvals"),
    icon: "clock",
    run: () => go("/automation?tab=approvals"),
  },
  {
    id: "tools",
    label: t("command.action.tools"),
    icon: "command",
    run: () => go("/automation?tab=tools"),
  },
  {
    id: "connections",
    label: t("command.action.connections"),
    icon: "settings",
    run: () => go("/settings/connections"),
  },
  {
    id: "lock",
    label: t("command.action.lock"),
    icon: "lock",
    run: async () => {
      close();
      if (ui.securityConfigured && (await lockVault())) {
        markLocked();
        return;
      }
      go("/settings/security");
    },
  },
  {
    id: "theme",
    label: theme.value === "light" ? t("command.action.dark") : t("command.action.light"),
    icon: theme.value === "light" ? "moon" : "sun",
    run: () => {
      toggleTheme();
      close();
    },
  },
  {
    id: "language",
    label: locale.value === "zh-CN" ? t("command.action.english") : t("command.action.chinese"),
    icon: "settings",
    run: () => {
      toggleLocale();
      close();
    },
  },
]);

const filteredNav = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  return navItems.filter((item) => !keyword || t(item.labelKey).toLowerCase().includes(keyword));
});

const filteredActions = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  return actions.value.filter((item) => !keyword || item.label.toLowerCase().includes(keyword));
});

function close() {
  closePalette();
}

function go(path: string) {
  router.push(path);
  close();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && ui.paletteOpen) close();
}

useDialogFocus(computed(() => ui.paletteOpen), panel, "first");

watch(
  () => ui.paletteOpen,
  (open) => {
    if (open) query.value = "";
  },
);

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <Transition name="palette">
      <div v-if="ui.paletteOpen" class="palette-layer" role="presentation" @click.self="close">
        <div ref="panel" class="palette" role="dialog" aria-modal="true" :aria-label="t('command.title')">
          <header class="palette-head">
            <AppIcon name="search" :size="18" />
            <input
              v-model="query"
              type="search"
              :placeholder="t('command.placeholder')"
              :aria-label="t('command.title')"
            />
            <button class="esc-key mono" type="button" :aria-label="t('common.close')" @click="close">ESC</button>
          </header>

          <div class="palette-body">
            <section class="palette-group">
              <span class="group-label">{{ t("command.nav") }}</span>
              <button v-for="item in filteredNav" :key="item.path" class="palette-row" type="button" @click="go(item.path)">
                <span class="row-icon"><AppIcon :name="item.icon" :size="15" /></span>
                <span class="row-copy">{{ t(item.labelKey) }}</span>
                <span class="mono row-meta">{{ item.path }}</span>
              </button>
            </section>

            <section class="palette-group">
              <span class="group-label">{{ t("command.actions") }}</span>
              <button v-for="action in filteredActions" :key="action.id" class="palette-row" type="button" @click="action.run">
                <span class="row-icon"><AppIcon :name="action.icon" :size="15" /></span>
                <span class="row-copy">{{ action.label }}</span>
              </button>
            </section>

            <p v-if="!filteredNav.length && !filteredActions.length" class="palette-empty">
              {{ t("command.empty") }}
            </p>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.palette-layer {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: 12vh 20px 20px;
  background: rgba(18, 22, 17, 0.34);
  backdrop-filter: blur(5px);
}

.palette {
  width: min(680px, 100%);
  overflow: hidden;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
  box-shadow: var(--shadow-float);
}

.palette-head {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 14px 18px;
  border-bottom: 1px solid var(--line);
  color: var(--muted);
}

.palette-head input {
  width: 100%;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--ink);
  font-size: 14px;
}

.palette-head input::placeholder {
  color: var(--muted);
}

.esc-key {
  padding: 3px 6px;
  border: 1px solid var(--line);
  border-radius: 5px;
  color: var(--muted);
  font-size: 8px;
  letter-spacing: 0.08em;
}

.palette-body {
  max-height: min(62vh, 520px);
  padding: 8px;
  overflow-y: auto;
}

.palette-group {
  padding: 8px 0;
}

.group-label {
  display: block;
  padding: 5px 10px 7px;
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 8px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.palette-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 10px;
  border-radius: var(--radius-sm);
  text-align: left;
}

.palette-row:hover {
  background: var(--surface-soft);
}

.row-icon {
  display: grid;
  flex-shrink: 0;
  width: 26px;
  height: 26px;
  border-radius: 7px;
  color: var(--muted-strong);
  place-items: center;
  background: var(--surface);
}

.row-copy {
  flex: 1;
  color: var(--ink-soft);
  font-size: 12px;
}

.row-meta {
  color: var(--muted);
  font-size: 9px;
}

.palette-empty {
  margin: 10px;
  color: var(--muted);
  font-size: 12px;
}

.palette-enter-active,
.palette-leave-active {
  transition: opacity var(--fast) var(--ease);
}

.palette-enter-from,
.palette-leave-to {
  opacity: 0;
}

.palette-enter-active .palette,
.palette-leave-active .palette {
  transition: transform var(--fast) var(--ease);
}

.palette-enter-from .palette,
.palette-leave-to .palette {
  transform: translateY(-8px) scale(0.99);
}
</style>
