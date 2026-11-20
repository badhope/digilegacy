<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
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
import { formatDateTime } from "@/shared/format";
import { usePolicyI18n } from "./messages";
import { useResource } from "@/platform/useResource";
import { platformClient } from "@/platform/client";
import type { Policy, PolicyCandidate, ProvenanceStatus } from "@/platform/types";

const { t, locale } = useI18n();
const { tp } = usePolicyI18n();
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
const activeCandidates = ref<PolicyCandidate[]>([]);
const candidatesLoading = ref(false);
const candidatesError = ref<string | null>(null);
const syncing = ref(false);
const syncMessage = ref("");
const syncError = ref<string | null>(null);
const syncAt = ref("");
const syncAtLabel = computed(() => (syncAt.value ? formatDateTime(syncAt.value, locale.value, true) : ""));
const candidateBusy = ref("");
const activeCandidateCount = computed(() => activeCandidates.value.length);

async function reloadCandidates() {
  candidatesLoading.value = true;
  candidatesError.value = null;
  const envelope = await platformClient.listPolicyCandidates({ status: "all", limit: 100 });
  candidatesLoading.value = false;
  if (!envelope.success || !envelope.data) {
    candidatesError.value = envelope.message || envelope.code;
    return;
  }
  activeCandidates.value = envelope.data.items.filter(
    (item) => item.status === "pending" || item.status === "conflict",
  );
  syncAt.value = envelope.data.lastSyncAt;
}

async function syncPolicies() {
  if (syncing.value) return;
  syncing.value = true;
  syncMessage.value = "";
  syncError.value = null;
  try {
    const envelope = await platformClient.syncPolicies();
    if (!envelope.success || !envelope.data) {
      syncError.value = envelope.message || envelope.code;
      return;
    }
    syncMessage.value = envelope.data.message;
    syncAt.value = envelope.data.lastSyncAt;
    await reloadCandidates();
  } finally {
    syncing.value = false;
  }
}

async function decideCandidate(candidate: PolicyCandidate, action: "approve" | "reject") {
  if (candidateBusy.value) return;
  candidateBusy.value = candidate.id;
  candidatesError.value = null;
  try {
    const envelope = await platformClient.decidePolicyCandidate({
      id: candidate.id,
      action,
    });
    if (!envelope.success || !envelope.data) {
      candidatesError.value = envelope.message || envelope.code;
      return;
    }
    if (action === "approve") {
      selected.value = null;
      await reload();
    }
    await reloadCandidates();
  } finally {
    candidateBusy.value = "";
  }
}

onMounted(reloadCandidates);

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
    provenanceSaved.value = tp("policies.provenance.saved");
  } finally {
    provenanceSaving.value = false;
  }
}

