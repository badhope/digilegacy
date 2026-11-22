<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { RouterLink } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import { useI18n } from "@/platform/i18n";
import type { Asset, AssetValueStatus } from "@/platform/types";
import {
  ASSET_KIND_GROUPS,
  ASSET_VALUE_STATUSES,
  assetGroupLabel,
  assetKindLabel,
  assetValueStatusLabel,
  draftFromAsset,
  normalizeValuation,
  type AssetDraft,
} from "./model";
import { clearAssetDraft, readAssetDraft, writeAssetDraft } from "./draftStorage";

const props = withDefaults(
  defineProps<{
    initial?: Partial<Asset> | null;
    mode?: "create" | "edit";
    saving?: boolean;
    error?: string;
    tokenMissing?: boolean;
  }>(),
  { initial: null, mode: "create", saving: false, error: "", tokenMissing: false },
);
const emit = defineEmits<{
  submit: [draft: AssetDraft, options: { continue: boolean }];
  cancel: [];
}>();
const { t, locale } = useI18n();
const draft = ref<AssetDraft>(draftFromAsset(props.initial));
const baseline = ref(JSON.stringify(draft.value));
const localError = ref("");
const confirmClose = ref(false);
const restored = ref(false);
const moreOpen = ref(false);

const dirty = computed(() => restored.value || JSON.stringify(draft.value) !== baseline.value);

onMounted(() => {
  if (props.mode !== "create") {
    baseline.value = JSON.stringify(draft.value);
    return;
  }
  const stored = readAssetDraft();
  if (stored) {
    draft.value = stored;
    restored.value = true;
    baseline.value = JSON.stringify(draftFromAsset(null));
    moreOpen.value = true;
  }
});

watch(
  draft,
  (value) => {
    if (props.mode === "create") writeAssetDraft(value);
  },
  { deep: true },
);

watch(
  () => draft.value.valueStatus,
  (status) => {
    if (status === "unknown") draft.value.value = "";
  },
);

function requestClose() {
  if (props.saving) return;
  if (!dirty.value) {
    emit("cancel");
    return;
  }
  confirmClose.value = true;
}

function discardAndClose() {
  clearAssetDraft();
  emit("cancel");
}

function keepEditing() {
  confirmClose.value = false;
}

function clearRestored() {
  clearAssetDraft();
  draft.value = draftFromAsset(null);
  baseline.value = JSON.stringify(draft.value);
  restored.value = false;
  localError.value = "";
}

function resetDraft(previous?: AssetDraft) {
  clearAssetDraft();
  draft.value = draftFromAsset({
    kind: previous?.kind ?? "account",
    valueStatus: previous?.valueStatus ?? "market",
  });
  baseline.value = JSON.stringify(draft.value);
  restored.value = false;
  localError.value = "";
  confirmClose.value = false;
  moreOpen.value = false;
}

defineExpose({ resetDraft });

function submit(keepGoing: boolean) {
  if (props.saving) return;
  if (!draft.value.name.trim()) {
    localError.value = t("assets.create.nameRequired");
    return;
  }
  const valueStatus = draft.value.valueStatus as AssetValueStatus;
  const value = valueStatus === "unknown" ? "" : normalizeValuation(draft.value.value);
  if (value === null) {
    localError.value = t("assets.create.valueInvalid");
    return;
  }
  const expiresAt = draft.value.expiresAt.trim();
  if (expiresAt) {
    const parsed = new Date(`${expiresAt}T00:00:00Z`);
    if (
      !/^\d{4}-\d{2}-\d{2}$/.test(expiresAt) ||
      Number.isNaN(parsed.getTime()) ||
      parsed.toISOString().slice(0, 10) !== expiresAt
    ) {
      localError.value = t("assets.create.expiresInvalid");
      return;
    }
  }
  localError.value = "";
  confirmClose.value = false;
  emit(
    "submit",
    {
      ...draft.value,
      name: draft.value.name.trim(),
      platform: draft.value.platform.trim(),
      value: value ?? "",
      note: draft.value.note.trim(),
      identifier: draft.value.identifier.trim(),
      ownershipShare: draft.value.ownershipShare.trim(),
      beneficiary: draft.value.beneficiary.trim(),
      beneficiaryContact: draft.value.beneficiaryContact.trim(),
      credentialLocation: draft.value.credentialLocation.trim(),
      expiresAt,
      quantity: draft.value.quantity.trim(),
      unit: draft.value.unit.trim(),
    },
    { continue: keepGoing },
  );
}
</script>

