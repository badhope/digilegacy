<template>
  <div class="assets fade-in">
    <div class="flex justify-between items-center mb-32">
      <div>
        <h2 style="font-size: 22px; font-weight: 700;">{{ t('assets.title') }}</h2>
        <p class="text-muted mt-16" style="font-size: 14px;">{{ t('assets.summary', { count: assets.assets.length, value: formatMoney(assets.totalValue) }) }}</p>
      </div>
      <router-link to="/discover" class="btn btn-primary">+ {{ t('assets.continue') }}</router-link>
    </div>

    <div class="card mb-24">
      <div class="flex items-center gap-12 mb-16" style="flex-wrap: wrap;">
        <div class="search-box">
          <span class="search-icon" aria-hidden="true"><SearchOutline /></span>
          <input
            v-model="keyword"
            class="form-input search-input"
            type="search"
            :placeholder="t('assets.searchPlaceholder')"
            :aria-label="t('assets.searchLabel')"
          />
          <button v-if="keyword" class="search-clear" :aria-label="t('assets.clearSearch')" @click="keyword = ''">×</button>
        </div>
        <select v-model="sortBy" class="form-select" style="width: auto;" :aria-label="t('assets.sortLabel')">
          <option value="recent">{{ t('assets.sortRecent') }}</option>
          <option value="value">{{ t('assets.sortValue') }}</option>
          <option value="platform">{{ t('assets.sortPlatform') }}</option>
          <option value="disposal">{{ t('assets.sortDisposal') }}</option>
        </select>
      </div>
      <div class="flex items-center gap-12" style="flex-wrap: wrap;" role="group" :aria-label="t('assets.filterLabel')">
        <button class="btn btn-sm" :class="filter === 'all' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="filter === 'all'" @click="filter = 'all'">{{ t('assets.all') }} ({{ assets.assets.length }})</button>
        <button class="btn btn-sm" :class="filter === 'property' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="filter === 'property'" @click="filter = 'property'">{{ t('cat.property') }} ({{ assets.propertyCount }})</button>
        <button class="btn btn-sm" :class="filter === 'personality' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="filter === 'personality'" @click="filter = 'personality'">{{ t('cat.personality') }} ({{ assets.personalityCount }})</button>
        <button class="btn btn-sm" :class="filter === 'mixed' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="filter === 'mixed'" @click="filter = 'mixed'">{{ t('cat.mixed') }} ({{ assets.mixedCount }})</button>
        <!-- 待完善：把所有「家人拿到也办不了」的资产集中起来，一次改完 -->
        <!-- 有缺口时用警示色：这是用户最该点的一格，灰色按钮会被直接跳过 -->
        <button
          class="btn btn-sm"
          :class="filter === 'incomplete' ? 'btn-primary' : (incompleteCount > 0 ? 'btn-warning-outline' : 'btn-secondary')"
          :aria-pressed="filter === 'incomplete'"
          :disabled="incompleteCount === 0"
          @click="filter = filter === 'incomplete' ? 'all' : 'incomplete'"
        >{{ t('assets.filterIncomplete') }} ({{ incompleteCount }})</button>
        <span class="text-muted" style="font-size: 12px; margin-left: auto;" aria-live="polite">
          {{ t('assets.showing', { shown: filteredAssets.length, total: assets.assets.length }) }}
        </span>
      </div>
    </div>

    <div v-for="asset in filteredAssets" :key="asset.id" class="card mb-16 asset-row"
         role="link" tabindex="0" :aria-label="t('assets.viewDetail', { name: asset.platform_name })"
         @click="$router.push(`/assets/${asset.id}`)"
         @keydown.enter="$router.push(`/assets/${asset.id}`)">
      <div style="display: flex; justify-content: space-between; align-items: flex-start;">
        <div style="display: flex; gap: 16px;">
          <div class="asset-icon"><PlatformIcon :name="asset.platform_name" :size="24" /></div>
          <div>
            <strong style="font-size: 16px;">{{ asset.platform_name }}</strong>
            <p class="text-muted" style="font-size: 13px; margin-top: 2px;">{{ asset.account_identifier }}</p>
            <div style="display: flex; gap: 8px; margin-top: 8px;">
              <span class="tag" :class="'tag-' + asset.category">{{ categoryLabel(asset.category) }}</span>
              <span class="tag" :class="asset.disposal_method !== 'no_action' ? 'tag-success' : 'tag-warning'">{{ disposalLabel(asset.disposal_method) }}</span>
              <span class="tag tag-success" v-if="asset.has_credential"><LockClosedOutline /> {{ t('assets.hasCredential') }}</span>
              <!-- 缺什么要写在行上：用户点进详情才知道哪里缺，等于白筛 -->
              <span v-for="gap in gapsOf(asset)" :key="gap" class="tag tag-warning">{{ t('assets.gap.' + gap) }}</span>
            </div>
          </div>
        </div>
        <div style="text-align: right;">
          <div style="font-size: 18px; font-weight: 700; color: var(--gray-900);">{{ asset.estimated_value ? '¥' + formatMoney(asset.estimated_value) : '-' }}</div>
          <div class="text-muted" style="font-size: 12px;">{{ t('assets.estimatedValueLabel') }}</div>
        </div>
      </div>
    </div>

    <!-- 有数据但筛不出来：说清楚是筛选结果，不要让人以为资产没了 -->
    <div v-if="!assets.loading && !assets.loadError && assets.assets.length > 0 && filteredAssets.length === 0" class="card text-center" style="padding: 40px;">
      <EmptyState kind="search" :size="140" />
      <h3 style="font-size: 16px; margin: 12px 0 8px;">{{ t('assets.noMatchTitle') }}</h3>
      <p class="text-muted mb-16" style="font-size: 13px;">{{ t('assets.noMatchDesc') }}</p>
      <button class="btn btn-secondary btn-sm" @click="resetFilters">{{ t('assets.resetFilter') }}</button>
    </div>

    <!-- 首次加载骨架屏：避免「先闪空态再出内容」的跳动 -->
    <div v-if="assets.loading" class="skeleton-list" aria-busy="true" aria-live="polite">
      <div v-for="n in 3" :key="n" class="card mb-16" style="padding: 20px;">
        <div class="skeleton skeleton-line w-40"></div>
        <div class="skeleton skeleton-line w-60"></div>
      </div>
    </div>

    <!-- 加载失败：必须与「成功但为空」区分，否则用户会误以为资产全部丢失 -->
    <div v-else-if="assets.loadError" class="card" role="alert" style="border-color: var(--danger);">
      <h3 style="font-size: 16px; margin-bottom: 8px; color: var(--danger-strong);">{{ t('assets.loadFailed') }}</h3>
      <p class="text-muted mb-16" style="font-size: 13px;">{{ assets.loadError }}</p>
      <button class="btn btn-primary btn-sm" @click="assets.loadAssets()">{{ t('common.retry') }}</button>
    </div>

    <div class="card text-center" v-else-if="assets.assets.length === 0" style="padding: 48px;">
      <EmptyState kind="assets" :size="160" />
      <h3 style="font-size: 18px; margin: 16px 0 8px;">{{ t('assets.emptyTitle') }}</h3>
      <p class="text-muted mb-24" style="font-size: 14px;">{{ t('assets.emptyDesc') }}</p>
      <router-link to="/discover" class="btn btn-primary btn-lg">{{ t('assets.startDiscover') }}</router-link>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useI18n } from '../i18n';
