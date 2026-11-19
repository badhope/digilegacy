<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useRouter } from "vue-router";
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
import type { Approval } from "@/platform/types";

const router = useRouter();
const { t, locale } = useI18n();
const filter = ref<"pending" | "approved" | "rejected" | "all">("pending");
const reasons = reactive<Record<string, string>>({});
const deciding = ref("");
const decideError = ref("");
const decidedRun = ref("");
const decidedJob = ref("");
const lastDecision = ref<"approve" | "reject">("approve");

const { data, loading, error, reload } = useResource(() =>
  platformClient.listApprovals({ status: filter.value }),
);

const items = computed(() => data.value?.items ?? []);
const pending = computed(() => data.value?.pending ?? 0);

function statusTone(status: Approval["status"]): "ready" | "planned" | "degraded" | "danger" | "neutral" {
  if (status === "approved") return "ready";
  if (status === "rejected") return "danger";
  if (status === "pending") return "degraded";
  return "neutral";
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(locale.value, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function prettyDiff(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value || "—";
  }
}

async function decide(approval: Approval, decision: "approve" | "reject") {
  if (deciding.value) return;
  decideError.value = "";
  deciding.value = approval.id;
  try {
    const envelope = await platformClient.decideApproval(
      approval.id,
      decision,
      reasons[approval.id] ?? "",
    );
    if (!envelope.success || !envelope.data) {
      decideError.value =
        envelope.code === "AUTH_REQUIRED"
          ? t("approvals.tokenRequired")
          : envelope.message || envelope.code;
      return;
    }
    reasons[approval.id] = "";
    decidedRun.value = approval.runId;
    decidedJob.value = envelope.data.jobId ?? "";
    lastDecision.value = decision;
    await reload();
  } finally {
    deciding.value = "";
  }
}
</script>

<template>
  <div class="approvals-page">
    <PageHeader eyebrow="AUTOMATION / APPROVALS" :title="t('automation.approvals.title')" :description="t('approvals.description')">
      <template #actions>
        <AppButton variant="secondary" size="sm" @click="router.push('/automation')">
          <AppIcon name="arrow-right" :size="14" class="back-arrow" />
          {{ t("approvals.back") }}
        </AppButton>
        <StatusPill :tone="pending ? 'degraded' : 'ready'" :label="t('approvals.pendingCount', { count: pending })" />
      </template>
    </PageHeader>

    <div class="approval-toolbar">
      <label class="filter-field">
        <span>{{ t("approvals.filter") }}</span>
        <select v-model="filter" @change="reload()">
          <option value="pending">{{ t("approvals.status.pending") }}</option>
          <option value="approved">{{ t("approvals.status.approved") }}</option>
          <option value="rejected">{{ t("approvals.status.rejected") }}</option>
          <option value="all">{{ t("common.all") }}</option>
        </select>
      </label>
      <span class="mono count">{{ t("approvals.count", { count: items.length }) }}</span>
    </div>

    <div v-if="loading" class="approval-loading">
      <div v-for="index in 3" :key="index" class="approval-skeleton">
        <SkeletonBlock width="100%" height="120px" />
      </div>
    </div>

    <ErrorState
      v-else-if="error"
      :title="t('error.generic')"
      :message="error"
      :retry-label="t('common.retry')"
      @retry="reload"
    />

    <template v-else>
      <div v-if="decideError" class="decide-error">{{ decideError }}</div>

      <div v-if="decidedRun" class="decide-note">
        <span>{{ lastDecision === "approve" ? t("approvals.background") : t("approvals.rejectedNote") }}</span>
        <span v-if="decidedJob" class="mono">{{ decidedJob }}</span>
        <AppButton variant="secondary" size="sm" @click="router.push(`/automation/runs/${decidedRun}`)">
          {{ t("approvals.viewRun") }}
        </AppButton>
      </div>

      <div v-if="items.length" class="approval-list">
        <article v-for="approval in items" :key="approval.id" class="approval-card">
          <header class="approval-head">
            <div>
              <span class="mono approval-id">{{ approval.id }}</span>
              <h2 class="mono">{{ approval.tool }}</h2>
            </div>
            <StatusPill :tone="statusTone(approval.status)" :label="t(`approvals.status.${approval.status}`)" />
          </header>

          <div class="approval-meta mono">
            <span>{{ t("approvals.run") }} {{ approval.runId }}</span>
            <span v-if="approval.objectKey">{{ t("approvals.object") }} {{ approval.objectKey }}</span>
            <span>{{ formatDate(approval.createdAt) }}</span>
          </div>

          <pre class="approval-diff">{{ prettyDiff(approval.diff) }}</pre>

          <div v-if="approval.status === 'pending'" class="approval-actions">
            <input
              v-model="reasons[approval.id]"
              type="text"
              maxlength="500"
              :placeholder="t('approvals.reasonPlaceholder')"
            />
            <AppButton
              variant="primary"
              size="sm"
              :disabled="deciding === approval.id"
              @click="decide(approval, 'approve')"
            >
              {{ t("approvals.approve") }}
            </AppButton>
            <AppButton
              variant="secondary"
              size="sm"
              :disabled="deciding === approval.id"
              @click="decide(approval, 'reject')"
            >
              {{ t("approvals.reject") }}
            </AppButton>
          </div>
          <div v-else-if="approval.reason" class="decided-reason">
            {{ t("approvals.reason") }}: {{ approval.reason }}
          </div>
        </article>
      </div>

      <EmptyState v-else :title="t('approvals.empty')" :note="t('approvals.emptyNote')" icon="check">
        <template #action>
          <AppButton variant="secondary" size="sm" @click="filter = 'all'; reload()">{{ t("common.all") }}</AppButton>
        </template>
      </EmptyState>
    </template>
  </div>
</template>

<style scoped>
.back-arrow { transform: rotate(180deg); }
.approvals-page { max-width: var(--content-max); margin: 0 auto; }
.approval-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 14px; }
.filter-field { display: flex; align-items: center; gap: 9px; min-height: 36px; padding: 0 12px; border: 1px solid var(--line); border-radius: 999px; background: var(--surface); color: var(--muted); font-size: 12px; }
.filter-field select { border: 0; outline: 0; background: transparent; color: var(--ink); font: inherit; font-size: 12px; }
.count { color: var(--muted); font-size: 11px; }
.approval-loading { display: grid; gap: 12px; }
.approval-skeleton { padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.approval-list { display: grid; gap: 12px; }
.approval-card { padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.approval-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.approval-id { color: var(--muted); font-size: 10px; }
.approval-head h2 { margin: 4px 0 0; font-size: 15px; }
.approval-meta { display: flex; flex-wrap: wrap; gap: 14px; margin: 10px 0; color: var(--muted); font-size: 10px; }
.approval-diff { max-height: 180px; overflow: auto; margin: 0; padding: 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); color: var(--ink-soft); font-size: 11px; line-height: 1.5; }
.approval-actions { display: flex; align-items: center; gap: 8px; margin-top: 12px; }
.approval-actions input { flex: 1; min-width: 0; padding: 8px 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); color: var(--ink); font: inherit; font-size: 12px; }
.decided-reason { margin-top: 10px; color: var(--muted); font-size: 11px; }
.decide-note { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); color: var(--ink-soft); font-size: 12px; }
.decide-note .mono { color: var(--muted); font-size: 10px; }
.decide-error { margin-bottom: 12px; padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); color: var(--danger, #b42318); background: var(--surface-soft); font-size: 12px; }
</style>
