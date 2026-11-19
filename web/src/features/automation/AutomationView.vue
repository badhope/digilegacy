<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import ErrorState from "@/shared/ui/ErrorState.vue";
import MetricCard from "@/shared/ui/MetricCard.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import SkeletonBlock from "@/shared/ui/SkeletonBlock.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import { platformClient } from "@/platform/client";
import { TOOL_CATALOG } from "@/platform/generated/tools";
import type { AgentRun, AgentRuns } from "@/platform/types";
import { loadRuns } from "./api";

const router = useRouter();
const { t, locale } = useI18n();
const { data, loading, error, reload } = useResource(loadRuns);
const usageResource = useResource(() => platformClient.usageSummary(30));
const approvalResource = useResource(() => platformClient.listApprovals({ status: "pending" }));
const readTools = computed(() => TOOL_CATALOG.filter((tool) => tool.mcp));

const approvals = computed(() => approvalResource.data.value?.items ?? []);
const waiting = computed(() => approvalResource.data.value?.pending ?? approvals.value.length);
const failed = computed(() => data.value?.items.filter((run) => run.status === "failed").length ?? 0);
const usage = computed(() => usageResource.data.value);
const usageLoading = usageResource.loading;
const usageError = usageResource.error;
const usageDays = computed(() => {
  const byDay = new Map((usage.value?.byDay ?? []).map((day) => [day.day, day]));
  const today = new Date();
  return Array.from({ length: 14 }, (_, index) => {
    const date = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate() - (13 - index)));
    const day = date.toISOString().slice(0, 10);
    return { day, tokens: byDay.get(day)?.tokens ?? 0 };
  });
});
const maxDayTokens = computed(() => Math.max(1, ...usageDays.value.map((day) => day.tokens)));

function dayBar(day: { tokens: number }): string {
  return `${Math.max(2, Math.round((day.tokens / maxDayTokens.value) * 34))}px`;
}

function tone(status: AgentRun["status"]): "ready" | "planned" | "degraded" | "danger" | "neutral" {
  if (status === "succeeded") return "ready";
  if (status === "failed") return "danger";
  if (status === "waiting_approval") return "degraded";
  if (status === "running" || status === "cancelling") return "planned";
  return "neutral";
}

function statusKey(status: AgentRun["status"]): string {
  return `run.status.${status}`;
}

function formatTokens(value: number | undefined): string {
  return new Intl.NumberFormat(locale.value === "zh-CN" ? "zh-CN" : "en-US").format(value ?? 0);
}

function formatCost(micros: number | undefined): string {
  if (!micros) return "—";
  return `¥${(micros / 1_000_000).toFixed(6)}`;
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(locale.value, { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }).format(date);
}

function openRun(run: AgentRun) {
  router.push(`/automation/runs/${run.id}`);
}
</script>

