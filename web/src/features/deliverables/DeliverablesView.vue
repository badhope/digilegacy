<script setup lang="ts">
import { onUnmounted, reactive, ref } from "vue";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { platformClient } from "@/platform/client";
import type { DeliverableExport, JobDetail } from "@/platform/types";
import type { IconName } from "@/shared/icons/paths";

const { t, locale } = useI18n();

const kinds: Array<{ kind: "report" | "checklist" | "legal" | "toolkit"; key: string; icon: IconName }> = [
  { kind: "report", key: "deliverables.kind.report", icon: "deliverables" },
  { kind: "checklist", key: "deliverables.kind.checklist", icon: "check" },
  { kind: "legal", key: "deliverables.kind.legal", icon: "shield" },
  { kind: "toolkit", key: "deliverables.kind.toolkit", icon: "assets" },
];

const busy = ref<string | null>(null);
const errors = reactive<Record<string, string>>({});
const results = reactive<Record<string, DeliverableExport | undefined>>({});
const pollers = new Map<string, number>();
const streams = new Map<string, EventSource>();
const cancelling = ref<string | null>(null);

function stopStream(kind: string) {
  const source = streams.get(kind);
  if (source) {
    source.close();
    streams.delete(kind);
  }
}

function stopPoll(kind: string) {
  const timer = pollers.get(kind);
  if (timer !== undefined) {
    window.clearInterval(timer);
    pollers.delete(kind);
  }
  stopStream(kind);
}

function applyJob(kind: string, job: JobDetail) {
  if (job.status === "succeeded") {
    results[kind] = {
      jobId: job.jobId,
      status: "succeeded",
      progress: 1,
      ...(job.result as Partial<DeliverableExport>),
    };
    stopPoll(kind);
    return;
  }
  if (job.status === "failed" || job.status === "cancelled") {
    errors[kind] = job.errorCode || job.status;
    delete results[kind];
    stopPoll(kind);
    return;
  }
  results[kind] = { jobId: job.jobId, status: job.status, progress: job.progress };
}

async function refreshJob(kind: string, jobId: string) {
  const envelope = await platformClient.getJob(jobId);
  if (!envelope.success || !envelope.data) {
    errors[kind] = envelope.message || envelope.code;
    stopPoll(kind);
    return;
  }
  applyJob(kind, envelope.data);
}

function startStream(kind: string, jobId: string) {
  if (typeof EventSource === "undefined") return;
  stopStream(kind);
  const source = new EventSource(platformClient.exportUrl(`/api/v1/events?jobId=${encodeURIComponent(jobId)}`));
  source.addEventListener("snapshot", (event) => {
    try {
      const payload = JSON.parse((event as MessageEvent).data) as { job?: JobDetail };
      if (payload.job) applyJob(kind, payload.job);
    } catch {
      // Keep the 1.5s fallback poller authoritative if a frame is malformed.
    }
  });
  source.addEventListener("error", () => {
    source.close();
    streams.delete(kind);
  });
  streams.set(kind, source);
}

async function cancel(kind: string) {
  const jobId = results[kind]?.jobId;
  if (!jobId || cancelling.value) return;
  cancelling.value = kind;
  errors[kind] = "";
  try {
    const envelope = await platformClient.cancelJob(jobId);
    if (!envelope.success) {
      errors[kind] = envelope.message || envelope.code;
      return;
    }
    stopPoll(kind);
    delete results[kind];
  } catch (cause) {
    errors[kind] = cause instanceof Error ? cause.message : t("deliverables.error");
  } finally {
    cancelling.value = null;
  }
}

function startPoll(kind: string, jobId: string) {
  stopPoll(kind);
  startStream(kind, jobId);
  pollers.set(
    kind,
    window.setInterval(() => {
      if (!streams.has(kind) && document.visibilityState !== "hidden") void refreshJob(kind, jobId);
    }, 1500),
  );
}

async function generate(kind: string) {
  if (busy.value) return;
  busy.value = kind;
  errors[kind] = "";
  try {
    const envelope = await platformClient.exportDeliverable(kind, locale.value);
    if (!envelope.success || !envelope.data) {
      errors[kind] = envelope.message || envelope.code;
      return;
    }
    results[kind] = envelope.data;
    if (envelope.data.status !== "succeeded") startPoll(kind, envelope.data.jobId);
  } catch (cause) {
    errors[kind] = cause instanceof Error ? cause.message : t("deliverables.error");
  } finally {
    busy.value = null;
  }
}

