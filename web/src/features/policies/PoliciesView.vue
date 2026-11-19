<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import EmptyState from "@/shared/ui/EmptyState.vue";
import ErrorState from "@/shared/ui/ErrorState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import AssetTabs from "@/features/assets/AssetTabs.vue";
import SkeletonBlock from "@/shared/ui/SkeletonBlock.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import { platformClient } from "@/platform/client";
import type { Policy, ProvenanceStatus } from "@/platform/types";

const { t } = useI18n();
const route = useRoute();
const query = ref(String(route.query.q ?? ""));
const category = ref("");
const { data, loading, error, reload } = useResource(() =>
  platformClient.listPolicies({
    limit: 100,
    ...(query.value.trim() ? { q: query.value.trim() } : {}),
    ...(category.value ? { category: category.value } : {}),
  }),
);
const selected = ref<Policy | null>(null);
const loadingMore = ref(false);
const provenanceStatus = ref("unverified");
const provenanceSource = ref("");
const provenanceNote = ref("");
const provenanceSaving = ref(false);
const provenanceSaved = ref("");
const detailLoading = ref(false);
const detailError = ref<string | null>(null);
const items = computed(() => data.value?.items ?? []);
const total = computed(() => data.value?.total ?? items.value.length);
const categories = computed(() => data.value?.categories ?? []);
const nextCursor = computed(() => data.value?.nextCursor ?? null);

async function loadMore() {
  if (!nextCursor.value || loadingMore.value) return;
  loadingMore.value = true;
  const envelope = await platformClient.listPolicies({
    limit: 100,
    cursor: nextCursor.value,
    ...(query.value.trim() ? { q: query.value.trim() } : {}),
    ...(category.value ? { category: category.value } : {}),
  });
  loadingMore.value = false;
  if (!envelope.success || !envelope.data) return;
  data.value = {
    ...envelope.data,
    items: [...items.value, ...envelope.data.items],
  };
}

let timer: ReturnType<typeof setTimeout> | undefined;
watch([query, category], () => {
  selected.value = null;
  detailError.value = null;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => reload(), 250);
});

async function select(policy: Policy) {
  selected.value = policy;
  provenanceStatus.value = policy.provenance.status;
  provenanceSource.value = policy.provenance.sourceUrl;
  provenanceNote.value = policy.provenance.note;
  provenanceSaved.value = "";
  detailLoading.value = true;
  detailError.value = null;
  const envelope = await platformClient.getPolicy(policy.id);
  detailLoading.value = false;
  if (!envelope.success || !envelope.data) {
    detailError.value = envelope.message || envelope.code;
    return;
  }
  selected.value = envelope.data;
  provenanceStatus.value = envelope.data.provenance.status;
  provenanceSource.value = envelope.data.provenance.sourceUrl;
  provenanceNote.value = envelope.data.provenance.note;
  provenanceSaved.value = "";
}

async function saveProvenance() {
  if (!selected.value || provenanceSaving.value) return;
  provenanceSaving.value = true;
  provenanceSaved.value = "";
  try {
    const envelope = await platformClient.setPolicyProvenance({
      id: selected.value.id,
      status: provenanceStatus.value,
      sourceUrl: provenanceSource.value.trim(),
      note: provenanceNote.value.trim(),
    });
    if (!envelope.success || !envelope.data) {
      detailError.value = envelope.message || envelope.code;
      return;
    }
    selected.value = envelope.data;
    provenanceSaved.value = t("policies.provenance.saved");
  } finally {
    provenanceSaving.value = false;
  }
}

function statusTone(status: ProvenanceStatus): "ready" | "planned" | "degraded" {
  if (status === "verified" || status === "source_cited") return "ready";
  if (status === "disputed") return "degraded";
  return "planned";
}

function safeUrl(value: string): string | null {
  return /^https?:\/\//.test(value.trim()) ? value.trim() : null;
}

function splitList(value: string): string[] {
  return value
    .split(/[,，;；\n]/)
    .map((item) => item.trim())
    .filter(Boolean);
}
</script>

