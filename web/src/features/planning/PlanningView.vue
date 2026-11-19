<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import ErrorState from "@/shared/ui/ErrorState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import SkeletonBlock from "@/shared/ui/SkeletonBlock.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import { platformClient } from "@/platform/client";
import { loadPlan } from "./api";

const router = useRouter();
const { t, locale } = useI18n();
const { data, loading, error, reload, refresh } = useResource(loadPlan);

const heirCount = computed(() => data.value?.heirs.length ?? 0);
const triggerMode = computed(() => {
  const mode = data.value?.trigger.mode ?? "";
  return mode === "dms" ? t("planning.trigger.mode.dms") : mode;
});
const triggerStatus = computed(() => {
  const status = data.value?.trigger.status ?? "";
  const key = `planning.trigger.status.${status}`;
  return t(key) === key ? status : t(key);
});

function formatDate(value: string): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(locale.value, { year: "numeric", month: "2-digit", day: "2-digit" }).format(date);
}

const acting = ref(false);
const actionError = ref("");
const notice = ref("");

async function checkIn() {
  acting.value = true;
  actionError.value = "";
  notice.value = "";
  try {
    const envelope = await platformClient.checkin();
    if (!envelope.success || !envelope.data) {
      actionError.value = envelope.code === "AUTH_REQUIRED" ? t("runs.detail.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    notice.value = t("planning.checkin.done", { date: formatDate(envelope.data.checkedInAt) });
    await refresh();
  } catch (cause) {
    actionError.value = cause instanceof Error ? cause.message : "check-in failed";
  } finally {
    acting.value = false;
  }
}
</script>

<template>
  <div class="planning-page">
    <PageHeader eyebrow="PLANNING / PEOPLE & TRIGGER" :title="t('planning.title')" :description="t('planning.description')">
      <template #actions>
        <StatusPill tone="ready" :label="t('app.mode')" />
      </template>
    </PageHeader>

    <div v-if="loading" class="planning-loading">
      <div v-for="index in 3" :key="index" class="plan-skeleton">
        <SkeletonBlock width="100px" height="11px" />
        <SkeletonBlock width="220px" height="28px" />
        <SkeletonBlock width="100%" height="60px" />
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
      <div class="planning-grid">
        <SectionCard :title="t('planning.holder')" :subtitle="t('planning.updated')">
          <div class="holder-block">
            <span class="holder-mark"><AppIcon name="shield" :size="21" /></span>
            <div>
              <h3>{{ data.holder.name || t("planning.holder.empty") }}</h3>
              <p class="mono">{{ formatDate(data.holder.updatedAt) }}</p>
            </div>
          </div>
          <div class="plan-line">
            <span>{{ t("settings.workspace.mode") }}</span>
            <StatusPill tone="ready" :label="t('app.mode')" />
          </div>
        </SectionCard>

        <SectionCard :title="t('planning.heirs')" :subtitle="`${heirCount} ${t('overview.unit.people')}`">
          <div v-if="!heirCount" class="heir-empty">{{ t("planning.heirs.empty") }}</div>
          <div v-else class="heir-list">
            <div v-for="heir in data.heirs" :key="heir.id" class="heir-row">
              <span class="heir-avatar">{{ heir.name.slice(0, 1) }}</span>
              <span class="heir-copy">
                <strong>{{ heir.name }}</strong>
                <small>{{ heir.role }}</small>
              </span>
            </div>
          </div>
        </SectionCard>

        <SectionCard :title="t('planning.trigger')" :subtitle="t('planning.trigger.status')">
          <div class="trigger-chain">
            <div class="trigger-node active">
              <span class="node-dot" />
              <span>{{ triggerMode }}</span>
            </div>
            <span class="trigger-line" />
            <div class="trigger-node">
              <span class="node-dot" />
              <span>{{ t("planning.trigger.days") }} · {{ data.trigger.days }}</span>
            </div>
            <span class="trigger-line" />
            <div class="trigger-node">
              <span class="node-dot" />
              <span>{{ triggerStatus }}</span>
            </div>
          </div>
          <div class="trigger-meta">
            <div>
              <span>{{ t("planning.checkin.last") }}</span>
              <strong class="mono">{{ formatDate(data.trigger.lastCheckIn) }}</strong>
            </div>
            <div>
              <span>{{ t("planning.checkin.next") }}</span>
              <strong class="mono">{{ formatDate(data.trigger.nextCheckAt) }}</strong>
            </div>
          </div>
          <div class="trigger-foot">
            <AppIcon name="clock" :size="15" />
            <span>{{ t("planning.trigger.note") }}</span>
          </div>
          <div class="trigger-action">
            <AppButton variant="primary" size="sm" :disabled="acting" @click="checkIn">
              {{ acting ? t("planning.checkin.acting") : t("planning.checkin.action") }}
              <AppIcon name="check" :size="14" />
            </AppButton>
            <span v-if="notice" class="trigger-notice">{{ notice }}</span>
            <span v-else-if="actionError" class="trigger-error">{{ actionError }}</span>
          </div>
        </SectionCard>
      </div>

      <div class="planning-note">
        <AppButton variant="ghost" size="sm" @click="router.push('/deliverables')">
          {{ t("nav.deliverables") }}
          <AppIcon name="arrow-right" :size="14" />
        </AppButton>
      </div>
    </template>
  </div>
</template>

<style scoped>
.planning-page {
  max-width: var(--content-max);
  margin: 0 auto;
}

.planning-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

.holder-block {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 10px 0 20px;
}

.holder-mark {
  display: grid;
  width: 48px;
  height: 48px;
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
}

.holder-block h3 {
  font-size: 22px;
}

.holder-block p {
  margin: 3px 0 0;
  color: var(--muted);
  font-size: 10px;
}

.plan-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: 11px;
}

.heir-list {
  display: flex;
  flex-direction: column;
}

.heir-row {
  display: flex;
  align-items: center;
  gap: 11px;
  width: 100%;
  padding: 11px 0;
  border-top: 1px solid var(--line);
}

.heir-row:first-child {
  border-top: 0;
}

.heir-avatar {
  display: grid;
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  color: var(--accent-strong);
  font-family: var(--font-display);
  font-size: 15px;
  place-items: center;
  background: var(--surface-soft);
}

.heir-copy {
  flex: 1;
}

.heir-copy strong {
  display: block;
  color: var(--ink);
  font-size: 13px;
  font-weight: 600;
  transition: color var(--fast) var(--ease);
}

.heir-copy small {
  display: block;
  margin-top: 2px;
  color: var(--muted);
  font-size: 10px;
}

.trigger-chain {
  display: flex;
  flex-direction: column;
  gap: 0;
  padding: 8px 0 4px;
}

.trigger-node {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--muted-strong);
  font-size: 12px;
}

