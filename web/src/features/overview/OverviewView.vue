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
import { openAgent } from "@/platform/ui";
import type { ModuleStatus } from "@/platform/types";
import { loadOverview } from "./api";

const router = useRouter();
const { t, locale } = useI18n();
const { data, loading, error, reload } = useResource(loadOverview);

const nextItems = computed(() => [
  { key: "overview.next.asset", path: "/assets", state: "planned" as const },
  { key: "overview.next.heir", path: "/planning", state: "planned" as const },
  { key: "overview.next.checkin", path: "/planning", state: "ready" as const },
]);

const tone = (status: ModuleStatus) => (status === "ready" ? "ready" : status === "degraded" ? "degraded" : "planned");

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(locale.value, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
</script>

<template>
  <div class="overview">
    <PageHeader
      eyebrow="PLATFORM / OVERVIEW"
      :title="t('overview.title')"
      :description="t('overview.description')"
    >
      <template #actions>
        <AppButton variant="secondary" @click="router.push('/automation/approvals')">
          {{ t("overview.action.review") }}
          <AppIcon name="arrow-right" :size="15" />
        </AppButton>
        <AppButton variant="primary" @click="router.push('/planning')">
          {{ t("overview.action.plan") }}
        </AppButton>
      </template>
    </PageHeader>

    <div v-if="loading" class="overview-loading" aria-busy="true">
      <div class="skeleton-hero">
        <SkeletonBlock width="140px" height="12px" />
        <SkeletonBlock width="340px" height="54px" />
        <SkeletonBlock width="540px" height="18px" />
      </div>
      <div class="metrics-grid">
        <div v-for="index in 4" :key="index" class="metric-skeleton">
          <SkeletonBlock width="120px" height="11px" />
          <SkeletonBlock width="90px" height="42px" />
        </div>
      </div>
      <div class="overview-grid">
        <div class="panel-skeleton"><SkeletonBlock width="100%" height="220px" /></div>
        <div class="panel-skeleton"><SkeletonBlock width="100%" height="220px" /></div>
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
      <section class="status-strip">
        <div class="strip-workspace">
          <span class="eyebrow">{{ t("app.workspace") }}</span>
          <strong>{{ data.workspace }}</strong>
        </div>
        <div class="strip-divider" />
        <div class="strip-progress">
          <div class="progress-label">
            <span>{{ t("overview.metric.completion") }}</span>
            <span class="mono">{{ data.completion }}%</span>
          </div>
          <div class="progress-track" aria-hidden="true">
            <span :style="{ width: `${data.completion}%` }" />
          </div>
        </div>
        <div class="strip-state">
          <StatusPill tone="ready" :label="t('app.mode')" />
          <StatusPill tone="planned" :label="t('overview.status.note')" />
        </div>
      </section>

      <section class="metrics-grid" :aria-label="t('overview.eyebrow')">
        <MetricCard
          :label="t('overview.metric.completion')"
          :value="data.completion"
          unit="%"
          :note="t('overview.status.subtitle')"
        >
          <template #icon><AppIcon name="planning" :size="16" /></template>
        </MetricCard>
        <MetricCard
          :label="t('overview.metric.assets')"
          :value="data.assetsTotal"
          :unit="t('overview.unit.items')"
          :note="t('assets.description')"
        >
          <template #icon><AppIcon name="assets" :size="16" /></template>
        </MetricCard>
        <MetricCard
          :label="t('overview.metric.heirs')"
          :value="data.heirsTotal"
          :unit="t('overview.unit.people')"
          :note="t('planning.description')"
        >
          <template #icon><AppIcon name="shield" :size="16" /></template>
        </MetricCard>
        <MetricCard
          :label="t('overview.metric.approvals')"
          :value="data.approvalsPending"
          :unit="t('overview.unit.pending')"
          :note="t('automation.approvals.subtitle')"
        >
          <template #icon><AppIcon name="agent" :size="16" /></template>
        </MetricCard>
      </section>

      <section class="overview-grid">
        <SectionCard :title="t('overview.next.title')" :subtitle="t('overview.next.subtitle')">
          <div class="next-list">
            <button
              v-for="(item, index) in nextItems"
              :key="item.key"
              class="next-row"
              type="button"
              @click="router.push(item.path)"
            >
              <span class="next-index mono">0{{ index + 1 }}</span>
              <span class="next-copy">
                <span class="next-title">{{ t(item.key) }}</span>
                <StatusPill
                  :tone="item.state === 'ready' ? 'ready' : 'planned'"
                  :label="item.state === 'ready' ? t('common.ready') : t('common.pending')"
                />
              </span>
              <AppIcon name="chevron-right" :size="16" />
            </button>
          </div>
          <div class="next-footer">
            <span class="mono">M2 / 3 ITEMS</span>
            <span>{{ t("overview.next.subtitle") }}</span>
          </div>
        </SectionCard>

        <SectionCard :title="t('overview.status.title')" :subtitle="t('overview.status.subtitle')">
          <div class="status-list">
            <div v-for="module in data.modules" :key="module.id" class="status-row">
              <span class="status-id mono">{{ module.id }}</span>
              <span class="status-label">{{ module.label }}</span>
              <StatusPill :tone="tone(module.status)" :label="module.status === 'ready' ? t('common.ready') : t('common.planned')" />
            </div>
          </div>
          <div class="runtime-note">
            <AppIcon name="database" :size="15" />
            <span>{{ t("feature.scaffold.note") }}</span>
          </div>
        </SectionCard>
      </section>

      <section class="lower-grid">
        <SectionCard :title="t('overview.activity.title')" :subtitle="t('overview.activity.subtitle')">
          <div v-if="data.activity.length" class="activity-list">
            <div v-for="item in data.activity" :key="`${item.at}-${item.title}`" class="activity-row">
              <span class="activity-dot" />
              <div>
                <p>{{ item.title }}</p>
                <span class="mono">{{ formatTime(item.at) }}</span>
              </div>
            </div>
          </div>
          <p v-else class="empty-copy">{{ t("overview.activity.empty") }}</p>
        </SectionCard>

        <aside class="agent-card">
          <div class="agent-card-top">
            <span class="agent-mark"><AppIcon name="agent" :size="20" /></span>
            <StatusPill tone="ready" :label="t('overview.agent.badge')" />
          </div>
          <h2>{{ t("overview.agent.title") }}</h2>
          <p>{{ t("overview.agent.subtitle") }}</p>
          <div class="tool-chips">
            <span class="mono">assets.list</span>
            <span class="mono">plan.get</span>
            <span class="mono">security.status</span>
          </div>
          <div class="agent-card-foot">
            <span class="agent-online"><span class="pulse" />{{ t("overview.agent.online") }}</span>
            <AppButton variant="secondary" size="sm" @click="openAgent()">
              {{ t("overview.agent.open") }}
              <AppIcon name="arrow-right" :size="14" />
            </AppButton>
          </div>
        </aside>
      </section>
    </template>
  </div>
</template>

<style scoped>
.overview {
  max-width: var(--content-max);
  margin: 0 auto;
}

.overview-loading {
  display: grid;
  gap: 18px;
}

.skeleton-hero {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px 0 8px;
}

.metrics-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
}

