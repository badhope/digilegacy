<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import ErrorState from "@/shared/ui/ErrorState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SkeletonBlock from "@/shared/ui/SkeletonBlock.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import { platformClient } from "@/platform/client";
import type { BulkCreateResult, DiscoveryCategory, DiscoverySuggestion } from "@/platform/types";

const router = useRouter();
const { t, locale, isEnglish } = useI18n();
const activeCategory = ref("");
const query = ref("");
const selected = ref<DiscoverySuggestion[]>([]);
const submitting = ref(false);
const submitError = ref("");
const result = ref<BulkCreateResult | null>(null);

const loadDiscovery = () =>
  platformClient.discoverAssets({
    ...(activeCategory.value ? { category: activeCategory.value } : {}),
    ...(query.value.trim() ? { q: query.value.trim() } : {}),
  });
const { data, loading, error, reload } = useResource(loadDiscovery);
let timer: ReturnType<typeof setTimeout> | undefined;
watch(activeCategory, () => reload());
watch(query, () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => reload(), 250);
});
const categories = computed<DiscoveryCategory[]>(() => data.value?.categories ?? []);
const suggestions = computed<DiscoverySuggestion[]>(() => data.value?.suggestions ?? []);
const catalogTotal = computed(() => data.value?.total ?? 36);
const selectedCount = computed(() => selected.value.length);

function categoryLabel(category: DiscoveryCategory): string {
  return isEnglish.value ? category.labelEn || category.label : category.label;
}
function suggestionReason(suggestion: DiscoverySuggestion): string {
  return isEnglish.value ? suggestion.reasonEn || suggestion.reason : suggestion.reason;
}
function categoryOf(id: string): string {
  const category = categories.value.find((item) => item.id === id);
  return category ? categoryLabel(category) : id;
}
function isSelected(id: string): boolean {
  return selected.value.some((item) => item.id === id);
}
function chooseCategory(id: string) {
  activeCategory.value = id;
  query.value = "";
  submitError.value = "";
}
function toggleSuggestion(suggestion: DiscoverySuggestion) {
  if (suggestion.exists) return;
  if (isSelected(suggestion.id)) {
    selected.value = selected.value.filter((item) => item.id !== suggestion.id);
  } else {
    selected.value = [...selected.value, suggestion];
  }
}
function toggleCurrent() {
  const available = suggestions.value.filter((item) => !item.exists);
  const allSelected = available.length > 0 && available.every((item) => isSelected(item.id));
  if (allSelected) {
    const ids = new Set(available.map((item) => item.id));
    selected.value = selected.value.filter((item) => !ids.has(item.id));
  } else {
    const current = new Set(selected.value.map((item) => item.id));
    selected.value = [
      ...selected.value,
      ...available.filter((item) => !current.has(item.id)),
    ];
  }
}
function removeSelected(id: string) {
  selected.value = selected.value.filter((item) => item.id !== id);
}
async function submit() {
  submitError.value = "";
  if (!selected.value.length) {
    submitError.value = t("assets.discover.selectRequired");
    return;
  }
  submitting.value = true;
  try {
    const envelope = await platformClient.bulkCreateAssets(
      selected.value.map((item) => ({
        name: item.name,
        kind: item.kind,
        platform: item.platform,
        note: suggestionReason(item),
      })),
    );
    if (!envelope.success || !envelope.data) {
      submitError.value =
        envelope.code === "AUTH_REQUIRED"
          ? t("assets.create.tokenRequired")
          : envelope.message || envelope.code;
      return;
    }
    result.value = envelope.data;
    selected.value = [];
    await reload();
  } finally {
    submitting.value = false;
  }
}
function discoverMore() {
  result.value = null;
  activeCategory.value = "";
  query.value = "";
  selected.value = [];
  submitError.value = "";
  void reload();
}
function backToAssets() {
  router.push("/assets");
}
</script>

