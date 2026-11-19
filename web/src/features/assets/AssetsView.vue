<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import EmptyState from "@/shared/ui/EmptyState.vue";
import ErrorState from "@/shared/ui/ErrorState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import AssetTabs from "./AssetTabs.vue";
import SkeletonBlock from "@/shared/ui/SkeletonBlock.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import type { Asset, AssetKind } from "@/platform/types";
import { loadAssets } from "./api";

const router = useRouter();
const { t, locale } = useI18n();
const query = ref("");
const status = ref<"all" | "active" | "archived">("all");
const loadingMore = ref(false);

const loadPage = () =>
  loadAssets({
    status: status.value,
    limit: 50,
    ...(query.value.trim() ? { q: query.value.trim() } : {}),
  });
const { data, loading, error, reload } = useResource(loadPage);
const items = computed(() => data.value?.items ?? []);
const total = computed(() => data.value?.total ?? items.value.length);
const nextCursor = computed(() => data.value?.nextCursor ?? null);

let timer: ReturnType<typeof setTimeout> | undefined;
watch([query, status], () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => reload(), 250);
});

async function loadMore() {
  if (!nextCursor.value || loadingMore.value) return;
  loadingMore.value = true;
  const envelope = await loadAssets({
    status: status.value,
    limit: 50,
    cursor: nextCursor.value,
  });
  loadingMore.value = false;
  if (!envelope.success || !envelope.data) return;
  data.value = {
    ...envelope.data,
    items: [...items.value, ...envelope.data.items],
  };
}

const kindKey = (kind: AssetKind) => `asset.kind.${kind}`;

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat(locale.value, { year: "numeric", month: "2-digit", day: "2-digit" }).format(date);
}

function openAsset(asset: Asset) {
  router.push(`/assets/${asset.id}`);
}
</script>

<template>
  <div class="assets-page">
    <PageHeader eyebrow="ASSETS / REGISTRY" :title="t('assets.title')" :description="t('assets.description')">
      <template #actions>
        <StatusPill tone="ready" :label="t('app.mode')" />
      </template>
    </PageHeader>

    <AssetTabs />

    <div v-if="loading" class="assets-loading" aria-busy="true">
      <div class="toolbar-skeleton">
        <SkeletonBlock width="280px" height="36px" />
        <SkeletonBlock width="120px" height="36px" />
      </div>
      <div v-for="index in 6" :key="index" class="row-skeleton">
        <SkeletonBlock width="28%" height="16px" />
        <SkeletonBlock width="12%" height="16px" />
        <SkeletonBlock width="18%" height="16px" />
        <SkeletonBlock width="10%" height="16px" />
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
      <div class="asset-toolbar">
        <label class="search-field">
          <AppIcon name="search" :size="16" />
          <input v-model="query" type="search" :placeholder="t('assets.search')" />
        </label>
        <label class="select-field">
          <span>{{ t('assets.filter') }}</span>
          <select v-model="status">
            <option value="all">{{ t("common.all") }}</option>
            <option value="active">{{ t("common.active") }}</option>
            <option value="archived">{{ t("common.archived") }}</option>
          </select>
        </label>
        <span class="asset-count mono">{{ t("assets.count", { count: total }) }}</span>
      </div>

      <div v-if="items.length" class="asset-table-wrap">
        <table class="asset-table">
          <thead>
            <tr>
              <th>{{ t("assets.table.name") }}</th>
              <th>{{ t("assets.table.kind") }}</th>
              <th>{{ t("assets.table.platform") }}</th>
              <th>{{ t("assets.table.status") }}</th>
              <th>{{ t("assets.table.updated") }}</th>
              <th><span class="sr-only">{{ t("common.open") }}</span></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="asset in items" :key="asset.id" tabindex="0" @click="openAsset(asset)" @keydown.enter="openAsset(asset)">
              <td>
                <div class="asset-name">
                  <span class="asset-glyph"><AppIcon name="assets" :size="15" /></span>
                  <span>
                    <strong>{{ asset.name }}</strong>
                    <small>{{ asset.value }}</small>
                  </span>
                </div>
              </td>
              <td><span class="mono kind">{{ t(kindKey(asset.kind)) }}</span></td>
              <td>{{ asset.platform }}</td>
              <td>
                <StatusPill
                  :tone="asset.status === 'active' ? 'ready' : 'planned'"
                  :label="asset.status === 'active' ? t('common.active') : t('common.archived')"
                />
              </td>
              <td><span class="mono date">{{ formatDate(asset.updatedAt) }}</span></td>
              <td><AppIcon name="chevron-right" :size="15" /></td>
            </tr>
          </tbody>
        </table>
      </div>

      <div v-if="nextCursor" class="load-more">
        <AppButton variant="secondary" size="sm" :disabled="loadingMore" @click="loadMore">
          {{ loadingMore ? t("common.loading") : t("common.loadMore") }}
        </AppButton>
      </div>

      <EmptyState v-if="!items.length" :title="t('assets.table.empty')" :note="t('empty.note')" icon="assets">
        <template #action>
          <AppButton variant="secondary" size="sm" @click="query = ''; status = 'all'">{{ t("common.all") }}</AppButton>
        </template>
      </EmptyState>
    </template>
  </div>