.metric-skeleton {
  display: flex;
  flex-direction: column;
  gap: 26px;
  min-height: 150px;
  padding: 20px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.overview-grid,
.lower-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.35fr) minmax(300px, 0.85fr);
  gap: 16px;
  margin-top: 16px;
}

.panel-skeleton {
  padding: 22px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.status-strip {
  display: flex;
  align-items: center;
  gap: 18px;
  margin-bottom: 14px;
  padding: 16px 20px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--surface) 72%, transparent);
}

.strip-workspace {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 160px;
}

.strip-workspace strong {
  font-family: var(--font-display);
  font-size: 19px;
  font-weight: 500;
}

.strip-divider {
  width: 1px;
  height: 34px;
  background: var(--line);
}

.strip-progress {
  flex: 1;
  min-width: 160px;
}

.progress-label {
  display: flex;
  justify-content: space-between;
  margin-bottom: 7px;
  color: var(--muted-strong);
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.progress-track {
  height: 5px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--surface-strong);
}

.progress-track span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 600ms var(--ease);
}

.strip-state {
  display: flex;
  flex-shrink: 0;
  gap: 8px;
}

.next-list {
  display: flex;
  flex-direction: column;
}

.next-row {
  display: flex;
  align-items: center;
  gap: 14px;
  width: 100%;
  padding: 14px 2px;
  border-top: 1px solid var(--line);
  text-align: left;
  transition: color var(--fast) var(--ease);
}

