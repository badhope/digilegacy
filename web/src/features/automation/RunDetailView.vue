<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import EmptyState from "@/shared/ui/EmptyState.vue";
import ErrorState from "@/shared/ui/ErrorState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SkeletonBlock from "@/shared/ui/SkeletonBlock.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import { platformClient } from "@/platform/client";
import type { RunStatus, RunStep } from "@/platform/types";

const route = useRoute();
const router = useRouter();
const { t, locale } = useI18n();
const runId = computed(() => String(route.params.id ?? ""));
const { data, loading, error, reload, refresh } = useResource(() => platformClient.getRun(runId.value));
watch(runId, () => reload());
const acting = ref(false);
const actionError = ref("");
let pollTimer: number | undefined;
let eventSource: EventSource | undefined;
let streamOpen = false;

function isActive(status: RunStatus | undefined): boolean {
  return status === "queued" || status === "running" || status === "waiting_approval";
}

function stopPolling() {
  if (pollTimer !== undefined) {
    window.clearInterval(pollTimer);
    pollTimer = undefined;
  }
  if (eventSource) {
    eventSource.close();
    eventSource = undefined;
  }
  streamOpen = false;
}

function startStream(id: string) {
  if (typeof EventSource === "undefined" || !id) return;
  const source = new EventSource(platformClient.exportUrl(`/api/v1/events?runId=${encodeURIComponent(id)}`));
  source.addEventListener("open", () => {
    streamOpen = true;
  });
  source.addEventListener("snapshot", (event) => {
    try {
      const payload = JSON.parse((event as MessageEvent).data) as { run?: typeof data.value };
      if (payload.run) data.value = payload.run;
    } catch {
      // A malformed frame must not break the fallback poller.
    }
  });
  source.addEventListener("error", () => {
    streamOpen = false;
  });
  eventSource = source;
}

watch(
  [runId, () => data.value?.status],
  ([, status]) => {
    stopPolling();
    if (!isActive(status)) return;
    startStream(runId.value);
    pollTimer = window.setInterval(() => {
      if (!streamOpen && document.visibilityState !== "hidden") refresh();
    }, 1500);
  },
  { immediate: true },
);

onUnmounted(stopPolling);