<template>
  <div class="policies-page">
    <PageHeader eyebrow="POLICY / LIBRARY" :title="t('policies.title')" :description="t('policies.description')">
      <template #actions>
        <StatusPill tone="ready" :label="t('app.mode')" />
      </template>
    </PageHeader>

    <AssetTabs />

    <div class="policy-toolbar">
      <label class="search-field">
        <AppIcon name="search" :size="16" />
        <input v-model="query" type="search" :placeholder="t('policies.search')" />
      </label>
      <label class="select-field">
        <span>{{ t("policies.category") }}</span>
        <select v-model="category">
          <option value="">{{ t("policies.category.all") }}</option>
          <option v-for="item in categories" :key="item" :value="item">{{ item }}</option>
        </select>
      </label>
      <span class="policy-count mono">{{ t("policies.count", { count: total }) }}</span>
    </div>

    <div v-if="loading" class="policy-loading" aria-busy="true">
      <div v-for="index in 5" :key="index" class="policy-skeleton">
        <SkeletonBlock width="30%" height="16px" />
        <SkeletonBlock width="55%" height="12px" />
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
      <div v-if="!items.length" class="policy-empty">
        <EmptyState :title="t('policies.empty')" :note="t('policies.emptyHint')" />
      </div>

      <div v-else class="policy-layout">
        <aside class="policy-list" aria-label="Policy list">
          <button
            v-for="policy in items"
            :key="policy.id"
            type="button"
            class="policy-row"
            :class="{ active: selected?.id === policy.id }"
            @click="select(policy)"
          >
            <span class="policy-row-main">
              <strong>{{ policy.name }}</strong>
              <span>{{ policy.category }} <span v-if="policy.subtype">· {{ policy.subtype }}</span></span>
            </span>
            <span class="policy-row-side">
              <StatusPill
                :tone="statusTone(policy.provenance.status)"
                :label="t(`policies.provenance.${policy.provenance.status}`)"
              />
              <AppIcon name="chevron-right" :size="14" />
            </span>
          </button>
          <button
            v-if="nextCursor"
            type="button"
            class="policy-more"
            :disabled="loadingMore"
            @click="loadMore"
          >
            {{ loadingMore ? t("common.loading") : t("common.loadMore") }}
          </button>
        </aside>

        <section class="policy-detail">
          <div v-if="detailLoading" class="detail-loading">
            <SkeletonBlock width="55%" height="24px" />
            <SkeletonBlock width="100%" height="120px" />
          </div>
          <ErrorState
            v-else-if="detailError"
            :title="t('error.generic')"
            :message="detailError"
            :retry-label="t('common.retry')"
            @retry="selected && select(selected)"
          />
          <template v-else-if="selected">
            <div class="detail-head">
              <div>
                <p class="eyebrow mono">{{ selected.category }} / {{ selected.id }}</p>
                <h2>{{ selected.name }}</h2>
                <p v-if="selected.nameEn" class="muted mono">{{ selected.nameEn }}</p>
              </div>
              <StatusPill
                :tone="statusTone(selected.provenance.status)"
                :label="t(`policies.provenance.${selected.provenance.status}`)"
              />
            </div>

            <div class="detail-meta">
              <div><span>{{ t("policies.difficulty") }}</span><strong>{{ selected.estimatedDifficulty || "—" }}</strong></div>
              <div><span>{{ t("policies.time") }}</span><strong>{{ selected.estimatedTimeDays || "—" }}</strong></div>
              <div><span>{{ t("policies.category") }}</span><strong>{{ selected.category }}</strong></div>
              <div><span>{{ t("policies.form") }}</span><strong>{{ selected.inheritable ? "✓" : "—" }}</strong></div>
            </div>

            <section class="detail-block">
              <h3>{{ t("policies.documents") }}</h3>
              <ul v-if="splitList(selected.requiredDocuments).length" class="detail-list">
                <li v-for="item in splitList(selected.requiredDocuments)" :key="item">{{ item }}</li>
              </ul>
              <p v-else class="muted">—</p>
            </section>

            <section class="detail-block">
              <h3>{{ t("policies.process") }}</h3>
              <p class="pre-line">{{ selected.processDescription || "—" }}</p>
            </section>

            <section class="detail-block" v-if="selected.riskWarnings">
              <h3>{{ t("policies.risk") }}</h3>
              <p class="pre-line risk">{{ selected.riskWarnings }}</p>
            </section>

            <section class="detail-block" v-if="selected.monetizationPath">
              <h3>{{ t("policies.monetization") }}</h3>
              <p class="pre-line">{{ selected.monetizationPath }}</p>
            </section>

            <section class="detail-block source-block">
              <h3>{{ t("policies.source") }}</h3>
              <p class="mono">{{ selected.provenance.status }} · {{ selected.dataSource || selected.provenance.note }}</p>
              <div class="source-actions">
                <a
                  v-if="safeUrl(selected.officialPolicyUrl)"
                  class="source-link"
                  :href="safeUrl(selected.officialPolicyUrl)!"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {{ t("policies.sourceOpen") }} <AppIcon name="external" :size="13" />
                </a>
                <a
                  v-if="safeUrl(selected.inheritanceFormUrl)"
                  class="source-link"
                  :href="safeUrl(selected.inheritanceFormUrl)!"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {{ t("policies.form") }} <AppIcon name="external" :size="13" />
                </a>
              </div>
            </section>

            <section class="provenance-editor">
              <h3>{{ t("policies.provenance.edit") }}</h3>
              <p class="muted">{{ t("policies.provenance.hint") }}</p>
              <div class="provenance-grid">
                <label>
                  <span>{{ t("policies.provenance.status") }}</span>
                  <select v-model="provenanceStatus">
                    <option value="unverified">{{ t("policies.provenance.unverified") }}</option>
                    <option value="source_cited">{{ t("policies.provenance.source_cited") }}</option>
                    <option value="verified">{{ t("policies.provenance.verified") }}</option>
                    <option value="disputed">{{ t("policies.provenance.disputed") }}</option>
                  </select>
                </label>
                <label>
                  <span>{{ t("policies.provenance.sourceUrl") }}</span>
                  <input v-model="provenanceSource" type="url" placeholder="https://" />
                </label>
              </div>
              <label class="provenance-note">
                <span>{{ t("policies.provenance.note") }}</span>
                <textarea v-model="provenanceNote" rows="2" />
              </label>
              <div class="provenance-actions">
                <AppButton variant="primary" size="sm" :disabled="provenanceSaving" @click="saveProvenance">
                  {{ provenanceSaving ? t("policies.provenance.saving") : t("policies.provenance.save") }}
                </AppButton>
                <span v-if="provenanceSaved" class="provenance-saved">{{ provenanceSaved }}</span>
              </div>
            </section>

            <div v-if="selected.tags" class="tag-row">
              <span v-for="tag in splitList(selected.tags)" :key="tag" class="tag mono">#{{ tag }}</span>
            </div>
          </template>
          <div v-else class="detail-placeholder">
            <AppIcon name="database" :size="26" />
            <p>{{ t("policies.noSelection") }}</p>
          </div>
        </section>
      </div>
    </template>
  </div>
