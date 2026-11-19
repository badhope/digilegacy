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
import { useResource } from "@/platform/useResource";
import type { AssetKind } from "@/platform/types";
import { loadAsset } from "./api";
import { platformClient } from "@/platform/client";

const route = useRoute();
const router = useRouter();
const { t, locale } = useI18n();
const assetId = computed(() => String(route.params.id ?? ""));
const { data, loading, error, reload } = useResource(() => loadAsset(assetId.value));
const kindKey = (kind: AssetKind) => `asset.kind.${kind}`;
const editing = ref(false);
const saving = ref(false);
const saveError = ref("");
const editName = ref("");
const editKind = ref<AssetKind>("account");
const editPlatform = ref("");
const editValue = ref("");
const editNote = ref("");
const assetKinds: AssetKind[] = ["account", "file", "device", "domain", "subscription", "physical", "other"];

function startEdit() {
  if (!data.value) return;
  editName.value = data.value.name;
  editKind.value = data.value.kind;
  editPlatform.value = data.value.platform;
  editValue.value = data.value.value;
  editNote.value = data.value.note;
  saveError.value = "";
  editing.value = true;
}

async function saveEdit() {
  if (!data.value) return;
  saveError.value = "";
  saving.value = true;
  try {
    const envelope = await platformClient.updateAsset({
      id: data.value.id,
      name: editName.value.trim(),
      kind: editKind.value,
      platform: editPlatform.value.trim(),
      value: editValue.value.trim(),
      note: editNote.value.trim(),
    });
    if (!envelope.success || !envelope.data) {
      saveError.value = envelope.code === "AUTH_REQUIRED" ? t("assets.create.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    data.value = envelope.data;
    editing.value = false;
  } finally {
    saving.value = false;
  }
}

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
          <AppButton variant="primary" size="sm" @click="startEdit">{{ t("assets.edit.open") }}</AppButton>
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
          <StatusPill tone="ready" :label="t('app.mode')" />
        </template>
      </PageHeader>

      <div v-if="editing" class="edit-card">
        <div class="edit-head">
          <h2>{{ t("assets.edit.title") }}</h2>
          <button type="button" class="close-button" :aria-label="t('common.close')" @click="editing = false">×</button>
        </div>
        <div class="edit-grid">
          <label>
            <span>{{ t("assets.table.name") }}</span>
            <input v-model="editName" type="text" maxlength="120" />
          </label>
          <label>
            <span>{{ t("assets.table.kind") }}</span>
            <select v-model="editKind">
              <option v-for="kind in assetKinds" :key="kind" :value="kind">{{ t(kindKey(kind)) }}</option>
            </select>
          </label>
          <label>
            <span>{{ t("assets.table.platform") }}</span>
            <input v-model="editPlatform" type="text" />
          </label>
          <label>
            <span>{{ t("assets.create.value") }}</span>
            <input v-model="editValue" type="text" inputmode="decimal" />
          </label>
        </div>
        <label class="edit-note">
          <span>{{ t("assets.create.note") }}</span>
          <textarea v-model="editNote" rows="2" />
        </label>
        <div class="edit-actions">
          <AppButton variant="primary" size="sm" :disabled="saving" @click="saveEdit">
            {{ saving ? t("assets.create.saving") : t("assets.edit.save") }}
          </AppButton>
          <span v-if="saveError" class="edit-error">{{ saveError }}</span>
        </div>
      </div>

      <div class="detail-grid">
        <SectionCard :title="t('asset.detail.basic')" :subtitle="t('asset.detail.basic.note')">
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
          <p class="note-body">{{ data.note || "—" }}</p>
          <div class="readonly-note">
            <AppIcon name="info" :size="15" />
            <span>{{ t("asset.detail.basic.note") }}</span>
          </div>
        </SectionCard>
      </div>

    </template>
  </div>
</template>

<style scoped>
.edit-card { margin-bottom: 16px; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.edit-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; }
.edit-head h2 { margin: 0; font-size: 14px; }
.close-button { border: 0; background: transparent; color: var(--muted); font-size: 20px; line-height: 1; cursor: pointer; }
.edit-grid { display: grid; grid-template-columns: 1.4fr 1fr 1fr 1fr; gap: 10px; }
.edit-grid label, .edit-note { display: grid; gap: 5px; }
.edit-grid span, .edit-note span { color: var(--muted); font-size: 10px; letter-spacing: .04em; text-transform: uppercase; }
.edit-grid input, .edit-grid select, .edit-note textarea { width: 100%; padding: 8px 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); color: var(--ink); font: inherit; font-size: 12px; }
.edit-note { margin-top: 10px; }
.edit-actions { display: flex; align-items: center; gap: 10px; margin-top: 10px; }
.edit-error { color: var(--danger, #b42318); font-size: 11px; }
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