function calculateCandidateTone(status: string): "planned" | "degraded" {
  return status === "conflict" ? "degraded" : "planned";
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
    <PageHeader eyebrow="POLICY / LIBRARY" :title="tp('policies.title')" :description="tp('policies.description')" />

    <AssetTabs />

    <div class="policy-toolbar">
      <label class="search-field">
        <AppIcon name="search" :size="16" />
        <input v-model="query" type="search" :placeholder="tp('policies.search')" />
      </label>
      <label class="select-field">
        <span>{{ tp("policies.category") }}</span>
        <select v-model="category">
          <option value="">{{ tp("policies.category.all") }}</option>
          <option v-for="item in categories" :key="item" :value="item">{{ item }}</option>
        </select>
      </label>
      <span class="policy-count mono">{{ tp("policies.count", { count: total }) }}</span>
    </div>

    <section class="policy-sync">
      <div class="sync-copy">
        <strong>{{ tp("policies.sync") }}</strong>
        <span v-if="syncError" class="sync-error">{{ syncError }}</span>
        <span v-else-if="syncMessage" class="sync-message">{{ syncMessage }}</span>
        <span v-else class="muted">
          {{ syncAt ? tp("policies.lastSync", { at: syncAtLabel }) : tp("policies.neverSynced") }}
        </span>
      </div>
      <AppButton variant="primary" size="sm" :disabled="syncing" @click="syncPolicies">
        {{ syncing ? t("common.loading") : tp("policies.syncAction") }}
      </AppButton>
    </section>

    <section
      v-if="candidatesError || activeCandidateCount"
      class="candidate-panel"
      aria-live="polite"
    >
      <header class="candidate-panel-head">
        <strong>{{ tp("policies.candidates", { count: activeCandidateCount }) }}</strong>
      </header>
      <p v-if="candidatesError" class="sync-error">{{ candidatesError }}</p>
      <article v-for="candidate in activeCandidates" :key="candidate.id" class="candidate-card">
        <header class="candidate-head">
          <div>
            <strong>{{ candidate.name }}</strong>
            <p class="mono">{{ candidate.platformId }} · {{ candidate.source }}</p>
          </div>
          <StatusPill
            :tone="calculateCandidateTone(candidate.status)"
            :label="
              candidate.status === 'conflict'
                ? tp('policies.candidateConflict')
                : tp('policies.candidatePending')
            "
          />
        </header>
        <details class="candidate-diff">
          <summary>{{ tp("policies.candidateFields", { count: candidate.changedFields.length }) }}</summary>
          <div v-for="field in candidate.diff" :key="field.field" class="diff-row">
            <span class="diff-field mono">{{ field.field }}</span>
            <span class="diff-value">{{ field.before || "—" }} → {{ field.after || "—" }}</span>
          </div>
        </details>
        <div class="candidate-actions">
          <AppButton
            variant="primary"
            size="sm"
            :disabled="candidateBusy === candidate.id"
            @click="decideCandidate(candidate, 'approve')"
          >
            {{ candidateBusy === candidate.id ? t("common.loading") : tp("policies.approve") }}
          </AppButton>
          <AppButton
            variant="ghost"
            size="sm"
            :disabled="candidateBusy === candidate.id"
            @click="decideCandidate(candidate, 'reject')"
          >
            {{ tp("policies.reject") }}
          </AppButton>
        </div>
      </article>
    </section>

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
        <EmptyState :title="tp('policies.empty')" :note="tp('policies.emptyHint')" />
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
                :label="tp(`policies.provenance.${policy.provenance.status}`)"
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
                :label="tp(`policies.provenance.${selected.provenance.status}`)"
              />
            </div>

            <div class="detail-meta">
              <div><span>{{ tp("policies.difficulty") }}</span><strong>{{ selected.estimatedDifficulty || "—" }}</strong></div>
              <div><span>{{ tp("policies.time") }}</span><strong>{{ selected.estimatedTimeDays || "—" }}</strong></div>
              <div><span>{{ tp("policies.category") }}</span><strong>{{ selected.category }}</strong></div>
              <div><span>{{ tp("policies.form") }}</span><strong>{{ selected.inheritable ? "✓" : "—" }}</strong></div>
            </div>

            <section class="detail-block">
              <h3>{{ tp("policies.documents") }}</h3>
              <ul v-if="splitList(selected.requiredDocuments).length" class="detail-list">
                <li v-for="item in splitList(selected.requiredDocuments)" :key="item">{{ item }}</li>
              </ul>
              <p v-else class="muted">—</p>
            </section>

            <section class="detail-block">
              <h3>{{ tp("policies.process") }}</h3>
              <p class="pre-line">{{ selected.processDescription || "—" }}</p>
            </section>

            <section class="detail-block" v-if="selected.riskWarnings">
              <h3>{{ tp("policies.risk") }}</h3>
              <p class="pre-line risk">{{ selected.riskWarnings }}</p>
            </section>

            <section class="detail-block" v-if="selected.monetizationPath">
              <h3>{{ tp("policies.monetization") }}</h3>
              <p class="pre-line">{{ selected.monetizationPath }}</p>
            </section>

            <section class="detail-block source-block">
              <h3>{{ tp("policies.source") }}</h3>
              <p class="mono">{{ selected.provenance.status }} · {{ selected.dataSource || selected.provenance.note }}</p>
              <div class="source-actions">
                <a
                  v-if="safeUrl(selected.officialPolicyUrl)"
                  class="source-link"
                  :href="safeUrl(selected.officialPolicyUrl)!"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {{ tp("policies.sourceOpen") }} <AppIcon name="external" :size="13" />
                </a>
                <a
                  v-if="safeUrl(selected.inheritanceFormUrl)"
                  class="source-link"
                  :href="safeUrl(selected.inheritanceFormUrl)!"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {{ tp("policies.form") }} <AppIcon name="external" :size="13" />
                </a>
              </div>
            </section>

            <section class="provenance-editor">
              <h3>{{ tp("policies.provenance.edit") }}</h3>
              <p class="muted">{{ tp("policies.provenance.hint") }}</p>
              <div class="provenance-grid">
                <label>
                  <span>{{ tp("policies.provenance.status") }}</span>
                  <select v-model="provenanceStatus">
                    <option value="unverified">{{ tp("policies.provenance.unverified") }}</option>
                    <option value="source_cited">{{ tp("policies.provenance.source_cited") }}</option>
                    <option value="verified">{{ tp("policies.provenance.verified") }}</option>
                    <option value="disputed">{{ tp("policies.provenance.disputed") }}</option>
                  </select>
                </label>
                <label>
                  <span>{{ tp("policies.provenance.sourceUrl") }}</span>
                  <input v-model="provenanceSource" type="url" placeholder="https://" />
                </label>
              </div>
              <label class="provenance-note">
                <span>{{ tp("policies.provenance.note") }}</span>
                <textarea v-model="provenanceNote" rows="2" />
              </label>
              <div class="provenance-actions">
                <AppButton variant="primary" size="sm" :disabled="provenanceSaving" @click="saveProvenance">
                  {{ provenanceSaving ? tp("policies.provenance.saving") : tp("policies.provenance.save") }}
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
            <p>{{ tp("policies.noSelection") }}</p>
          </div>
        </section>
      </div>
    </template>
  </div>
</template>

<style scoped>
.policies-page { max-width: var(--content-max); margin: 0 auto; }
.policy-sync { display: flex; align-items: center; justify-content: space-between; gap: 14px; margin-bottom: 12px; padding: 12px 14px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.sync-copy { display: grid; min-width: 0; gap: 3px; }
.sync-copy strong { font-size: 12px; letter-spacing: .03em; }
.sync-copy .muted, .sync-message, .sync-error { overflow-wrap: anywhere; font-size: 10px; }
.sync-message { color: var(--accent-strong); }
.sync-error { color: var(--warning, #9a5b21); }
.candidate-panel { display: grid; gap: 8px; margin-bottom: 14px; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface-soft); }
.candidate-panel-head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px; }
.candidate-panel-head strong { font-size: 12px; }
.candidate-card { padding: 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); }
.candidate-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 10px; }
.candidate-head strong { font-size: 13px; }
.candidate-head p { overflow: hidden; margin: 3px 0 0; color: var(--muted); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
.candidate-diff { margin-top: 8px; border-top: 1px solid var(--line); padding-top: 7px; }
.candidate-diff summary { color: var(--accent-strong); font-size: 10px; cursor: pointer; }
.diff-row { display: grid; grid-template-columns: minmax(96px, 140px) minmax(0, 1fr); gap: 10px; padding: 7px 0; border-top: 1px dashed var(--line); }
.diff-row:first-child { margin-top: 6px; }
.diff-field { align-self: start; color: var(--muted); font-size: 9px; overflow-wrap: anywhere; }
.diff-value { min-width: 0; color: var(--ink); overflow-wrap: anywhere; font-size: 10px; line-height: 1.5; }
.candidate-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 9px; }
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
@media (max-width: 560px) { .policy-toolbar { align-items: stretch; } .search-field { min-width: 100%; } .policy-count { margin-left: 0; } .policy-sync { align-items: flex-start; flex-direction: column; } .candidate-head { flex-direction: column; } .diff-row { grid-template-columns: 1fr; gap: 4px; } }
</style>