</template>

<style scoped>
.assets-page {
  max-width: var(--content-max);
  margin: 0 auto;
}

.asset-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 14px;
}

.search-field {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 9px;
  max-width: 480px;
  min-height: 38px;
  padding: 0 13px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--surface);
  color: var(--muted);
}

.search-field input {
  width: 100%;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--ink);
  font-size: 13px;
}

.search-field input::placeholder {
  color: var(--muted);
}

.select-field {
  display: flex;
  align-items: center;
  gap: 9px;
  min-height: 38px;
  padding: 0 14px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--surface);
  color: var(--muted-strong);
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.select-field select {
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 12px;
  text-transform: none;
}

.asset-count {
  margin-left: auto;
  color: var(--muted);
  font-size: 10px;
}

.asset-table-wrap {
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.asset-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.asset-table thead th {
  padding: 14px 18px;
  border-bottom: 1px solid var(--line);
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.asset-table tbody tr {
  border-bottom: 1px solid var(--line);
  cursor: pointer;
  transition: background var(--fast) var(--ease);
}

.asset-table tbody tr:last-child {
  border-bottom: 0;
}

.asset-table tbody tr:hover {
  background: var(--surface-soft);
}

.asset-table td {
  padding: 14px 18px;
  color: var(--ink-soft);
  font-size: 12px;
}

.asset-name {
  display: flex;
  align-items: center;
  gap: 10px;
}

.asset-glyph {
  display: grid;
  flex-shrink: 0;
  width: 30px;
  height: 30px;
  border: 1px solid var(--line);
  border-radius: 8px;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
}

.asset-name strong {
  display: block;
  color: var(--ink);
  font-size: 13px;
  font-weight: 600;
}

.asset-name small {
  display: block;
  max-width: 340px;
  margin-top: 2px;
  overflow: hidden;
  color: var(--muted);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.kind,
.date {
  color: var(--muted-strong);
  font-size: 10px;
}

.assets-loading {
  display: grid;
  gap: 12px;
}

.toolbar-skeleton {
  display: flex;
  gap: 12px;
  margin-bottom: 4px;
}

.row-skeleton {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  padding: 20px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface);
}

@media (max-width: 760px) {
  .asset-toolbar {
    flex-wrap: wrap;
  }

  .asset-count {
    width: 100%;
    margin-left: 0;
  }

  .asset-table-wrap {
    overflow-x: auto;
  }

  .asset-table {
    min-width: 720px;
  }
}
.load-more {
  display: flex;
  justify-content: center;
  padding: 16px 0 4px;
}

</style>
