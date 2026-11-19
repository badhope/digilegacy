<script setup lang="ts">
import AppIcon from "@/shared/icons/AppIcon.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import type { IconName } from "@/shared/icons/paths";

const { t } = useI18n();

const kinds: Array<{ key: string; icon: IconName; tone: "planned" | "ready" }> = [
  { key: "deliverables.kind.report", icon: "deliverables", tone: "planned" },
  { key: "deliverables.kind.checklist", icon: "check", tone: "planned" },
  { key: "deliverables.kind.legal", icon: "shield", tone: "planned" },
  { key: "deliverables.kind.toolkit", icon: "assets", tone: "planned" },
];
</script>

<template>
  <div class="deliverables-page">
    <PageHeader eyebrow="DELIVERABLES / EXPORT" :title="t('deliverables.title')" :description="t('deliverables.description')">
      <template #actions>
        <StatusPill tone="planned" :label="t('app.mode')" />
      </template>
    </PageHeader>

    <div class="deliverable-grid">
      <article v-for="kind in kinds" :key="kind.key" class="deliverable-card">
        <div class="deliverable-top">
          <span class="deliverable-icon"><AppIcon :name="kind.icon" :size="19" /></span>
          <StatusPill :tone="kind.tone" :label="t('deliverables.status.contract')" />
        </div>
        <h2>{{ t(kind.key) }}</h2>
        <p>{{ t("deliverables.readiness") }}</p>
        <div class="deliverable-meta mono">M0 / CONTRACT</div>
      </article>
    </div>

    <div class="deliverable-lower">
      <SectionCard :title="t('deliverables.quality')" :subtitle="t('feature.scaffold.note')">
        <div class="queue-row">
          <span class="queue-id mono">REV-01</span>
          <span>{{ t("deliverables.quality") }}</span>
          <StatusPill tone="planned" :label="t('common.planned')" />
        </div>
        <div class="queue-row">
          <span class="queue-id mono">REV-02</span>
          <span>{{ t("deliverables.drills") }}</span>
          <StatusPill tone="planned" :label="t('common.planned')" />
        </div>
      </SectionCard>

      <SectionCard :title="t('deliverables.drills')" :subtitle="t('overview.next.checkin')">
        <div class="drill-callout">
          <AppIcon name="clock" :size="17" />
          <div>
            <strong>{{ t("overview.next.checkin") }}</strong>
            <p>{{ t("deliverables.readiness") }}</p>
          </div>
        </div>
        <div class="drill-line">
          <span class="mono">DRILL-01</span>
          <StatusPill tone="planned" :label="t('common.planned')" />
        </div>
      </SectionCard>
    </div>
  </div>
</template>

<style scoped>
.deliverables-page {
  max-width: var(--content-max);
  margin: 0 auto;
}

.deliverable-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
}

.deliverable-card {
  display: flex;
  flex-direction: column;
  min-height: 210px;
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
  margin-top: 20px;
  font-size: 20px;
}

.deliverable-card p {
  margin: 7px 0 0;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.6;
}

.deliverable-meta {
  margin-top: auto;
  padding-top: 18px;
  color: var(--muted);
  font-size: 9px;
  letter-spacing: 0.08em;
}

.deliverable-lower {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
  margin-top: 14px;
}

.queue-row,
.drill-line {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 0;
  border-top: 1px solid var(--line);
  color: var(--ink-soft);
  font-size: 12px;
}

.queue-row:first-child,
.drill-line:first-child {
  border-top: 0;
}

.queue-id {
  min-width: 56px;
  color: var(--muted);
  font-size: 9px;
}

.queue-row > span:nth-child(2) {
  flex: 1;
}

.drill-callout {
  display: flex;
  gap: 12px;
  padding: 14px;
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
  color: var(--muted-strong);
}

.drill-callout strong {
  display: block;
  color: var(--ink);
  font-size: 12px;
}

.drill-callout p {
  margin: 4px 0 0;
  color: var(--muted);
  font-size: 11px;
}

@media (max-width: 1100px) {
  .deliverable-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 780px) {
  .deliverable-grid,
  .deliverable-lower {
    grid-template-columns: 1fr;
  }
}
</style>
