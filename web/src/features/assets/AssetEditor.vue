<script setup lang="ts">
import { ref } from "vue";
import AppButton from "@/shared/ui/AppButton.vue";
import { useI18n } from "@/platform/i18n";
import type { Asset } from "@/platform/types";
import { ASSET_KIND_GROUPS, assetGroupLabel, assetKindLabel, draftFromAsset, type AssetDraft } from "./model";

const props = withDefaults(
  defineProps<{
    initial?: Partial<Asset> | null;
    mode?: "create" | "edit";
    saving?: boolean;
    error?: string;
  }>(),
  { initial: null, mode: "create", saving: false, error: "" },
);
const emit = defineEmits<{ submit: [draft: AssetDraft]; cancel: [] }>();
const { t, locale } = useI18n();
const draft = ref<AssetDraft>(draftFromAsset(props.initial));
const localError = ref("");

function submit() {
  if (!draft.value.name.trim()) {
    localError.value = t("assets.create.nameRequired");
    return;
  }
  localError.value = "";
  emit("submit", {
    ...draft.value,
    name: draft.value.name.trim(),
    platform: draft.value.platform.trim(),
    value: draft.value.value.trim(),
    note: draft.value.note.trim(),
  });
}
</script>

<template>
  <div class="asset-editor">
    <div class="editor-head">
      <div>
        <h2>{{ mode === "edit" ? t("assets.edit.title") : t("assets.create.title") }}</h2>
        <p v-if="mode === 'create'">{{ t("assets.create.hint") }}</p>
      </div>
      <button type="button" class="close-button" :aria-label="t('common.close')" @click="emit('cancel')">×</button>
    </div>
    <div class="editor-grid">
      <label>
        <span>{{ t("assets.table.name") }}</span>
        <input v-model="draft.name" type="text" maxlength="120" :placeholder="t('assets.create.namePlaceholder')" />
      </label>
      <label>
        <span>{{ t("assets.table.kind") }}</span>
        <select v-model="draft.kind">
          <optgroup v-for="group in ASSET_KIND_GROUPS" :key="group.label" :label="assetGroupLabel(group.label, locale)">
            <option v-for="kind in group.kinds" :key="kind" :value="kind">{{ assetKindLabel(kind, locale) }}</option>
          </optgroup>
        </select>
      </label>
      <label>
        <span>{{ t("assets.table.platform") }}</span>
        <input v-model="draft.platform" type="text" :placeholder="t('assets.create.platformPlaceholder')" />
      </label>
      <label>
        <span>{{ t("assets.create.value") }}</span>
        <input v-model="draft.value" type="text" inputmode="decimal" :placeholder="t('assets.create.valuePlaceholder')" />
      </label>
    </div>
    <label class="editor-note">
      <span>{{ t("assets.create.note") }}</span>
      <textarea v-model="draft.note" rows="2" :placeholder="t('assets.create.notePlaceholder')" />
    </label>
    <div class="editor-actions">
      <AppButton variant="primary" size="sm" :disabled="saving" @click="submit">
        {{ saving ? t("assets.create.saving") : mode === "edit" ? t("assets.edit.save") : t("assets.create.save") }}
      </AppButton>
      <span v-if="localError || error" class="editor-error">{{ localError || error }}</span>
    </div>
  </div>
</template>

<style scoped>
.asset-editor { margin-bottom: 14px; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.editor-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 12px; }
.editor-head h2 { margin: 0 0 4px; font-size: 14px; }
.editor-head p { margin: 0; color: var(--muted); font-size: 11px; }
.close-button { border: 0; background: transparent; color: var(--muted); font-size: 20px; line-height: 1; cursor: pointer; }
.editor-grid { display: grid; grid-template-columns: 1.4fr 1fr 1fr 1fr; gap: 10px; }
.editor-grid label, .editor-note { display: grid; gap: 5px; }
.editor-grid span, .editor-note span { color: var(--muted); font-size: 10px; letter-spacing: .04em; text-transform: uppercase; }
.editor-grid input, .editor-grid select, .editor-note textarea { width: 100%; padding: 8px 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); color: var(--ink); font: inherit; font-size: 12px; }
.editor-note { margin-top: 10px; }
.editor-actions { display: flex; align-items: center; gap: 10px; margin-top: 10px; }
.editor-error { color: var(--danger, #b42318); font-size: 11px; }
@media (max-width: 760px) {
  .editor-grid { grid-template-columns: 1fr 1fr; }
  .asset-editor { padding: 14px; }
}
@media (max-width: 480px) {
  .editor-grid { grid-template-columns: 1fr; }
}
</style>