import SearchOutline from '@vicons/ionicons5/es/SearchOutline';
import LockClosedOutline from '@vicons/ionicons5/es/LockClosedOutline';
import { useAssetsStore } from '../stores/assets';
import type { DigitalAsset } from '../types';
import { formatMoney, categoryLabel, disposalLabel } from '../utils/format';
import PlatformIcon from '../components/PlatformIcon.vue';
import EmptyState from '../components/EmptyState.vue';

const { t } = useI18n();
const assets = useAssetsStore();
const filter = ref<string>('all');
const keyword = ref('');
const sortBy = ref<'recent' | 'value' | 'platform' | 'disposal'>('recent');

/**
 * 「待完善」的判定与仪表板待办同源：
 * 选了继承却没继承人、账号标识为空、需要交接但没有凭据。
 * 三条都直接影响家人能否执行，不是风格问题。
 */
function isIncomplete(a: DigitalAsset): boolean {
  const noHeir = a.disposal_method === 'inherit' && !(a.heir_name || '').trim();
  const noIdent = !(a.account_identifier || '').trim();
  const needsCred = ['inherit', 'liquidate', 'authorize_view'].includes(a.disposal_method);
  const noCred = needsCred && !a.has_credential;
  return noHeir || noIdent || noCred;
}

const incompleteCount = computed(() => assets.assets.filter(isIncomplete).length);

