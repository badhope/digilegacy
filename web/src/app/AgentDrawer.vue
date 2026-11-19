<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { TOOL_CATALOG } from "@/platform/generated/tools";
import { useResource } from "@/platform/useResource";
import { loadRuns } from "@/features/automation/api";
import { closeAgent, ui } from "@/platform/ui";
import { useI18n } from "@/platform/i18n";

const router = useRouter();
const { t } = useI18n();
const panel = ref<HTMLElement | null>(null);
const readTools = computed(() => TOOL_CATALOG.filter((tool) => tool.mcp));
const { data: runs } = useResource(loadRuns);
const waitingRuns = computed(() => runs.value?.items.filter((run) => run.status === "waiting_approval") ?? []);

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && ui.agentOpen) closeAgent();
}

watch(
  () => ui.agentOpen,
  async (open) => {
    if (open) {
      await nextTick();
      panel.value?.focus();
    }
  },
);

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
            <span class="mono status-num">{{ readTools.length }} TOOLS</span>
          </div>

          <p class="drawer-note">{{ t("agent.drawer.subtitle") }}</p>

          <section class="drawer-section">
            <h3>{{ t("agent.drawer.tools") }}</h3>
            <div class="drawer-tools">
              <button
                v-for="tool in readTools"
                :key="tool.name"
                type="button"
                class="drawer-tool"
                @click="router.push('/automation/tools'); closeAgent()"
              >
                <span class="tool-sign"><AppIcon name="command" :size="14" /></span>
                <span class="tool-copy">
                  <strong class="mono">{{ tool.name }}</strong>
                  <small>{{ tool.title }}</small>
                </span>
                <AppIcon name="chevron-right" :size="14" />
              </button>
            </div>
          </section>

          <section class="drawer-section">
            <div class="section-head">
              <h3>{{ t("agent.drawer.approvals") }}</h3>
              <span class="mono section-count">{{ waitingRuns.length }}</span>
            </div>
            <div v-if="waitingRuns.length" class="approval-list">
              <div v-for="run in waitingRuns" :key="run.id" class="approval-row">
                <span class="approval-sign"><AppIcon name="clock" :size="15" /></span>
                <div>
                  <strong>{{ run.title }}</strong>
                  <small class="mono">{{ run.id }} · {{ t("run.status.waiting_approval") }}</small>
                </div>
              </div>
            </div>
            <div v-else class="approval-empty">
              <span class="approval-sign"><AppIcon name="check" :size="16" /></span>
              <span>{{ t("automation.approvals.empty") }}</span>
            </div>
          </section>

          <p class="drawer-disclaimer">{{ t("agent.drawer.note") }}</p>

          <footer class="drawer-foot">
            <AppButton variant="primary" @click="router.push('/automation'); closeAgent()">
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

.drawer-section {
  margin-top: 24px;
}

.drawer-section h3 {
  font-family: var(--font-body);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.drawer-tools {
  margin-top: 8px;
  border-top: 1px solid var(--line);
}

.drawer-tool {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 10px 0;
  border-bottom: 1px solid var(--line);
  text-align: left;
}

.drawer-tool:hover .tool-copy strong {
  color: var(--accent-strong);
}

.tool-sign {
  display: grid;
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  border-radius: 7px;
  color: var(--muted-strong);
  place-items: center;
  background: var(--surface-soft);
}

.tool-copy {
  flex: 1;
  min-width: 0;
}

.tool-copy strong {
  display: block;
  color: var(--ink-soft);
  font-size: 10px;
  font-weight: 500;
  transition: color var(--fast) var(--ease);
}

.tool-copy small {
  display: block;
  margin-top: 2px;
  overflow: hidden;
  color: var(--muted);
  font-size: 9px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.drawer-tool > :last-child {
  color: var(--muted);
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.section-count {
  color: var(--muted);
  font-size: 10px;
}

.approval-list {
  margin-top: 8px;
  border-top: 1px solid var(--line);
}

.approval-row {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 10px 0;
  border-bottom: 1px solid var(--line);
}

.approval-row strong {
  display: block;
  color: var(--ink-soft);
  font-size: 11px;
}

.approval-row small {
  display: block;
  margin-top: 2px;
  color: var(--muted);
  font-size: 8px;
}

.approval-empty {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-top: 10px;
  padding: 12px;
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
  color: var(--muted);
  font-size: 11px;
}

.approval-sign {
  display: grid;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
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
