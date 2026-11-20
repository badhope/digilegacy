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
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import { loadOverview } from "./api";

const router = useRouter();
const { t, locale } = useI18n();
const { data, loading, error, reload } = useResource(loadOverview);

const nextItems = computed(() => {
  if (!data.value) return [];
  const items: Array<{ key: string; path: string }> = [];
  if (data.value.approvalsPending > 0) {
    items.push({ key: "overview.next.approval", path: "/automation?tab=approvals" });
  }
  if (data.value.assetsTotal === 0) {
    items.push({ key: "overview.next.asset", path: "/assets" });
  }
  if (data.value.heirsTotal === 0) {
    items.push({ key: "overview.next.heir", path: "/planning" });
  }
  if (items.length < 3) {
    items.push({ key: "overview.next.checkin", path: "/planning" });
  }
  return items.slice(0, 3);
});

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
        <AppButton variant="secondary" @click="router.push('/automation?tab=approvals')">
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
        <span class="strip-note">{{ t("overview.status.note") }}</span>
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
              </span>
              <AppIcon name="chevron-right" :size="16" />
            </button>
          </div>
        </SectionCard>

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

.overview-grid {
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

.strip-note {
  margin-left: auto;
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 9px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
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

@media (max-width: 1100px) {
  .metrics-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .overview-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 700px) {
  .status-strip {
    flex-wrap: wrap;
  }

  .metrics-grid {
    grid-template-columns: 1fr;
  }
}
</style>