.trigger-node.active {
  color: var(--ink);
  font-weight: 600;
}

.node-dot {
  width: 8px;
  height: 8px;
  border: 1px solid var(--line-strong);
  border-radius: 50%;
  background: var(--surface);
}

.trigger-node.active .node-dot {
  border-color: var(--accent);
  background: var(--accent);
  box-shadow: 0 0 0 4px var(--accent-soft);
}

.trigger-line {
  width: 1px;
  height: 22px;
  margin-left: 3px;
  background: var(--line);
}

.trigger-meta { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; margin-top: 16px; padding-top: 12px; border-top: 1px solid var(--line); }
.trigger-meta > div { display: grid; gap: 3px; }
.trigger-meta span { color: var(--muted); font-size: 10px; text-transform: uppercase; letter-spacing: .04em; }
.trigger-meta strong { font-size: 12px; font-weight: 500; }
.trigger-action { display: flex; align-items: center; gap: 10px; margin-top: 12px; flex-wrap: wrap; }
.trigger-notice { color: var(--accent-strong); font-size: 11px; }
.trigger-error { color: var(--danger, #b54949); font-size: 11px; }
.trigger-foot {
  display: flex;
  gap: 8px;
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: 11px;
  line-height: 1.55;
}

.planning-note {
  display: flex;
  justify-content: flex-end;
  margin-top: 12px;
}

.planning-loading {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

.plan-skeleton {
  display: flex;
  flex-direction: column;
  gap: 18px;
  min-height: 220px;
  padding: 22px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
}

@media (max-width: 1000px) {
  .planning-grid,
  .planning-loading {
    grid-template-columns: 1fr;
  }
}
.heir-empty {
  padding: 18px 0;
  color: var(--muted);
  font-size: 13px;
}

</style>
