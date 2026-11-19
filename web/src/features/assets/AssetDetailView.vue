<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import AppIcon from "@/shared/icons/AppIcon.vue";
import ErrorState from "@/shared/ui/ErrorState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import SkeletonBlock from "@/shared/ui/SkeletonBlock.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import type { AssetKind } from "@/platform/types";
import { loadAsset } from "./api";

const route = useRoute();
const router = useRouter();
const { t, locale } = useI18n();
const assetId = computed(() => String(route.params.id ?? ""));
const { data, loading, error, reload } = useResource(() => loadAsset(assetId.value));
const kindKey = (kind: AssetKind) => `asset.kind.${kind}`;

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(locale.value, { year: "numeric", month: "2-digit", day: "2-digit" }).format(date);
}
</script>

<template>
  <div class="detail-page">
    <button class="back-link" type="button" @click="router.push('/assets')">
      <AppIcon name="chevron-right" :size="14" class="back-icon" />
      {{ t("asset.detail.back") }}
    </button>

    <div v-if="loading" class="detail-loading">
      <SkeletonBlock width="120px" height="12px" />
      <SkeletonBlock width="420px" height="48px" />
      <SkeletonBlock width="100%" height="240px" />
    </div>

    <ErrorState
      v-else-if="error"
      :title="t('empty.title')"
      :message="error"
      :retry-label="t('common.retry')"
      @retry="reload"
    />

    <template v-else-if="data">
      <PageHeader :eyebrow="`ASSET / ${data.id}`" :title="data.name" :description="data.value">
        <template #actions>
          <StatusPill
            :tone="data.status === 'active' ? 'ready' : 'planned'"
            :label="data.status === 'active' ? t('common.active') : t('common.archived')"
          />
          <StatusPill tone="planned" :label="t('app.mode')" />
        </template>
      </PageHeader>

      <div class="detail-grid">
        <SectionCard :title="t('asset.detail.basic')" :subtitle="t('asset.detail.readonly')">
          <dl class="detail-list">
            <div>
              <dt>{{ t("assets.table.kind") }}</dt>
              <dd class="mono">{{ t(kindKey(data.kind)) }}</dd>
            </div>
            <div>
              <dt>{{ t("assets.table.platform") }}</dt>
              <dd>{{ data.platform }}</dd>
            </div>
            <div>
              <dt>{{ t("assets.table.status") }}</dt>
              <dd>{{ data.status === "active" ? t("common.active") : t("common.archived") }}</dd>
            </div>
            <div>
              <dt>{{ t("assets.table.updated") }}</dt>
              <dd class="mono">{{ formatDate(data.updatedAt) }}</dd>
            </div>
          </dl>
        </SectionCard>

        <SectionCard :title="t('asset.detail.value')" :subtitle="t('asset.detail.note')">
          <p class="note-body">{{ data.value || "—" }}</p>
          <div class="readonly-note">
            <AppIcon name="lock" :size="15" />
            <span>{{ t("asset.detail.readonly") }}</span>
          </div>
        </SectionCard>
      </div>

    </template>
  </div>
</template>

<style scoped>
.detail-page {
  max-width: 1100px;
  margin: 0 auto;
}

.back-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 22px;
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.back-link:hover {
  color: var(--accent-strong);
}

.back-icon {
  transform: rotate(180deg);
}

.detail-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  margin-bottom: 16px;
}

.detail-list {
  display: grid;
  gap: 0;
  margin: 0;
}

.detail-list > div {
  display: flex;
  justify-content: space-between;
  gap: 20px;
  padding: 13px 0;
  border-top: 1px solid var(--line);
}

.detail-list > div:first-child {
  border-top: 0;
}

.detail-list dt {
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.detail-list dd {
  margin: 0;
  color: var(--ink-soft);
  font-size: 12px;
  text-align: right;
}

.note-body {
  min-height: 100px;
  margin: 0;
  color: var(--ink-soft);
  font-size: 13px;
  line-height: 1.8;
}

.readonly-note {
  display: flex;
  gap: 8px;
  margin-top: 18px;
  padding: 11px 12px;
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
  color: var(--muted);
  font-size: 11px;
}

.detail-loading {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

@media (max-width: 800px) {
  .detail-grid {
    grid-template-columns: 1fr;
  }
}
</style>