async function cancelRun() {
  actionError.value = "";
  acting.value = true;
  try {
    const envelope = await platformClient.cancelRun(runId.value);
    if (!envelope.success) {
      actionError.value = envelope.code === "AUTH_REQUIRED" ? t("runs.detail.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    reload();
  } finally {
    acting.value = false;
  }
}

async function retryRun() {
  actionError.value = "";
  acting.value = true;
  try {
    const envelope = await platformClient.retryRun(runId.value);
    if (!envelope.success || !envelope.data) {
      actionError.value = envelope.code === "AUTH_REQUIRED" ? t("runs.detail.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    router.push(`/automation/runs/${envelope.data.id}`);
  } finally {
    acting.value = false;
  }
}

function tone(status: RunStatus): "ready" | "planned" | "degraded" | "danger" | "neutral" {
  if (status === "succeeded") return "ready";
  if (status === "failed" || status === "rejected") return "danger";
  if (status === "waiting_approval") return "degraded";
  if (status === "running") return "planned";
  return "neutral";
}

function stepTone(status: RunStep["status"]): "ready" | "planned" | "degraded" | "danger" | "neutral" {
  if (status === "done") return "ready";
  if (status === "failed" || status === "rejected") return "danger";
  if (status === "pending_approval") return "degraded";
  if (status === "queued" || status === "cancelled") return "neutral";
  return "planned";
}

function formatDuration(ms: number): string {
  if (!ms) return "—";
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`;
}

function formatUsage(): string {
  if (!data.value || data.value.tokens <= 0) return "—";
  const tokens = new Intl.NumberFormat(locale.value === "zh-CN" ? "zh-CN" : "en-US").format(data.value.tokens);
  if (!data.value.usagePriced) return `${tokens} tokens · ${t("runs.detail.costUnknown")}`;
  return `${tokens} tokens · ¥${(data.value.costMicros / 1_000_000).toFixed(6)}`;
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value || "—";
  return new Intl.DateTimeFormat(locale.value, {
    month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit",
  }).format(date);
}

function pretty(value: unknown): string {
  if (value === null || value === undefined) return "";
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
</script>

<template>
  <div class="run-detail-page">
    <PageHeader
      eyebrow="AUTOMATION / RUN"
      :title="data ? `#${data.id}` : `#${runId}`"
      :description="data?.task || t('runs.detail.subtitle')"
    >
      <template #actions>
        <AppButton variant="secondary" size="sm" @click="router.push('/automation')">
          <AppIcon name="arrow-right" :size="14" class="back-arrow" />
          {{ t("runs.detail.back") }}
        </AppButton>
        <StatusPill v-if="data" :tone="tone(data.status)" :label="t(`run.status.${data.status}`)" />
        <AppButton
          v-if="data && ['running', 'waiting_approval'].includes(data.status)"
          variant="secondary"
          size="sm"
          :disabled="acting"
          @click="cancelRun"
        >{{ t("runs.detail.cancel") }}</AppButton>
        <AppButton
          v-if="data && ['failed', 'rejected', 'cancelled'].includes(data.status)"
          variant="primary"
          size="sm"
          :disabled="acting"
          @click="retryRun"
        >{{ t("runs.detail.retry") }}</AppButton>
      </template>
    </PageHeader>

    <div v-if="actionError" class="run-error-banner">{{ actionError }}</div>

    <div v-if="loading" class="run-loading">
      <SkeletonBlock width="100%" height="180px" />
    </div>

    <ErrorState
      v-else-if="error"
      :title="t('error.generic')"
      :message="error"
      :retry-label="t('common.retry')"
      @retry="reload"
    />

    <template v-else-if="data">
      <div class="run-summary">
        <div><span>{{ t("runs.detail.started") }}</span><strong class="mono">{{ formatDate(data.startedAt) }}</strong></div>
        <div><span>{{ t("runs.detail.rounds") }}</span><strong class="mono">{{ data.rounds }}</strong></div>
        <div><span>{{ t("runs.detail.tools") }}</span><strong class="mono">{{ data.toolsUsed || "—" }}</strong></div>
        <div><span>{{ t("runs.detail.duration") }}</span><strong class="mono">{{ formatDuration(data.durationMs) }}</strong></div>
        <div><span>{{ t("runs.detail.usage") }}</span><strong class="mono">{{ formatUsage() }}</strong></div>
      </div>

      <p v-if="isActive(data.status)" class="run-live mono">{{ t("runs.detail.live") }}</p>

      <div v-if="data.status === 'waiting_approval'" class="run-banner">
        <span>{{ t("runs.detail.waitingBanner") }}</span>
        <AppButton variant="primary" size="sm" @click="router.push('/automation/approvals')">
          {{ t("runs.detail.goApprovals") }}
          <AppIcon name="arrow-right" :size="14" />
        </AppButton>
      </div>

      <div class="step-list">
        <article v-for="(step, index) in data.steps" :key="index" class="step-card">
          <header class="step-head">
            <div class="step-title">
              <span class="step-index mono">{{ String(index + 1).padStart(2, "0") }}</span>
              <strong class="mono">{{ step.tool }}</strong>
            </div>
            <StatusPill :tone="stepTone(step.status)" :label="t(`runs.step.${step.status}`)" />
          </header>
          <div v-if="step.at" class="step-time mono">{{ formatDate(step.at) }}</div>
          <pre v-if="Object.keys(step.args ?? {}).length" class="step-block arg">{{ pretty(step.args) }}</pre>
          <pre v-if="step.result" class="step-block result">{{ pretty(step.result) }}</pre>
          <p v-if="step.error" class="step-error">{{ step.error }}</p>
        </article>
      </div>

      <div v-if="!data.steps.length" class="run-empty">
        <EmptyState :title="t('runs.detail.empty')" :note="t('runs.detail.emptyNote')" icon="agent" />
      </div>

      <div v-if="data.finalText" class="run-final">
        <span>{{ t("runs.detail.final") }}</span>
        <p>{{ data.finalText }}</p>
      </div>
    </template>
  </div>
</template>

<style scoped>
.back-arrow { transform: rotate(180deg); }
.run-detail-page { max-width: var(--content-max); margin: 0 auto; }
.run-loading { padding: 10px 0; }
.run-summary { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 1px; margin-bottom: 16px; border: 1px solid var(--line); border-radius: var(--radius-md); overflow: hidden; background: var(--line); }
.run-summary > div { display: flex; flex-direction: column; gap: 6px; padding: 14px 16px; background: var(--surface); }
.run-summary span { color: var(--muted); font-size: 10px; letter-spacing: 0.06em; text-transform: uppercase; }
.run-summary strong { font-size: 13px; font-weight: 500; }
.run-error-banner { margin-bottom: 12px; padding: 9px 11px; border: 1px solid var(--line); border-radius: var(--radius-sm); color: var(--danger, #b42318); font-size: 12px; }
.run-live { margin: -6px 0 12px; color: var(--muted); font-size: 10px; letter-spacing: 0.04em; }
.run-banner { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 16px; padding: 12px 14px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); color: var(--ink-soft); font-size: 12px; }
.step-list { display: grid; gap: 10px; }
.step-card { padding: 14px 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.step-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.step-title { display: flex; align-items: center; gap: 10px; }
.step-index { color: var(--muted); font-size: 10px; }
.step-title strong { font-size: 13px; }
.step-time { margin-top: 8px; color: var(--muted); font-size: 10px; }
.step-block { max-height: 220px; overflow: auto; margin: 10px 0 0; padding: 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); color: var(--ink-soft); font-size: 11px; line-height: 1.5; }
.step-block.arg { color: var(--muted); }
.step-error { margin: 10px 0 0; color: var(--danger, #b42318); font-size: 11px; }
.run-empty { margin-top: 16px; }
.run-final { margin-top: 16px; padding: 14px 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.run-final span { color: var(--muted); font-size: 10px; letter-spacing: 0.06em; text-transform: uppercase; }
.run-final p { margin: 6px 0 0; font-size: 13px; }
@media (max-width: 640px) {
  .run-summary { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
</style>