/** 逐条列出该资产缺什么，用于行内提示。 */
function gapsOf(a: DigitalAsset): string[] {
  const gaps: string[] = [];
  if (a.disposal_method === 'inherit' && !(a.heir_name || '').trim()) gaps.push('heir');
  if (!(a.account_identifier || '').trim()) gaps.push('identifier');
  if (['inherit', 'liquidate', 'authorize_view'].includes(a.disposal_method) && !a.has_credential) {
    gaps.push('credential');
  }
  return gaps;
}

/** 搜索匹配平台名 / 账号标识 / 备注；大小写不敏感，多关键词用空格分隔（AND）。 */
const filteredAssets = computed(() => {
  const byCat = filter.value === 'all'
    ? assets.assets
    : filter.value === 'incomplete'
      ? assets.assets.filter(isIncomplete)
      : assets.assets.filter(a => a.category === filter.value);

  const terms = keyword.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const matched = terms.length === 0
    ? byCat
    : byCat.filter(a => {
        const haystack = [a.platform_name, a.account_identifier, a.account_description, a.notes]
          .join('\n')
          .toLowerCase();
        return terms.every(t => haystack.includes(t));
      });

  const sorted = [...matched];
  switch (sortBy.value) {
    case 'value':
      sorted.sort((a, b) => (b.estimated_value || 0) - (a.estimated_value || 0));
      break;
    case 'platform':
      sorted.sort((a, b) => a.platform_name.localeCompare(b.platform_name, 'zh-CN'));
      break;
    case 'disposal':
      sorted.sort((a, b) => Number(a.disposal_method === 'no_action') - Number(b.disposal_method === 'no_action'));
      break;
    default:
      sorted.sort((a, b) => String(b.created_at).localeCompare(String(a.created_at)));
  }
  return sorted;
});

function resetFilters() {
  keyword.value = '';
  filter.value = 'all';
  sortBy.value = 'recent';
}

onMounted(async () => {
  if (assets.assets.length === 0) await assets.loadAssets();
});
</script>

<style scoped>
.asset-row { cursor: pointer; }
.asset-row:hover { border-color: var(--primary); }
.asset-icon { width: 48px; height: 48px; border-radius: 12px; background: var(--gray-100); display: flex; align-items: center; justify-content: center; font-size: 22px; flex-shrink: 0; }

.search-box {
  position: relative;
  flex: 1 1 260px;
  max-width: 420px;
}
.search-icon {
  position: absolute;
  left: 12px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 14px;
  pointer-events: none;
  opacity: 0.7;
}
.search-input {
  padding-left: 34px;
  padding-right: 32px;
}
.search-clear {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  border: none;
  background: transparent;
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  color: var(--gray-500);
  padding: 2px 4px;
  border-radius: 4px;
}
.search-clear:hover { color: var(--gray-800); background: var(--gray-100); }
</style>