<template>
  <div class="asset-editor">
    <div class="editor-head">
      <div>
        <h2>{{ mode === "edit" ? t("assets.edit.title") : t("assets.create.title") }}</h2>
        <p v-if="mode === 'create'">{{ t("assets.create.hint") }}</p>
      </div>
      <button type="button" class="close-button" :aria-label="t('common.close')" @click="requestClose">×</button>
    </div>

    <div v-if="restored" class="draft-note" role="status">
      <span>{{ t("assets.create.draftRestored") }}</span>
      <button type="button" @click="clearRestored">{{ t("assets.create.draftClear") }}</button>
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
        <input v-model="draft.platform" type="text" maxlength="80" :placeholder="t('assets.create.platformPlaceholder')" />
      </label>
      <label>
        <span>{{ t("assets.create.value") }}</span>
        <input
          v-model="draft.value"
          type="text"
          inputmode="decimal"
          :disabled="draft.valueStatus === 'unknown'"
          :placeholder="draft.valueStatus === 'unknown' ? t('assets.create.valueUnknown') : t('assets.create.valuePlaceholder')"
        />
      </label>
      <label>
        <span>{{ t("assets.create.valueStatus") }}</span>
        <select v-model="draft.valueStatus">
          <option v-for="status in ASSET_VALUE_STATUSES" :key="status" :value="status">
            {{ assetValueStatusLabel(status, locale) }}
          </option>
        </select>
      </label>
    </div>

    <button type="button" class="more-toggle" :aria-expanded="moreOpen" @click="moreOpen = !moreOpen">
      {{ moreOpen ? t("assets.create.moreCollapse") : t("assets.create.moreExpand") }}
    </button>

    <div v-if="moreOpen" class="more-fields">
      <div class="editor-grid">
        <label>
          <span>{{ t("assets.create.identifier") }}</span>
          <input v-model="draft.identifier" type="text" maxlength="120" :placeholder="t('assets.create.identifierPlaceholder')" />
        </label>
        <label>
          <span>{{ t("assets.create.ownershipShare") }}</span>
          <input v-model="draft.ownershipShare" type="text" maxlength="120" :placeholder="t('assets.create.ownershipSharePlaceholder')" />
        </label>
        <label>
          <span>{{ t("assets.create.beneficiary") }}</span>
          <input v-model="draft.beneficiary" type="text" maxlength="80" :placeholder="t('assets.create.beneficiaryPlaceholder')" />
        </label>
        <label>
          <span>{{ t("assets.create.beneficiaryContact") }}</span>
          <input v-model="draft.beneficiaryContact" type="text" maxlength="120" :placeholder="t('assets.create.beneficiaryContactPlaceholder')" />
        </label>
        <label>
          <span>{{ t("assets.create.credentialLocation") }}</span>
          <input v-model="draft.credentialLocation" type="text" maxlength="120" :placeholder="t('assets.create.credentialLocationPlaceholder')" />
        </label>
        <label>
          <span>{{ t("assets.create.expiresAt") }}</span>
          <input v-model="draft.expiresAt" type="date" />
        </label>
        <label>
          <span>{{ t("assets.create.quantity") }}</span>
          <input v-model="draft.quantity" type="text" maxlength="40" :placeholder="t('assets.create.quantityPlaceholder')" />
        </label>
        <label>
          <span>{{ t("assets.create.unit") }}</span>
          <input v-model="draft.unit" type="text" maxlength="20" :placeholder="t('assets.create.unitPlaceholder')" />
        </label>
      </div>
    </div>

    <label class="editor-note">
      <span>{{ t("assets.create.note") }}</span>
      <textarea v-model="draft.note" rows="2" :placeholder="t('assets.create.notePlaceholder')" />
    </label>

    <div v-if="confirmClose" class="close-confirm" role="alert">
      <span>{{ t("assets.create.discardConfirm") }}</span>
      <div class="close-confirm-actions">
        <AppButton variant="secondary" size="sm" @click="keepEditing">{{ t("assets.create.keepEditing") }}</AppButton>
        <AppButton variant="danger" size="sm" @click="discardAndClose">{{ t("assets.create.discard") }}</AppButton>
      </div>
    </div>

    <div class="editor-actions">
      <AppButton variant="primary" size="sm" :disabled="saving" @click="submit(false)">
        {{ saving ? t("assets.create.saving") : mode === "edit" ? t("assets.edit.save") : t("assets.create.save") }}
      </AppButton>
      <AppButton v-if="mode === 'create'" variant="secondary" size="sm" :disabled="saving" @click="submit(true)">
        {{ t("assets.create.saveAndContinue") }}
      </AppButton>
      <span v-if="localError || error" class="editor-error">{{ localError || error }}</span>
      <RouterLink v-if="tokenMissing" class="editor-settings" to="/settings">
        {{ t("assets.create.openSettings") }}
      </RouterLink>
    </div>
  </div>
