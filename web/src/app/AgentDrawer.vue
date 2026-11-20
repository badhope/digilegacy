<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { TOOL_CATALOG } from "@/platform/generated/tools";
import { platformClient } from "@/platform/client";
import { useResource } from "@/platform/useResource";
import { closeAgent, ui } from "@/platform/ui";
import { useI18n } from "@/platform/i18n";
import { useDialogFocus } from "./dialogFocus";

const router = useRouter();
const { t } = useI18n();
const panel = ref<HTMLElement | null>(null);
const toolCount = TOOL_CATALOG.length;
const { data: approvals } = useResource(() => platformClient.listApprovals({ status: "pending" }));
const pending = computed(() => approvals.value?.pending ?? 0);
useDialogFocus(computed(() => ui.agentOpen), panel);

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && ui.agentOpen) closeAgent();
}

function open(path: string) {
  router.push(path);
  closeAgent();
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <Transition name="drawer">
      <div v-if="ui.agentOpen" class="agent-layer" role="presentation" @click.self="closeAgent">
        <aside ref="panel" class="agent-drawer" role="dialog" aria-modal="true" :aria-label="t('agent.drawer.title')" tabindex="-1">
          <header class="drawer-head">
            <div class="head-copy">
              <span class="agent-sign"><AppIcon name="agent" :size="19" /></span>
              <div>
                <span class="eyebrow">AGENT / WORKBENCH</span>
                <h2>{{ t("agent.drawer.title") }}</h2>
              </div>
            </div>
            <button class="icon-button" type="button" :aria-label="t('common.close')" @click="closeAgent">
              <AppIcon name="close" :size="18" />
            </button>
          </header>

          <div class="drawer-status">
            <StatusPill tone="ready" :label="t('overview.agent.online')" />
            <StatusPill tone="planned" :label="t('overview.agent.readonly')" />
            <span class="mono status-num">{{ toolCount }} TOOLS</span>
          </div>

          <p class="drawer-note">{{ t("agent.drawer.subtitle") }}</p>

          <div class="pending-box">
            <span class="pending-sign"><AppIcon name="clock" :size="16" /></span>
            <div class="pending-copy">
              <strong>{{ t("agent.drawer.approvals") }}</strong>
              <small>{{ t("automation.approvals.subtitle") }}</small>
            </div>
            <StatusPill :tone="pending ? 'degraded' : 'ready'" :label="t('approvals.pendingCount', { count: pending })" />
          </div>

          <div class="drawer-actions">
            <AppButton variant="primary" @click="open('/automation/runs/new')">
              {{ t("automation.runs.new") }}
              <AppIcon name="arrow-right" :size="14" />
            </AppButton>
            <AppButton variant="secondary" @click="open('/automation?tab=approvals')">
              {{ t("automation.approvals") }}
              <AppIcon name="arrow-right" :size="14" />
            </AppButton>
            <AppButton variant="secondary" @click="open('/automation?tab=tools')">
              {{ t("automation.tools") }}
              <AppIcon name="arrow-right" :size="14" />
            </AppButton>
          </div>

          <p class="drawer-disclaimer">{{ t("agent.drawer.note") }}</p>

          <footer class="drawer-foot">
            <AppButton variant="secondary" @click="open('/automation')">
              {{ t("agent.drawer.open") }}
              <AppIcon name="arrow-right" :size="14" />
            </AppButton>
          </footer>
        </aside>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.agent-layer {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  justify-content: flex-end;
  background: rgba(18, 22, 17, 0.28);
  backdrop-filter: blur(3px);
}

.agent-drawer {
  display: flex;
  flex-direction: column;
  width: min(430px, 92vw);
  height: 100%;
  padding: 22px;
  overflow-y: auto;
  outline: none;
  border-left: 1px solid var(--line);
  background: var(--bg-elevated);
  box-shadow: var(--shadow-float);
}

.drawer-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 18px;
  border-bottom: 1px solid var(--line);
}

.head-copy {
  display: flex;
  align-items: center;
  gap: 12px;
}

.agent-sign {
  display: grid;
  width: 42px;
  height: 42px;
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
}

.head-copy h2 {
  margin-top: 5px;
  font-size: 24px;
}

.icon-button {
  display: grid;
  width: 34px;
  height: 34px;
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--muted);
  place-items: center;
  transition:
    color var(--fast) var(--ease),
    border-color var(--fast) var(--ease);
}

.icon-button:hover {
  border-color: var(--accent);
  color: var(--accent-strong);
}

.drawer-status {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 15px 0 0;
}

.status-num {
  margin-left: auto;
  color: var(--muted);
  font-size: 9px;
  letter-spacing: 0.08em;
}

.drawer-note {
  margin: 12px 0 0;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.6;
}

.pending-box {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 20px;
  padding: 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface);
}

.pending-sign {
  display: grid;
  flex-shrink: 0;
  width: 30px;
  height: 30px;
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
}

.pending-copy {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.pending-copy strong {
  color: var(--ink);
  font-size: 12px;
}

.pending-copy small {
  margin-top: 2px;
  color: var(--muted);
  font-size: 9px;
  line-height: 1.4;
}

.drawer-actions {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
  margin-top: 18px;
}

.drawer-actions :deep(.app-button) {
  justify-content: space-between;
}

.drawer-disclaimer {
  margin: 24px 0 0;
  color: var(--muted);
  font-size: 10px;
  line-height: 1.6;
}

.drawer-foot {
  margin-top: auto;
  padding-top: 22px;
}

.drawer-enter-active,
.drawer-leave-active {
  transition: opacity var(--normal) var(--ease);
}

.drawer-enter-active .agent-drawer,
.drawer-leave-active .agent-drawer {
  transition: transform var(--normal) var(--ease);
}

.drawer-enter-from,
.drawer-leave-to {
  opacity: 0;
}

.drawer-enter-from .agent-drawer,
.drawer-leave-to .agent-drawer {
  transform: translateX(24px);
}
</style>