<template>
  <div class="discover-page">
    <PageHeader eyebrow="ASSETS / DISCOVERY" :title="t('assets.discover.title')" :description="t('assets.discover.description')">
      <template #actions>
        <AppButton variant="secondary" size="sm" @click="backToAssets">{{ t("assets.discover.back") }}</AppButton>
      </template>
    </PageHeader>

    <ErrorState v-if="error" :title="t('error.generic')" :message="error" :retry-label="t('common.retry')" @retry="reload" />

    <div v-else-if="result" class="panel result-panel">
      <span class="result-icon"><AppIcon name="check" :size="22" /></span>
      <h2>{{ t("assets.discover.created") }}</h2>
      <p>{{ t("assets.discover.createdHint", { created: result?.created ?? 0, skipped: result?.skipped ?? 0 }) }}</p>
      <div class="result-stats">
        <div><span class="mono">{{ result?.created ?? 0 }}</span><small>{{ t("assets.discover.createdLabel") }}</small></div>
        <div><span class="mono">{{ result?.skipped ?? 0 }}</span><small>{{ t("assets.discover.skippedLabel") }}</small></div>
      </div>
      <div class="panel-actions">
        <AppButton variant="secondary" size="sm" @click="discoverMore">{{ t("assets.discover.addMore") }}</AppButton>
        <AppButton variant="primary" size="sm" @click="backToAssets">{{ t("assets.discover.viewAssets") }}</AppButton>
      </div>
    </div>

    <template v-else>
      <div class="category-strip" role="group" :aria-label="t('assets.discover.step.category')">
        <button
          v-for="category in categories"
          :key="category.id"
          type="button"
          class="category-chip"
          :class="{ active: activeCategory === category.id }"
          @click="chooseCategory(category.id)"
        >
          <span>{{ categoryLabel(category) }}</span>
          <small class="mono">{{ category.count }}</small>
        </button>
        <button
          type="button"
          class="category-chip"
          :class="{ active: activeCategory === '' }"
          @click="chooseCategory('')"
        >
          <span>{{ t("assets.discover.allCategories") }}</span>
          <small class="mono">{{ catalogTotal }}</small>
        </button>
      </div>

      <div class="panel">
        <div class="panel-head">
          <div>
            <h2>{{ activeCategory ? categoryOf(activeCategory) : t("assets.discover.allCategories") }}</h2>
            <p>{{ t("assets.discover.selectHint") }}</p>
          </div>
          <StatusPill tone="ready" :label="t('assets.discover.selected', { count: selectedCount })" />
        </div>

        <div class="discover-toolbar">
          <label class="search-field">
            <AppIcon name="search" :size="16" />
            <input v-model="query" type="search" :placeholder="t('assets.discover.search')" />
          </label>
          <AppButton variant="secondary" size="sm" :disabled="loading || !suggestions.length" @click="toggleCurrent">
            {{ t("assets.discover.toggleAll") }}
          </AppButton>
        </div>

        <div v-if="loading" class="suggestion-grid" aria-busy="true">
          <div v-for="index in 6" :key="index" class="suggestion-skeleton"><SkeletonBlock width="65%" height="16px" /><SkeletonBlock width="90%" height="12px" /></div>
        </div>
        <div v-else-if="suggestions.length" class="suggestion-grid">
          <button
            v-for="suggestion in suggestions"
            :key="suggestion.id"
            type="button"
            class="suggestion-card"
            :class="{ picked: isSelected(suggestion.id), exists: suggestion.exists }"
            :disabled="suggestion.exists"
            @click="toggleSuggestion(suggestion)"
          >
            <span class="check-mark"><AppIcon v-if="isSelected(suggestion.id)" name="check" :size="14" /><AppIcon v-else-if="suggestion.exists" name="lock" :size="13" /></span>
            <strong>{{ suggestion.name }}</strong>
            <small class="suggestion-platform">{{ suggestion.platform }} · {{ t(`asset.kind.${suggestion.kind}`) }}</small>
            <span class="suggestion-reason">{{ suggestionReason(suggestion) }}</span>
            <span v-if="suggestion.exists" class="exists-label">{{ t("assets.discover.exists") }}</span>
          </button>
        </div>
        <div v-else class="empty-panel">
          <p>{{ t("assets.discover.empty") }}</p>
          <AppButton variant="secondary" size="sm" @click="query = ''; reload()">{{ t("common.all") }}</AppButton>
        </div>

        <div v-if="selectedCount" class="selected-tray">
          <div class="selected-tray-head">
            <strong>{{ t("assets.discover.selected", { count: selectedCount }) }}</strong>
            <button type="button" class="link-button" @click="selected = []">{{ t("common.clear") }}</button>
          </div>
          <div class="selected-chips">
            <button v-for="item in selected" :key="item.id" type="button" class="selected-chip" @click="removeSelected(item.id)">
              <span>{{ item.name }}</span>
              <span aria-hidden="true">×</span>
            </button>
          </div>
        </div>

        <div class="panel-actions">
          <span v-if="submitError" class="action-error">{{ submitError }}</span>
          <AppButton variant="primary" size="sm" :disabled="submitting || !selectedCount" @click="submit">
            {{ submitting ? t("assets.discover.creating") : t("assets.discover.create", { count: selectedCount }) }}
          </AppButton>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.discover-page { max-width: var(--content-max); margin: 0 auto; }