<template>
  <div class="automation-page">
    <PageHeader
      eyebrow="AUTOMATION / AGENT"
      :title="t('automation.title')"
      :description="t('automation.description')"
    >
      <template #actions>
        <AppButton variant="primary" @click="router.push('/automation/runs/new')">
          {{ t("automation.runs.new") }}
          <AppIcon name="arrow-right" :size="14" />
        </AppButton>
        <AppButton variant="secondary" @click="router.push('/automation/approvals')">
          {{ t("automation.approvals") }}
          <AppIcon name="arrow-right" :size="14" />
        </AppButton>
        <StatusPill tone="ready" :label="t('app.mode')" />
      </template>
    </PageHeader>

    <div v-if="loading" class="automation-loading">
      <div v-for="index in 4" :key="index" class="automation-skeleton">
        <SkeletonBlock width="100px" height="11px" />
        <SkeletonBlock width="60px" height="38px" />
      </div>
      <div class="automation-skeleton wide">
        <SkeletonBlock width="100%" height="180px" />
      </div>
    </div>

    <ErrorState
      v-else-if="error"
      :title="t('error.generic')"
      :message="error"
      :retry-label="t('common.retry')"
      @retry="reload"
    />

    <template v-else-if="data">
      <section class="automation-metrics">
        <MetricCard :label="t('automation.metric.runs')" :value="data.items.length" :unit="t('overview.unit.items')">
          <template #icon><AppIcon name="agent" :size="16" /></template>
        </MetricCard>
        <MetricCard :label="t('automation.metric.waiting')" :value="waiting" :unit="t('overview.unit.pending')">
          <template #icon><AppIcon name="clock" :size="16" /></template>
        </MetricCard>
        <MetricCard :label="t('automation.metric.failed')" :value="failed" :unit="t('overview.unit.items')">
          <template #icon><AppIcon name="alert" :size="16" /></template>
        </MetricCard>
        <MetricCard :label="t('automation.tools.count')" :value="readTools.length" :unit="t('overview.unit.items')">
          <template #icon><AppIcon name="command" :size="16" /></template>
        </MetricCard>
      </section>

      <div class="automation-grid">
        <SectionCard :title="t('automation.runs.title')" :subtitle="t('automation.runs.subtitle')" flush>
          <div class="run-table-wrap">
            <div class="run-table-head mono">
              <span>RUN</span>
              <span>{{ t("automation.runs") }}</span>
              <span>{{ t("assets.table.status") }}</span>
              <span>STEPS</span>
              <span>{{ t("assets.table.updated") }}</span>
            </div>
            <button v-for="run in data.items" :key="run.id" class="run-row" type="button" @click="openRun(run)">
              <span class="mono run-id">{{ run.id }}</span>
              <span class="run-title">{{ run.title }}</span>
              <StatusPill :tone="tone(run.status)" :label="t(statusKey(run.status))" />
              <span class="mono run-steps">{{ run.steps }}</span>
              <span class="mono run-date">{{ formatDate(run.startedAt) }}</span>
            </button>
          </div>
        </SectionCard>

        <div class="automation-side">
          <SectionCard :title="t('automation.usage.title')" :subtitle="t('automation.usage.subtitle')">
            <div v-if="usageLoading" class="usage-empty">{{ t("common.loading") }}</div>
            <div v-else-if="usageError" class="usage-empty">{{ usageError }}</div>
            <div v-else-if="usage" class="usage-grid">
              <div><span>{{ t("automation.usage.plans") }}</span><strong class="mono">{{ usage.totals.plans }}</strong></div>
              <div><span>{{ t("automation.usage.runs") }}</span><strong class="mono">{{ usage.totals.runs }}</strong></div>
              <div><span>{{ t("automation.usage.tokens") }}</span><strong class="mono">{{ formatTokens(usage.totals.tokens) }}</strong></div>
              <div><span>{{ t("automation.usage.cost") }}</span><strong class="mono">{{ formatCost(usage.totals.costMicros) }}</strong></div>
            </div>
            <div v-if="usageDays.length" class="usage-bars" :aria-label="t('automation.usage.title')">
              <div v-for="day in usageDays" :key="day.day" class="usage-bar" :title="`${day.day} · ${formatTokens(day.tokens)} tokens`">
                <span :style="{ height: dayBar(day) }"></span>
              </div>
            </div>
            <div v-if="usage?.byModel.length" class="usage-models">
              <span class="mono" v-for="model in usage.byModel.slice(0, 3)" :key="model.model">
                {{ model.model }} · {{ formatTokens(model.tokens) }} tokens · {{ formatCost(model.costMicros) }}
              </span>
            </div>
            <p v-if="usage && usage.totals.unpricedPlans > 0" class="usage-note">{{ t("automation.usage.unpriced", { count: usage.totals.unpricedPlans }) }}</p>
            <div v-else-if="usage && !usage.totals.plans" class="usage-empty">{{ t("automation.usage.empty") }}</div>
          </SectionCard>

          <SectionCard :title="t('automation.approvals.title')" :subtitle="t('automation.approvals.subtitle')">
            <div v-if="approvals.length" class="approval-list">
              <div v-for="approval in approvals" :key="approval.id" class="approval-run">
                <span class="approval-icon"><AppIcon name="clock" :size="16" /></span>
                <div>
                  <strong class="mono">{{ approval.tool }}</strong>
                  <p class="mono">{{ approval.runId }} · {{ approval.objectKey }}</p>
                </div>
              </div>
            </div>
            <div v-else class="approval-empty">
              <span class="approval-icon"><AppIcon name="check" :size="18" /></span>
              <div>
                <strong>{{ t("automation.approvals.empty") }}</strong>
                <p>{{ t("automation.approvals.subtitle") }}</p>
              </div>
            </div>
            <AppButton variant="ghost" size="sm" @click="router.push('/automation/approvals')">
              {{ t("automation.approvals.link") }}
              <AppIcon name="arrow-right" :size="14" />
            </AppButton>
          </SectionCard>

          <SectionCard :title="t('automation.tools.title')" :subtitle="t('automation.tools.subtitle')">
            <div class="tool-list">
              <button
                v-for="tool in readTools"
                :key="tool.name"
                class="tool-row"
                type="button"
                @click="router.push('/automation/tools')"
              >
                <span class="tool-icon"><AppIcon name="command" :size="14" /></span>
                <span class="tool-copy">
                  <strong class="mono">{{ tool.name }}</strong>
                  <small>{{ tool.title }}</small>
                </span>
                <StatusPill tone="ready" :label="t('common.ready')" />
              </button>
            </div>
          </SectionCard>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.automation-page {
  max-width: var(--content-max);
  margin: 0 auto;
}

.automation-metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
}

.automation-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.4fr) minmax(320px, 0.8fr);
  gap: 16px;
  margin-top: 16px;
}

