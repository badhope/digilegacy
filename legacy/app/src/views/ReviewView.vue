<template>
  <div class="review fade-in">
    <h2 style="font-size: var(--text-xl); font-weight: 700; margin-bottom: 8px;">{{ t('review.title') }}</h2>
    <p class="text-muted mb-24" style="font-size: var(--text-base);">{{ t('review.subtitle') }}</p>

    <!-- 场景切换：场景是数据（后端注册表），界面不写死任何业务名 -->
    <div class="card mb-24">
      <div class="card-title mb-12">{{ t('review.sceneTitle') }}</div>
      <div class="flex gap-8 flex-wrap" role="tablist" :aria-label="t('review.sceneTitle')">
        <button
          v-for="p in profiles"
          :key="p.id"
          class="btn btn-sm"
          :class="activeId === p.id ? 'btn-primary' : 'btn-secondary'"
          role="tab"
          :aria-selected="activeId === p.id"
          @click="selectProfile(p.id)"
        >{{ t(p.title_key) }}</button>
      </div>
      <p v-if="activeProfile" class="form-hint" style="margin-top: 10px;">
        {{ activeProfile.use_ai ? t('review.aiEnabled') : t('review.aiDisabled') }}
      </p>
      <p v-if="isStrict" class="form-hint" style="margin-top: 4px; color: var(--warning-strong);">
        {{ t('review.strictNotice') }}
      </p>
      <p v-if="progress" class="form-hint" style="margin-top: 6px;" aria-live="polite">
        {{ t('review.progress', { approved: progress.approved, total: progress.total, awaiting: progress.awaiting_human, pending: progress.pending }) }}
      </p>
      <p v-if="aiNotice" class="form-hint" style="margin-top: 6px; color: var(--warning-strong);">{{ aiNotice }}</p>
    </div>

    <div v-if="activeId" class="card mb-24">
      <div class="card-title mb-12">
        {{ activeProfile?.use_ai ? t('review.payloadTitle') : t('review.scopeTitle') }}
      </div>
      <p class="form-hint">
        {{ activeProfile?.use_ai ? t('review.payloadNote') : t('review.scopeNote') }}
      </p>
      <ul class="payload-list">
        <li v-for="f in payloadFields" :key="f">{{ t(f) }}</li>
      </ul>
      <div v-if="isStrict" style="margin-top: 12px;">
        <div style="font-size: var(--text-sm); font-weight: 600;">{{ t('review.strictChecksTitle') }}</div>
        <p class="form-hint">{{ t('review.strictChecksNote') }}</p>
        <ul class="payload-list" style="margin-top: 6px;">
          <li v-for="c in activeProfile?.strict_checks ?? []" :key="c.id">{{ c.label }}</li>
        </ul>
      </div>
      <div style="display: flex; gap: 12px; flex-wrap: wrap; margin-top: 12px;">
        <!-- 纯人工场景不给 AI 按钮：按钮存在本身就会让人以为「点了就审了」 -->
        <button v-if="activeProfile?.use_ai" class="btn btn-secondary btn-sm" :disabled="busy" @click="runAi">
          {{ busy ? t('review.running') : t('review.runAi') }}
        </button>
        <button class="btn btn-secondary btn-sm" :disabled="busy" @click="loadItems">{{ t('review.reload') }}</button>
      </div>
      <p v-if="message" style="font-size: var(--text-xs); margin-top: 8px; color: var(--gray-600);" aria-live="polite">{{ message }}</p>
    </div>

    <div v-if="activeId" class="card">
      <div class="card-header">
        <div class="card-title">{{ t('review.itemsTitle') }}</div>
        <div class="flex gap-8 flex-wrap" role="group" :aria-label="t('review.filterLabel')">
          <button
            v-for="f in filters"
            :key="f.value"
            class="btn btn-sm"
            :class="statusFilter === f.value ? 'btn-primary' : 'btn-secondary'"
            :aria-pressed="statusFilter === f.value"
            @click="statusFilter = f.value"
          >{{ t(f.labelKey) }}</button>
        </div>
      </div>

      <div v-if="loading" style="padding: 24px; text-align: center;" class="text-muted">{{ t('common.loading') }}</div>
      <div v-else-if="visibleItems.length === 0" style="padding: 24px; text-align: center;" class="text-muted">
        {{ t('review.empty') }}
      </div>

      <div v-for="item in visibleItems" :key="item.key" class="review-row">
        <!-- 摘要行可点击展开：152 条政策全展开会有几千像素，
             默认折叠才能一眼扫完「哪些还没人看」。 -->
        <button
          class="review-head review-head-btn"
          :aria-expanded="expanded.has(item.key)"
          @click="toggleExpanded(item.key)"
        >
          <span class="expand-caret" aria-hidden="true">{{ expanded.has(item.key) ? '▾' : '▸' }}</span>
          <strong>{{ item.title }}</strong>
          <span class="tag" :class="statusClass(item.status)">{{ statusLabel(item.status) }}</span>
          <span v-if="item.ai_verdict" class="tag tag-secondary" style="font-size: var(--text-2xs);">
            {{ t('review.aiLabel') }}: {{ aiLabel(item.ai_verdict) }}
          </span>
          <span v-if="item.stale" class="tag tag-warning" style="font-size: var(--text-2xs);">{{ t('review.stale') }}</span>
          <span v-if="item.reviewed_at" class="review-meta" style="margin-left: auto;">
            {{ t('review.reviewedAt', { time: item.reviewed_at }) }}
          </span>
        </button>

        <template v-if="expanded.has(item.key)">
        <!-- 待审内容摘要：审核员必须能看见「审的是什么」 -->
        <p class="review-preview">{{ item.preview }}</p>
        <p v-if="item.ai_note" class="review-note">{{ item.ai_note }}</p>
        <p v-if="item.human_note" class="review-note">{{ t('review.humanNoteLabel') }}{{ item.human_note }}</p>

        <div class="review-actions">
          <input
            class="form-input"
            style="max-width: 320px;"
            v-model="notes[item.key]"
            :placeholder="noteRequired(item) ? t('review.noteRequired') : t('review.notePlaceholder')"
            :aria-label="t('review.notePlaceholder')"
          />
          <button class="btn btn-primary btn-sm" :disabled="busy" @click="submit(item.key, 'approved')">{{ t('review.approve') }}</button>
          <button class="btn btn-danger btn-sm" :disabled="busy" @click="submit(item.key, 'rejected')">{{ t('review.reject') }}</button>
          <button class="btn btn-secondary btn-sm" :disabled="busy" @click="submit(item.key, 'needs_edit')">{{ t('review.needsEdit') }}</button>
          <button class="btn btn-secondary btn-sm" :disabled="busy" @click="showHistory(item.key)">{{ t('review.history') }}</button>
        </div>

        </template>

        <div v-if="historyFor === item.key" class="review-history">
          <p v-if="history.length === 0" class="text-muted">{{ t('review.noHistory') }}</p>
          <table v-else class="asset-table">
            <thead>
              <tr>
                <th>{{ t('review.colTime') }}</th>
                <th>{{ t('review.colBy') }}</th>
                <th>{{ t('review.colAi') }}</th>
                <th>{{ t('review.colHuman') }}</th>
                <th>{{ t('review.colNote') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(r, i) in history" :key="i">
                <td>{{ r.created_at }}</td>
                <td>{{ r.decided_by }}</td>
                <td>{{ aiLabel(r.ai_verdict) }}</td>
                <td>{{ r.human_verdict ? humanLabel(r.human_verdict) : '-' }}</td>
                <td>{{ r.human_note || r.ai_note || '-' }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <p v-if="error" class="card" role="alert" style="border-color: var(--danger); color: var(--danger-strong);">{{ error }}</p>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { invoke } from '../api/invoke';
import { useI18n } from '../i18n';
import type { ApiResponse, ReviewProfile, ReviewRecord, ReviewItemState } from '../types';

const { t } = useI18n();
const profiles = ref<ReviewProfile[]>([]);
const activeId = ref('');
const payloadFields = ref<string[]>([]);
const items = ref<ReviewItemState[]>([]);
const loading = ref(false);
const busy = ref(false);
const error = ref('');
const message = ref('');
const aiNotice = ref('');
const progress = ref<{ total: number; approved: number; awaiting_human: number; pending: number } | null>(null);
const statusFilter = ref('all');
const historyFor = ref('');
const expanded = ref<Set<string>>(new Set());

function toggleExpanded(key: string) {
  const next = new Set(expanded.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expanded.value = next;
}
const history = ref<ReviewRecord[]>([]);
const notes = reactive<Record<string, string>>({});

const filters = [
  { value: 'all', labelKey: 'review.filterAll' },
  { value: 'pending', labelKey: 'review.filterPending' },
  { value: 'awaiting_human', labelKey: 'review.filterAwaiting' },
  { value: 'approved', labelKey: 'review.filterApproved' },
  { value: 'rejected', labelKey: 'review.filterRejected' },
];

const activeProfile = computed(() => profiles.value.find(p => p.id === activeId.value) || null);

/** 严格模式：逐项必检 + 疑点通过须写理由 */
const isStrict = computed(() => activeProfile.value?.strictness === 'strict');

/** AI 报过疑点的条目，人工「通过」时必须写理由（后端也会拦）。 */
function noteRequired(item: ReviewItemState): boolean {
  return isStrict.value
    && (item.ai_verdict === 'flag' || item.ai_verdict === 'unknown');
}
const visibleItems = computed(() =>
  statusFilter.value === 'all'
    ? items.value
    : items.value.filter(i => i.status === statusFilter.value),
);

const STATUS_KEYS = new Set(['pending', 'awaiting_human', 'approved', 'rejected', 'needs_edit']);

function statusLabel(s: string): string {
  if (!STATUS_KEYS.has(s)) return '—';
  return t(`review.status.${s}`);
}
function statusClass(s: string): string {
  if (s === 'approved') return 'tag-success';
  if (s === 'rejected') return 'tag-danger';
  if (s === 'awaiting_human') return 'tag-warning';
  return 'tag-secondary';
}
/** 未知值一律显示「—」，绝不把 i18n 键名（review.ai.null）露给用户。 */
const AI_KEYS = new Set(['pass', 'flag', 'unknown', 'skipped']);
const HUMAN_KEYS = new Set(['approved', 'rejected', 'needs_edit']);

function aiLabel(v: string | null | undefined): string {
  if (!v || !AI_KEYS.has(v)) return '—';
  return t(`review.ai.${v}`);
}
function humanLabel(v: string | null | undefined): string {
  if (!v || !HUMAN_KEYS.has(v)) return '—';
  return t(`review.human.${v}`);
}

async function loadProfiles() {
  try {
    const res = await invoke<ApiResponse<ReviewProfile[]>>('list_review_profiles');
    if (res.success && res.data) {
      profiles.value = res.data;
      if (res.data.length > 0 && !activeId.value) {
        await selectProfile(res.data[0].id);
      }
    } else {
      error.value = res.message;
    }
  } catch (e) {
    error.value = String(e);
  }
}

async function selectProfile(id: string) {
  activeId.value = id;
  statusFilter.value = 'all';
  historyFor.value = '';
  message.value = '';
  aiNotice.value = '';
  try {
    const [fields, list] = await Promise.all([
      invoke<ApiResponse<string[]>>('review_payload_fields', { profileId: id }),
      invoke<ApiResponse<ReviewItemState[]>>('list_review_items', { profileId: id }),
    ]);
    payloadFields.value = fields.data ?? [];
    items.value = list.data ?? [];
    await loadProgress();
  } catch (e) {
    error.value = String(e);
  }
}

/** 审核进度摘要：让用户知道「还差多少」，而不是一条条数。 */
async function loadProgress() {
  if (!activeId.value) return;
  try {
    const res = await invoke<ApiResponse<typeof progress.value>>('review_progress', { profileId: activeId.value });
    if (res.success && res.data) progress.value = res.data;
  } catch {
    progress.value = null;
  }
}

async function loadItems() {
  if (!activeId.value) return;
  loading.value = true;
  try {
    const res = await invoke<ApiResponse<ReviewItemState[]>>('list_review_items', { profileId: activeId.value });
    if (res.success && res.data) items.value = res.data;
    else error.value = res.message;
    await loadProgress();
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function runAi() {
  if (!activeId.value) return;
  busy.value = true;
  error.value = '';
  message.value = '';
  aiNotice.value = '';
  try {
    const res = await invoke<ApiResponse<ReviewItemState[]>>('run_ai_review', { profileId: activeId.value });
    if (res.success && res.data) {
      items.value = res.data;
      message.value = res.message;
      await loadProgress();
      // 后端在未配置 AI 时会回退为纯人工，这里如实告知
      if (res.message.includes('纯人工')) aiNotice.value = res.message;
    } else {
      error.value = res.message;
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function submit(key: string, verdict: string) {
  if (!activeId.value) return;
  // 前端先拦一次：不让用户点了「通过」再被后端拒绝
  const target = items.value.find(i => i.key === key);
  if (verdict === 'approved' && target && noteRequired(target) && !(notes[key] || '').trim()) {
    error.value = t('review.noteRequiredError');
    return;
  }
  busy.value = true;
  error.value = '';
  try {
    const res = await invoke<ApiResponse<null>>('submit_human_review', {
      profileId: activeId.value,
      itemKey: key,
      verdict,
      note: notes[key] || null,
    });
    if (res.success) {
      notes[key] = '';
      await loadItems();
    } else {
      error.value = res.message;
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function showHistory(key: string) {
  if (!activeId.value) return;
  historyFor.value = historyFor.value === key ? '' : key;
  if (historyFor.value !== key) return;
  try {
    const res = await invoke<ApiResponse<ReviewRecord[]>>('review_history', {
      profileId: activeId.value,
      itemKey: key,
    });
    history.value = res.data ?? [];
  } catch (e) {
    error.value = String(e);
  }
}

onMounted(loadProfiles);
</script>

<style scoped>
.payload-list {
  font-size: var(--text-xs);
  color: var(--gray-600);
  margin: 8px 0 0 20px;
  columns: 2;
}
.review-row {
  padding: 14px 0;
  border-bottom: 1px solid var(--gray-200, #e8e4de);
}
.review-head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.review-head-btn {
  width: 100%;
  background: none;
  border: none;
  padding: 4px 0;
  font: inherit;
  color: inherit;
  text-align: left;
  cursor: pointer;
  border-radius: 6px;
}
.review-head-btn:hover { background: var(--gray-50); }
.expand-caret { color: var(--gray-400); font-size: var(--text-2xs); width: 10px; }
.review-preview {
  font-size: var(--text-xs);
  color: var(--gray-600);
  background: var(--gray-50);
  border-left: 2px solid var(--gray-300, #d6cfc4);
  border-radius: 4px;
  padding: 8px 10px;
  margin-top: 8px;
  line-height: 1.6;
  max-height: 120px;
  overflow-y: auto;
  white-space: pre-wrap;
}
.review-note {
  font-size: var(--text-sm);
  color: var(--gray-600);
  margin-top: 6px;
  white-space: pre-wrap;
}
.review-meta {
  font-size: var(--text-2xs);
  color: var(--gray-500);
  margin-top: 4px;
}
.review-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  align-items: center;
  margin-top: 10px;
}
.review-history {
  margin-top: 12px;
  padding: 10px;
  background: var(--gray-50);
  border-radius: 8px;
  overflow-x: auto;
}
</style>