</template>

<style scoped>
.asset-editor { margin-bottom: 14px; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.editor-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 12px; }
.editor-head h2 { margin: 0 0 4px; font-size: 14px; }
.editor-head p { margin: 0; color: var(--muted); font-size: 11px; }
.close-button { border: 0; background: transparent; color: var(--muted); font-size: 20px; line-height: 1; cursor: pointer; }
.draft-note { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-bottom: 10px; padding: 8px 10px; border: 1px dashed var(--line-strong); border-radius: var(--radius-sm); color: var(--ink-soft); font-size: 11px; }
.draft-note button { border: 0; background: transparent; padding: 0; color: var(--accent-strong); font: inherit; cursor: pointer; text-decoration: underline; }
.editor-grid { display: grid; grid-template-columns: 1.4fr 1fr 1fr 1fr 0.8fr; gap: 10px; }
.editor-grid label, .editor-note { display: grid; gap: 5px; }
.editor-grid span, .editor-note span { color: var(--muted); font-size: 10px; letter-spacing: .04em; text-transform: uppercase; }
.editor-grid input, .editor-grid select, .editor-note textarea { width: 100%; min-height: 36px; padding: 8px 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); color: var(--ink); font: inherit; font-size: 12px; }
.editor-grid input:disabled { opacity: .55; }
.more-toggle { margin-top: 10px; border: 0; background: transparent; padding: 2px 0; color: var(--accent-strong); font: inherit; font-size: 11px; cursor: pointer; }
.more-toggle::before { content: "+"; margin-right: 4px; }
.more-toggle[aria-expanded="true"]::before { content: "−"; }
.more-fields { margin-top: 8px; padding-top: 10px; border-top: 1px dashed var(--line); }
.editor-note { margin-top: 10px; }
.close-confirm { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px; margin-top: 10px; padding: 9px 10px; border: 1px solid color-mix(in srgb, var(--danger, #b42318) 35%, var(--line)); border-radius: var(--radius-sm); background: color-mix(in srgb, var(--danger, #b42318) 6%, var(--surface)); color: var(--ink-soft); font-size: 11px; }
.close-confirm-actions { display: flex; gap: 8px; }
.editor-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin-top: 10px; }
.editor-error { color: var(--danger, #b42318); font-size: 11px; }
.editor-settings { color: var(--accent-strong); font-size: 11px; text-decoration: underline; }
@media (max-width: 900px) {
  .editor-grid { grid-template-columns: 1fr 1fr 1fr; }
}
@media (max-width: 760px) {
  .editor-grid { grid-template-columns: 1fr 1fr; }
  .asset-editor { padding: 14px; }
}
@media (max-width: 480px) {
  .editor-grid { grid-template-columns: 1fr; }
}
</style>
