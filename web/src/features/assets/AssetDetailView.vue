<script setup lang="ts">
import { computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import ErrorState from "@/shared/ui/ErrorState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import SkeletonBlock from "@/shared/ui/SkeletonBlock.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { formatDate } from "@/shared/format";
import { useResource } from "@/platform/useResource";
import AssetEditor from "./AssetEditor.vue";
import { assetKindLabel, formatAssetValue, type AssetDraft } from "./model";
import { loadAsset } from "./api";
import { platformClient } from "@/platform/client";

const route = useRoute();
const router = useRouter();
const { t, locale } = useI18n();
const assetId = computed(() => String(route.params.id ?? ""));
const { data, loading, error, reload } = useResource(() => loadAsset(assetId.value));
const editing = ref(false);
const saving = ref(false);
const saveError = ref("");
const saveNotice = ref("");
const statusSaving = ref(false);
const statusError = ref("");

function startEdit() {
  if (!data.value) return;
  saveError.value = "";
  saveNotice.value = "";
  editing.value = true;
}

async function saveEdit(draft: AssetDraft) {
  if (!data.value) return;
  saveError.value = "";
  saveNotice.value = "";
  saving.value = true;
  try {
    const envelope = await platformClient.updateAsset({ id: data.value.id, ...draft });
    if (!envelope.success || !envelope.data) {
      saveError.value = envelope.code === "AUTH_REQUIRED" ? t("assets.create.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    data.value = envelope.data;
    editing.value = false;
    saveNotice.value = t("asset.detail.saved");
  } finally {
    saving.value = false;
  }
}

async function toggleStatus() {
  const asset = data.value;
  if (!asset || statusSaving.value) return;
  const next = asset.status === "active" ? "archived" : "active";
  statusError.value = "";
  saveNotice.value = "";
  statusSaving.value = true;
  try {
    const envelope = await platformClient.updateAsset({
      id: asset.id,
      name: asset.name,
      kind: asset.kind,
      platform: asset.platform,
      value: asset.value,
      note: asset.note,
      status: next,
    });
    if (!envelope.success || !envelope.data) {
      statusError.value =
        envelope.code === "AUTH_REQUIRED"
          ? t("assets.create.tokenRequired")
          : envelope.message || envelope.code;
      return;
    }
    data.value = envelope.data;
    saveNotice.value =
      next === "archived" ? t("asset.detail.archivedNotice") : t("asset.detail.restoredNotice");
  } finally {
    statusSaving.value = false;
  }
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
      <PageHeader
        :eyebrow="`ASSET / ${data.id}`"
        :title="data.name"
        :description="formatAssetValue(data.value, locale)"
      >
        <template #actions>
          <AppButton v-if="!editing" variant="primary" size="sm" @click="startEdit">{{ t("assets.edit.open") }}</AppButton>
          <AppButton
            v-if="data.platform && data.platform !== '未分类'"
            variant="secondary"
            size="sm"
            @click="router.push({ path: '/policies', query: { q: data.platform } })"
          >
            <AppIcon name="database" :size="14" />
            {{ t("asset.detail.policy") }}
          </AppButton>
          <StatusPill
            :tone="data.status === 'active' ? 'ready' : 'planned'"
            :label="data.status === 'active' ? t('common.active') : t('common.archived')"
          />
        </template>
      </PageHeader>

      <div v-if="statusError" class="detail-error" role="alert">{{ statusError }}</div>
      <div v-else-if="saveNotice" class="detail-notice" role="status">{{ saveNotice }}</div>

      <AssetEditor
        v-if="editing"
        mode="edit"
        :initial="data"
        :saving="saving"
        :error="saveError"
        @submit="saveEdit"
        @cancel="editing = false"
      />

      <div class="detail-grid">
        <SectionCard :title="t('asset.detail.basic')" :subtitle="t('asset.detail.basic.note')">
          <dl class="detail-list">
            <div>
              <dt>{{ t("assets.table.kind") }}</dt>
              <dd class="mono">{{ assetKindLabel(data.kind, locale) }}</dd>
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
              <dd class="mono">{{ formatDate(data.updatedAt, locale) }}</dd>
            </div>
          </dl>

          <div v-if="!editing" class="asset-lifecycle">
            <div>
              <strong>{{ t("asset.detail.lifecycle") }}</strong>
              <p>
                {{ data.status === "active" ? t("asset.detail.archiveNote") : t("asset.detail.restoreNote") }}
              </p>
            </div>
            <AppButton
              variant="secondary"
              size="sm"
              :disabled="statusSaving"
              @click="toggleStatus"
            >
              {{
                statusSaving
                  ? t("assets.create.saving")
                  : data.status === "active"
                    ? t("asset.detail.archive")
                    : t("asset.detail.restore")
              }}
            </AppButton>
          </div>
        </SectionCard>

        <SectionCard :title="t('asset.detail.value')" :subtitle="t('asset.detail.note')">
          <p class="note-body">{{ data.note || "—" }}</p>
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

.detail-notice,
.detail-error {
  margin: 0 0 14px;
  padding: 9px 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
  color: var(--ink-soft);
  font-size: 12px;
}

.detail-error {
  border-color: color-mix(in srgb, var(--danger, #b42318) 45%, var(--line));
  color: var(--danger, #b42318);
}

.asset-lifecycle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-top: 16px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
}

.asset-lifecycle strong {
  display: block;
  margin-bottom: 4px;
  font-size: 12px;
}

.asset-lifecycle p {
  margin: 0;
  color: var(--muted);
  font-size: 11px;
  line-height: 1.6;
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

@media (max-width: 560px) {
  .asset-lifecycle {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