.next-row:first-child {
  border-top: 0;
}

.next-row:hover {
  color: var(--accent-strong);
}

.next-index {
  color: var(--muted);
  font-size: 10px;
}

.next-copy {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}

.next-title {
  color: var(--ink-soft);
  font-size: 13px;
}

.next-row > :last-child {
  color: var(--muted);
}

.next-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  margin-top: 8px;
  padding-top: 12px;
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: 11px;
}

.status-list {
  display: flex;
  flex-direction: column;
}

.status-row {
  display: grid;
  grid-template-columns: 74px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  padding: 12px 0;
  border-top: 1px solid var(--line);
}

.status-row:first-child {
  border-top: 0;
}

.status-id {
  color: var(--muted);
  font-size: 9px;
}

.status-label {
  color: var(--ink-soft);
  font-size: 12px;
}

.runtime-note {
  display: flex;
  align-items: flex-start;
  gap: 9px;
  margin-top: 14px;
  padding: 12px;
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
  color: var(--muted);
  font-size: 11px;
  line-height: 1.55;
}

.activity-list {
  display: flex;
  flex-direction: column;
}

.activity-row {
  display: flex;
  gap: 13px;
  padding: 12px 0;
  border-top: 1px solid var(--line);
}

.activity-row:first-child {
  border-top: 0;
}

.activity-dot {
  flex-shrink: 0;
  width: 7px;
  height: 7px;
  margin-top: 7px;
  border: 1px solid var(--accent);
  border-radius: 50%;
  background: var(--accent-soft);
}

.activity-row p {
  margin: 0;
  color: var(--ink-soft);
  font-size: 12px;
}

.activity-row span {
  color: var(--muted);
  font-size: 10px;
}

.empty-copy {
  margin: 0;
  color: var(--muted);
  font-size: 12px;
}

.agent-card {
  position: relative;
  display: flex;
  flex-direction: column;
  min-height: 100%;
  padding: 22px;
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--accent) 28%, var(--line));
  border-radius: var(--radius-md);
  background:
    radial-gradient(circle at 100% 0, color-mix(in srgb, var(--accent) 16%, transparent), transparent 55%),
    var(--surface);
  box-shadow: var(--shadow-sm);
}

.agent-card::after {
  position: absolute;
  right: -50px;
  bottom: -60px;
  width: 170px;
  height: 170px;
  border: 1px solid color-mix(in srgb, var(--accent) 20%, transparent);
  border-radius: 50%;
  content: "";
}

.agent-card-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.agent-mark {
  display: grid;
  width: 42px;
  height: 42px;
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
}

.agent-card h2 {
  margin-top: 24px;
  font-size: 24px;
}

.agent-card p {
  max-width: 320px;
  margin: 7px 0 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.6;
}

.tool-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 20px;
}

.tool-chips span {
  padding: 4px 8px;
  border: 1px solid var(--line);
  border-radius: 999px;
  color: var(--muted-strong);
  font-size: 9px;
}

.agent-card-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-top: auto;
  padding-top: 24px;
}

.agent-online {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 9px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.pulse {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 16%, transparent);
}

@media (max-width: 1100px) {
  .metrics-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .overview-grid,
  .lower-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 700px) {
  .status-strip {
    flex-wrap: wrap;
  }

  .strip-divider {
    display: none;
  }

  .strip-state {
    width: 100%;
  }

  .metrics-grid {
    grid-template-columns: 1fr;
  }
}
</style>