onUnmounted(() => {
  for (const timer of pollers.values()) window.clearInterval(timer);
  pollers.clear();
  for (const source of streams.values()) source.close();
  streams.clear();
});
</script>

<template>
  <div class="deliverables-page">
    <PageHeader eyebrow="DELIVERABLES / EXPORT" :title="t('deliverables.title')" :description="t('deliverables.description')" />

    <p class="deliverable-boundary">{{ t("deliverables.boundary") }}</p>
    <p class="deliverable-note">{{ t("deliverables.note") }}</p>

    <div class="deliverable-grid">
      <article v-for="kind in kinds" :key="kind.kind" class="deliverable-card">
        <div class="deliverable-top">
          <span class="deliverable-icon"><AppIcon :name="kind.icon" :size="19" /></span>
          <StatusPill
            :tone="results[kind.kind]?.status === 'succeeded' ? 'ready' : 'planned'"
            :label="results[kind.kind]?.status === 'succeeded' ? t('deliverables.generated') : results[kind.kind] ? t('deliverables.generating') : t('deliverables.ready')"
          />
        </div>
        <h2>{{ t(kind.key) }}</h2>

        <div v-if="results[kind.kind]?.status === 'succeeded'" class="deliverable-result">
          <span class="mono">{{ results[kind.kind]?.fileName }}</span>
          <a
            v-if="results[kind.kind]?.downloadUrl"
            class="deliverable-download"
            :href="platformClient.exportUrl(results[kind.kind]!.downloadUrl!)"
            target="_blank"
            rel="noopener"
          >
            <AppIcon name="external" :size="14" />
            {{ t("deliverables.download") }}
          </a>
        </div>
        <div v-else-if="results[kind.kind]" class="deliverable-result">
          <span class="mono">{{ results[kind.kind]?.jobId }}</span>
          <span class="deliverable-meta">{{ t("deliverables.generating") }}</span>
        </div>
        <div v-if="results[kind.kind]?.status === 'queued'" class="deliverable-cancel">
          <AppButton size="sm" :disabled="Boolean(cancelling)" @click="cancel(kind.kind)">
            {{ cancelling === kind.kind ? t("deliverables.cancelling") : t("deliverables.cancel") }}
          </AppButton>
        </div>
        <p v-if="errors[kind.kind]" class="deliverable-error">{{ errors[kind.kind] }}</p>

        <div class="deliverable-actions">
          <AppButton
            variant="primary"
            size="sm"
            :disabled="Boolean(busy)"
            @click="generate(kind.kind)"
          >
            <AppIcon name="plus" :size="14" />
            {{ busy === kind.kind ? t("deliverables.generating") : t("deliverables.generate") }}
          </AppButton>
        </div>
      </article>
    </div>
  </div>
</template>

<style scoped>
.deliverables-page {
  max-width: var(--content-max);
  margin: 0 auto;
}

.deliverable-boundary {
  margin: 0;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.6;
}

.deliverable-note {
  margin: 2px 0 16px;
  color: var(--muted);
  font-size: 10px;
  line-height: 1.6;
}

.deliverable-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
}

.deliverable-card {
  display: flex;
  flex-direction: column;
  padding: 20px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
  transition:
    border-color var(--fast) var(--ease),
    transform var(--fast) var(--ease);
}

.deliverable-card:hover {
  border-color: var(--accent);
  transform: translateY(-2px);
}

.deliverable-top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}

.deliverable-icon {
  display: grid;
  width: 42px;
  height: 42px;
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
}

.deliverable-card h2 {
  margin-top: 18px;
  font-size: 20px;
}

.deliverable-card p {
  margin: 7px 0 0;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.6;
}

.deliverable-result {
  display: grid;
  gap: 6px;
  margin-top: 12px;
  padding: 9px 10px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
  color: var(--ink-soft);
  font-size: 9px;
  word-break: break-all;
}

.deliverable-download {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--accent-strong);
  font-size: 11px;
  font-weight: 600;
  text-decoration: none;
}

.deliverable-download:hover {
  text-decoration: underline;
}

.deliverable-error {
  color: var(--danger, #b33a3a) !important;
}

.deliverable-cancel { margin-top: 8px; }
.deliverable-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: auto;
  padding-top: 16px;
}

.deliverable-meta {
  color: var(--muted);
  font-size: 9px;
  letter-spacing: 0.08em;
}

@media (max-width: 1100px) {
  .deliverable-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 780px) {
  .deliverable-grid {
    grid-template-columns: 1fr;
  }
}
</style>