.usage-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
.usage-grid > div { display: flex; flex-direction: column; gap: 4px; }
.usage-grid span { color: var(--muted); font-size: 10px; letter-spacing: 0.04em; text-transform: uppercase; }
.usage-grid strong { font-size: 16px; font-weight: 500; }
.usage-bars { display: flex; align-items: end; gap: 3px; height: 40px; margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--line); }
.usage-bar { display: flex; flex: 1 1 0; align-items: end; min-width: 0; height: 34px; }
.usage-bar span { display: block; width: 100%; min-height: 2px; background: var(--accent); border-radius: 2px 2px 0 0; opacity: 0.55; }
.usage-models { display: grid; gap: 4px; margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--line); }
.usage-models span { color: var(--muted); font-size: 10px; }
.usage-note { margin: 10px 0 0; color: var(--muted); font-size: 10px; }
.usage-empty { color: var(--muted); font-size: 12px; }
.automation-side {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.run-table-wrap {
  overflow-x: auto;
}

.run-table-head,
.run-row {
  display: grid;
  grid-template-columns: 78px minmax(180px, 1fr) 106px 46px 100px;
  align-items: center;
  gap: 12px;
  min-width: 640px;
  padding: 12px 22px;
}

.run-table-head {
  border-bottom: 1px solid var(--line);
  color: var(--muted);
  font-size: 9px;
  letter-spacing: 0.08em;
}

.run-row {
  width: 100%;
  border-bottom: 1px solid var(--line);
  text-align: left;
  transition: background var(--fast) var(--ease);
}

.run-row:last-child {
  border-bottom: 0;
}

.run-row:hover {
  background: var(--surface-soft);
}

.run-id,
.run-steps,
.run-date {
  color: var(--muted);
  font-size: 9px;
}

.run-title {
  overflow: hidden;
  color: var(--ink-soft);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.approval-list {
  display: flex;
  flex-direction: column;
}

.approval-run {
  display: flex;
  gap: 10px;
  padding: 12px 0;
  border-top: 1px solid var(--line);
}

.approval-run:first-child {
  border-top: 0;
}

.approval-run strong {
  display: block;
  color: var(--ink);
  font-size: 12px;
}

.approval-run p {
  margin: 3px 0 0;
  color: var(--muted);
  font-size: 9px;
}

.approval-empty {
  display: flex;
  gap: 12px;
  padding: 14px;
  border-radius: var(--radius-sm);
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.approval-icon {
  display: grid;
  flex-shrink: 0;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: color-mix(in srgb, var(--surface) 65%, transparent);
}

.approval-empty strong {
  display: block;
  color: var(--ink);
  font-size: 12px;
}

.approval-empty p {
  margin: 3px 0 0;
  color: var(--muted);
  font-size: 10px;
  line-height: 1.5;
}

.tool-list {
  display: flex;
  flex-direction: column;
}

.tool-row {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 9px 0;
  border-top: 1px solid var(--line);
  text-align: left;
}

.tool-row:first-child {
  border-top: 0;
}

.tool-icon {
  display: grid;
  flex-shrink: 0;
  width: 26px;
  height: 26px;
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
  overflow: hidden;
  color: var(--ink-soft);
  font-size: 10px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tool-copy small {
  display: block;
  margin-top: 1px;
  overflow: hidden;
  color: var(--muted);
  font-size: 9px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.automation-loading {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
}

.automation-skeleton {
  display: flex;
  flex-direction: column;
  gap: 24px;
  min-height: 140px;
  padding: 18px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.automation-skeleton.wide {
  display: none;
  grid-column: 1 / -1;
}

@media (max-width: 1100px) {
  .automation-metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .automation-grid {
    grid-template-columns: 1fr;
  }

  .usage-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
.usage-grid > div { display: flex; flex-direction: column; gap: 4px; }
.usage-grid span { color: var(--muted); font-size: 10px; letter-spacing: 0.04em; text-transform: uppercase; }
.usage-grid strong { font-size: 16px; font-weight: 500; }
.usage-bars { display: flex; align-items: end; gap: 3px; height: 40px; margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--line); }
.usage-bar { display: flex; flex: 1 1 0; align-items: end; min-width: 0; height: 34px; }
.usage-bar span { display: block; width: 100%; min-height: 2px; background: var(--accent); border-radius: 2px 2px 0 0; opacity: 0.55; }
.usage-models { display: grid; gap: 4px; margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--line); }
.usage-models span { color: var(--muted); font-size: 10px; }
.usage-note { margin: 10px 0 0; color: var(--muted); font-size: 10px; }
.usage-empty { color: var(--muted); font-size: 12px; }
.automation-side {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
}

@media (max-width: 760px) {
  .automation-metrics {
    grid-template-columns: 1fr;
  }

  .usage-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
.usage-grid > div { display: flex; flex-direction: column; gap: 4px; }
.usage-grid span { color: var(--muted); font-size: 10px; letter-spacing: 0.04em; text-transform: uppercase; }
.usage-grid strong { font-size: 16px; font-weight: 500; }
.usage-bars { display: flex; align-items: end; gap: 3px; height: 40px; margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--line); }
.usage-bar { display: flex; flex: 1 1 0; align-items: end; min-width: 0; height: 34px; }
.usage-bar span { display: block; width: 100%; min-height: 2px; background: var(--accent); border-radius: 2px 2px 0 0; opacity: 0.55; }
.usage-models { display: grid; gap: 4px; margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--line); }
.usage-models span { color: var(--muted); font-size: 10px; }
.usage-note { margin: 10px 0 0; color: var(--muted); font-size: 10px; }
.usage-empty { color: var(--muted); font-size: 12px; }
.automation-side {
    grid-template-columns: 1fr;
  }
}
</style>