</template>

<style scoped>
.policies-page { max-width: var(--content-max); margin: 0 auto; }
.policy-toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin-bottom: 14px; }
.search-field { display: flex; align-items: center; gap: 8px; min-width: 260px; padding: 8px 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); }
.search-field input { flex: 1; min-width: 0; border: 0; background: transparent; color: var(--ink); font: inherit; outline: none; }
.select-field { display: flex; align-items: center; gap: 8px; padding: 7px 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); color: var(--muted); font-size: 11px; }
.select-field select { border: 0; background: transparent; color: var(--ink); font: inherit; outline: none; }
.policy-count { margin-left: auto; color: var(--muted); font-size: 10px; }
.policy-layout { display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr); gap: 14px; align-items: start; }
.policy-list { overflow: hidden; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.policy-row { display: flex; width: 100%; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 14px; border: 0; border-top: 1px solid var(--line); background: transparent; color: var(--ink); text-align: left; cursor: pointer; }
.policy-row:first-child { border-top: 0; }
.policy-row:hover, .policy-row.active { background: var(--accent-soft); }
.policy-more { display: block; width: 100%; padding: 12px; border: 0; border-top: 1px solid var(--line); background: var(--surface-soft); color: var(--accent-strong); font-size: 11px; cursor: pointer; }
.policy-more:disabled { color: var(--muted); cursor: wait; }
.policy-row-main { display: flex; min-width: 0; flex-direction: column; gap: 4px; }
.policy-row-main strong { overflow: hidden; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.policy-row-main span { color: var(--muted); font-size: 10px; }
.policy-row-side { display: flex; flex: 0 0 auto; align-items: center; gap: 6px; }
.policy-detail { min-height: 360px; padding: 18px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.detail-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.detail-head h2 { margin: 3px 0 2px; font-size: 24px; }
.detail-head .eyebrow { margin: 0; color: var(--muted); font-size: 9px; letter-spacing: .08em; text-transform: uppercase; }
.detail-meta { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; margin: 16px 0; }
.detail-meta div { padding: 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); }
.detail-meta span { display: block; margin-bottom: 5px; color: var(--muted); font-size: 9px; letter-spacing: .06em; text-transform: uppercase; }
.detail-meta strong { font-size: 12px; font-weight: 600; }
.detail-block { padding: 12px 0; border-top: 1px solid var(--line); }
.detail-block h3 { margin: 0 0 8px; font-size: 12px; letter-spacing: .04em; text-transform: uppercase; }
.detail-block p, .detail-list { margin: 0; color: var(--ink-soft); font-size: 12px; line-height: 1.7; }
.detail-list { padding-left: 18px; }
.detail-list li { margin: 4px 0; }
.pre-line { white-space: pre-line; }
.risk { color: var(--warning, #9a5b21); }
.source-block .source-actions { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 8px; }
.source-link { display: inline-flex; align-items: center; gap: 4px; color: var(--accent-strong); font-size: 11px; text-decoration: none; }
.source-link:hover { text-decoration: underline; }
.provenance-editor { margin-top: 14px; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); }
.provenance-editor h3 { margin: 0 0 4px; font-size: 12px; letter-spacing: .04em; text-transform: uppercase; }
.provenance-editor .muted { margin: 0 0 10px; font-size: 10px; }
.provenance-grid { display: grid; grid-template-columns: 1fr 1.5fr; gap: 8px; }
.provenance-grid label, .provenance-note { display: grid; gap: 4px; }
.provenance-grid span, .provenance-note span { color: var(--muted); font-size: 9px; letter-spacing: .04em; text-transform: uppercase; }
.provenance-grid select, .provenance-grid input, .provenance-note textarea { width: 100%; padding: 7px 8px; border: 1px solid var(--line); border-radius: 4px; background: var(--surface); color: var(--ink); font: inherit; font-size: 11px; outline: none; }
.provenance-note { margin-top: 8px; }
.provenance-actions { display: flex; align-items: center; gap: 10px; margin-top: 8px; }
.provenance-saved { color: var(--accent-strong); font-size: 10px; }
.tag-row { display: flex; flex-wrap: wrap; gap: 6px; padding-top: 10px; border-top: 1px solid var(--line); }
.tag { padding: 3px 7px; border: 1px solid var(--line); border-radius: 99px; color: var(--muted); font-size: 9px; }
.detail-placeholder { display: grid; min-height: 320px; place-items: center; align-content: center; gap: 10px; color: var(--muted); text-align: center; font-size: 12px; }
.policy-skeleton { display: grid; gap: 8px; padding: 14px; border-bottom: 1px solid var(--line); }
.policy-empty { padding: 24px 0; }
@media (max-width: 980px) { .policy-layout { grid-template-columns: 1fr; } .detail-meta { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
@media (max-width: 560px) { .policy-toolbar { align-items: stretch; } .search-field { min-width: 100%; } .policy-count { margin-left: 0; } }
</style>