.category-strip { display: flex; flex-wrap: wrap; gap: 7px; margin-bottom: 14px; }
.category-chip { display: inline-flex; align-items: center; gap: 7px; padding: 7px 11px; border: 1px solid var(--line); border-radius: 999px; background: var(--surface); color: var(--muted); font-size: 11px; }
.category-chip:hover { border-color: var(--accent); color: var(--ink); }
.category-chip.active { border-color: var(--accent); background: var(--accent-soft); color: var(--accent-strong); font-weight: 600; }
.category-chip small { color: var(--muted); font-size: 9px; }
.panel { border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); box-shadow: var(--shadow-sm); padding: 20px 22px; }
.panel-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 16px; }
.panel-head h2 { margin: 0 0 5px; font-family: var(--font-display); font-size: 20px; }
.panel-head p { margin: 0; color: var(--muted); font-size: 12px; }
.discover-toolbar { display: flex; align-items: center; gap: 10px; margin-bottom: 14px; }
.search-field { display: flex; align-items: center; gap: 8px; flex: 1; max-width: 340px; padding: 0 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-muted); color: var(--muted); }
.search-field input { width: 100%; height: 34px; border: 0; background: transparent; color: var(--fg); outline: none; font-size: 12px; }
.suggestion-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(245px, 1fr)); gap: 10px; }
.suggestion-card { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; padding: 13px 14px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-muted); text-align: left; cursor: pointer; position: relative; }
.suggestion-card:hover:not(:disabled) { border-color: var(--accent); background: var(--surface); }
.suggestion-card.picked { border-color: var(--accent); background: var(--accent-soft); }
.suggestion-card.exists { opacity: .58; cursor: default; }
.suggestion-card strong { font-size: 13px; }
.suggestion-platform { color: var(--muted); font-size: 10px; }
.suggestion-reason { color: var(--muted); font-size: 11px; line-height: 1.55; }
.check-mark { position: absolute; top: 11px; right: 11px; display: inline-grid; place-items: center; width: 18px; height: 18px; border: 1px solid var(--line); border-radius: 50%; color: var(--accent); background: var(--surface); }
.suggestion-card.picked .check-mark { border-color: var(--accent); background: var(--accent); color: var(--accent-fg); }
.exists-label { color: var(--muted); font-size: 10px; }
.suggestion-skeleton { display: flex; flex-direction: column; gap: 8px; padding: 14px; border: 1px solid var(--line); border-radius: var(--radius-sm); }
.empty-panel { display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 32px 10px; color: var(--muted); font-size: 12px; }
.selected-tray { margin-top: 16px; padding-top: 14px; border-top: 1px solid var(--line); }
.selected-tray-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.selected-tray-head strong { font-size: 12px; color: var(--ink); }
.selected-chips { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; }
.selected-chip { display: inline-flex; align-items: center; gap: 6px; padding: 5px 9px; border: 1px solid var(--line); border-radius: 999px; background: var(--surface-soft); color: var(--ink-soft); font-size: 10px; }
.selected-chip:hover { border-color: var(--accent); color: var(--accent-strong); }
.panel-actions { display: flex; align-items: center; justify-content: flex-end; gap: 10px; margin-top: 16px; padding-top: 14px; border-top: 1px solid var(--line); flex-wrap: wrap; }
.action-error { margin-right: auto; color: var(--danger); font-size: 11px; }
.result-panel { text-align: center; padding: 36px 22px; }
.result-icon { display: inline-grid; place-items: center; width: 48px; height: 48px; margin-bottom: 6px; border-radius: 50%; background: var(--ok-soft); color: var(--ok); }
.result-panel h2 { margin: 0 0 5px; font-family: var(--font-display); font-size: 22px; }
.result-panel p { margin: 0; color: var(--muted); font-size: 12px; }
.result-stats { display: flex; justify-content: center; gap: 34px; margin: 20px 0 4px; }
.result-stats span { display: block; font-size: 24px; color: var(--accent); }
.result-stats small { color: var(--muted); font-size: 10px; }
.result-panel .panel-actions { justify-content: center; }
@media (max-width: 640px) {
  .panel { padding: 16px 14px; }
  .discover-toolbar { flex-direction: column; align-items: stretch; }
  .search-field { max-width: none; }
  .panel-head { flex-direction: column; }
}
</style>
