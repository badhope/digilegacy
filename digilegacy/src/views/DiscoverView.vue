<template>
  <div class="discover fade-in">
    <!-- 步骤指示器 -->
    <div class="card mb-32 step-indicator">
      <div v-for="(step, idx) in steps" :key="idx" class="step-item" :class="{ active: currentStep >= idx }">
        <div class="step-circle">{{ idx + 1 }}</div>
        <span>{{ t(step) }}</span>
      </div>
    </div>

    <!-- 步骤0：启发式自检问卷 -->
    <div class="card mb-32" v-if="currentStep === 0">
      <div class="card-title" style="font-size: 18px;">{{ t('discover.checkTitle') }}</div>
      <div class="card-subtitle">{{ t('discover.checkSubtitle') }}</div>
      <div class="checklist" v-for="(item, idx) in checklistItems" :key="idx">
        <input :id=" 'check-' + idx" type="checkbox" v-model="checkedItems[item.value]" />
        <label :for=" 'check-' + idx" style="font-size: 14px; margin-left: 8px;">{{ t(item.textKey) }}</label>
      </div>
      <div class="checklist-footer">
        {{ t('discover.checked') }} <strong>{{ checkedCount }} / {{ checklistItems.length }}</strong> {{ t('discover.itemSuffix') }}
        <button @click="goToCategoriesFromChecklist" class="btn btn-primary" style="margin-left: 16px;">{{ t('discover.continue') }} →</button>
      </div>
    </div>

    <!-- 步骤1：资产大类选择 -->
    <div class="card mb-32" v-if="currentStep === 1">
      <div class="card-title" style="font-size: 18px;">{{ t('discover.catTitle') }}</div>
      <div class="card-subtitle">{{ t('discover.catSubtitle') }}</div>
    </div>

    <div class="card mb-16" v-if="currentStep === 1 && prefillCount > 0" role="status">
      <p style="font-size: 13px; color: var(--gray-600);">
        {{ t('discover.prefillHint', { count: prefillCount }) }}
        <button class="btn btn-sm btn-secondary ml-8" @click="clearCategorySelection">{{ t('discover.clearSelection') }}</button>
      </p>
    </div>

    <div class="category-grid" v-if="currentStep === 1">
      <button v-for="cat in assetCategories" :key="cat.key"
           type="button"
           class="category-card" :class="{ selected: selectedCats.has(cat.key) }"
           :aria-pressed="selectedCats.has(cat.key)"
           @click="toggleCat(cat.key)">
        <div class="category-icon" aria-hidden="true"><PlatformIcon :name="cat.key" :size="34" /></div>
        <div class="category-name">{{ t(cat.nameKey) }}</div>
        <div class="category-desc">{{ t(cat.descKey) }}</div>
        <div class="category-check" v-if="selectedCats.has(cat.key)" aria-hidden="true">✓</div>
      </button>
    </div>

    <div class="card mt-24" v-if="currentStep === 1" style="text-align: center;">
      <p class="text-muted mb-16">{{ t('discover.selectedCats') }} <strong>{{ selectedCats.size }}</strong> {{ t('discover.catSuffix') }}</p>
      <button class="btn btn-primary btn-lg" :disabled="selectedCats.size === 0" @click="goToStep(2)">
        {{ t('discover.nextPlatforms') }} →
      </button>
    </div>

    <!-- 步骤2：平台勾选 + 资产登记 -->
    <div v-if="currentStep === 2">
      <div class="card mb-24" style="display: flex; justify-content: space-between; align-items: center; gap: 16px; flex-wrap: wrap;">
        <div>
          <div class="card-title">{{ t('discover.platformsTitle') }}</div>
          <div class="card-subtitle">{{ t('discover.platformsSubtitle', { count: filteredPlatformCount }) }} <strong>{{ platforms.selectedIds.size }}</strong> {{ t('discover.selectedCount') }}</div>
        </div>
        <div class="flex gap-8" style="flex-wrap: wrap;">
          <div class="search-box">
            <span class="search-icon" aria-hidden="true"><SearchOutline /></span>
            <input v-model="platformKeyword" class="form-input search-input" type="search" :placeholder="t('discover.searchPlaceholder')" :aria-label="t('discover.searchLabel')" />
          </div>
          <button class="btn btn-sm btn-secondary" @click="goToStep(1)">← {{ t('discover.backCategories') }}</button>
        </div>
      </div>

      <!-- 按大类分组展示平台（可折叠，默认展开） -->
      <CollapseGroup
        v-for="cat in selectedCategoriesWithPlatforms"
        :key="cat.key"
        :title="t(cat.nameKey)"
        :count="cat.platforms.length"
        :default-open="true"
      >
        <div class="platform-grid">
          <div v-for="platform in cat.platforms" :key="platform.id" class="platform-card"
               :class="{ selected: platforms.isSelected(platform.id) }"
               role="checkbox" :aria-checked="platforms.isSelected(platform.id)" tabindex="0"
               :aria-label="platform.name + '（' + (platform.inheritable ? t('discover.inheritable') : t('discover.hardInherit')) + '）'"
               @click="platforms.toggle(platform.id)"
               @keydown.enter.prevent="platforms.toggle(platform.id)"
               @keydown.space.prevent="platforms.toggle(platform.id)">
            <div class="platform-card-header">
              <div class="minw-0">
                <div class="platform-name ellipsis">{{ platform.name }}</div>
                <div class="platform-type">{{ platform.subtype }}</div>
              </div>
              <div class="platform-check">{{ platforms.isSelected(platform.id) ? '✓' : '+' }}</div>
            </div>
            <div class="platform-meta">
              <span class="tag" :class="platform.inheritable ? 'tag-success' : 'tag-danger'">
                {{ platform.inheritable ? t('discover.inheritable') : t('discover.hardInherit') }}
              </span>
              <span class="tag tag-warning" v-if="platform.has_property">{{ t('discover.hasProperty') }}</span>
              <span class="tag tag-personality" v-if="platform.has_personality">{{ t('discover.hasPersonality') }}</span>
            </div>
            <div class="platform-process clamp-3"><strong>{{ t('discover.processLabel') }}：</strong>{{ platform.process_description }}</div>
          </div>
        </div>
      </CollapseGroup>

      <div class="card" style="text-align: center;">
        <button class="btn btn-primary btn-lg" :disabled="platforms.selectedIds.size === 0" @click="goToStep(3)">
          {{ t('discover.nextDetails') }} ({{ platforms.selectedIds.size }}) →
        </button>
      </div>
    </div>

    <!-- 步骤3：资产详情登记 -->
    <div v-if="currentStep === 3">
      <div class="card mb-24">
        <div class="card-title" style="font-size: 18px;">{{ t('discover.detailsTitle') }}</div>
        <div class="card-subtitle">{{ t('discover.detailsSubtitle') }}</div>
      </div>

      <div v-for="platform in selectedPlatformList" :key="platform.id" class="card mb-16">
        <div class="flex items-center gap-12 mb-16">
          <div class="platform-icon" aria-hidden="true"><PlatformIcon :name="platform.category" :size="26" /></div>
          <div>
            <strong>{{ platform.name }}</strong>
            <span class="tag ml-12" :class="platform.inheritable ? 'tag-success' : 'tag-danger'">
              {{ platform.inheritable ? t('discover.inheritable') : t('discover.hardInherit') }}
            </span>
          </div>
        </div>

        <div class="grid grid-2">
          <div class="form-group">
            <label class="form-label" :for="'identifier-' + platform.id">{{ t('discover.identifierLabel') }}</label>
            <input class="form-input" :id="'identifier-' + platform.id" v-model="assetForms[platform.id].identifier" :placeholder="t('discover.identifierPlaceholder')" />
          </div>
          <div class="form-group">
            <label class="form-label" :for="'category-' + platform.id">{{ t('discover.categoryLabel') }}</label>
            <select class="form-select" :id="'category-' + platform.id" v-model="assetForms[platform.id].category">
              <option value="property">{{ t('discover.catProperty') }}</option>
              <option value="personality">{{ t('discover.catPersonality') }}</option>
              <option value="mixed">{{ t('discover.catMixed') }}</option>
            </select>
          </div>
        </div>

        <div class="grid grid-2">
          <div class="form-group">
            <label class="form-label" :for="'value-' + platform.id">{{ t('discover.valueLabel') }}</label>
            <input class="form-input" :id="'value-' + platform.id" type="number" v-model.number="assetForms[platform.id].value" :placeholder="t('discover.valuePlaceholder')" />
          </div>
          <div class="form-group">
            <label class="form-label" :for="'disposal-' + platform.id">{{ t('discover.disposalLabel') }}</label>
            <select class="form-select" :id="'disposal-' + platform.id" v-model="assetForms[platform.id].disposal">
              <option value="inherit">{{ t('discover.disposalInherit') }}</option>
              <option value="liquidate">{{ t('discover.disposalLiquidate') }}</option>
              <option value="memorialize">{{ t('discover.disposalMemorialize') }}</option>
              <option value="authorize_view">{{ t('discover.disposalAuthorize') }}</option>
              <option value="delete">{{ t('discover.disposalDelete') }}</option>
              <option value="no_action">{{ t('discover.disposalNone') }}</option>
            </select>
          </div>
        </div>

        <div class="grid grid-2" v-if="assetForms[platform.id].disposal === 'inherit'">
          <div class="form-group">
            <label class="form-label" :for="'heir-name-' + platform.id">{{ t('discover.heirNameLabel') }}</label>
            <input class="form-input" :id="'heir-name-' + platform.id" v-model="assetForms[platform.id].heirName" :placeholder="t('discover.heirNameLabel')" />
          </div>
          <div class="form-group">
            <label class="form-label" :for="'heir-contact-' + platform.id">{{ t('discover.heirContactLabel') }}</label>
            <input class="form-input" :id="'heir-contact-' + platform.id" v-model="assetForms[platform.id].heirContact" :placeholder="t('discover.heirContactPlaceholder')" />
          </div>
        </div>
      </div>

      <div class="card" style="text-align: center;">
        <p v-if="saveError" class="text-muted" role="alert" style="color: var(--danger); font-size: 13px; margin-bottom: 12px;">{{ saveError }}</p>
        <button class="btn btn-success btn-lg" :disabled="saving" @click="saveAllAssets">
          {{ saving ? t('common.saving') : t('discover.saveAll') }}
        </button>
        <button class="btn btn-secondary btn-lg ml-12" :disabled="saving" @click="goToStep(2)">← {{ t('discover.backSelect') }}</button>
      </div>    </div>

    <!-- 完成 -->
    <div v-if="currentStep === 4" class="card text-center" style="padding: 48px;">
      <div class="done-hero" aria-hidden="true"><CheckmarkCircleOutline /></div>
      <h2 style="font-size: 22px; margin-bottom: 8px;">{{ t('discover.doneTitle') }}</h2>
      <p class="text-muted mb-24" v-if="savedCount > 0">{{ t('discover.doneRegistered', { count: savedCount }) }}</p>
      <p class="text-muted mb-24" v-else>{{ t('discover.doneAllExisting') }}</p>
      <div style="display: flex; gap: 12px; justify-content: center;">
        <router-link to="/assets" class="btn btn-primary btn-lg">{{ t('discover.viewAssets') }}</router-link>
        <router-link to="/report" class="btn btn-secondary btn-lg">{{ t('discover.generateReport') }}</router-link>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted } from 'vue';

import { useI18n } from '../i18n';
import { usePlatformsStore } from '../stores/platforms';
import { useAssetsStore } from '../stores/assets';
import CheckmarkCircleOutline from '@vicons/ionicons5/es/CheckmarkCircleOutline';
import SearchOutline from '@vicons/ionicons5/es/SearchOutline';
import PlatformIcon from '../components/PlatformIcon.vue';
import CollapseGroup from '../components/CollapseGroup.vue';
import type { Platform } from '../types';

const { t } = useI18n();
const platforms = usePlatformsStore();
const assets = useAssetsStore();

const steps = ['discover.step1', 'discover.step2', 'discover.step3', 'discover.step4', 'discover.step5'];
const currentStep = ref(0);
const savedCount = ref(0);

// ── 启发式自检问卷项 ──
const checklistItems = [
  { value: 'bank', textKey: 'discover.check.bank', checkedInit: true },
  { value: 'credit_card', textKey: 'discover.check.credit_card', checkedInit: true },
  { value: 'crypto_wallet', textKey: 'discover.check.crypto_wallet', checkedInit: false },
  { value: 'social_media', textKey: 'discover.check.social_media', checkedInit: true },
  { value: 'email', textKey: 'discover.check.email', checkedInit: true },
  { value: 'gaming', textKey: 'discover.check.gaming', checkedInit: true },
  { value: 'cloud_storage', textKey: 'discover.check.cloud_storage', checkedInit: true },
  { value: 'domain_website', textKey: 'discover.check.domain_website', checkedInit: false },
  { value: 'digital_subscriptions', textKey: 'discover.check.digital_subscriptions', checkedInit: false },
  { value: 'photos_files', textKey: 'discover.check.photos_files', checkedInit: true },
];

const checkedItems = reactive<Record<string, boolean>>({});
checklistItems.forEach(item => {
  checkedItems[item.value] = !!item.checkedInit;
});

const checkedCount = computed(() => {
  return checklistItems.filter(item => checkedItems[item.value]).length;
});

// 问卷答案 → 资产大类。旧实现只把答案用来显示「已勾选 7/10」，
// 勾了什么都不影响下一步，问卷纯属装饰。现在把勾选项变成下一步的预选，
// 用户仍可自由增减——这是「帮你想起」而不是「替你决定」。
const CHECK_TO_CATEGORIES: Record<string, string[]> = {
  bank: ['支付金融', '金融证券'],
  credit_card: ['支付金融', '金融证券'],
  crypto_wallet: ['支付金融'],
  social_media: ['社交通讯', '内容创作'],
  email: ['社交通讯'],
  gaming: ['游戏'],
  cloud_storage: ['知识付费', '工具订阅'],
  domain_website: ['内容创作', '工具订阅'],
  digital_subscriptions: ['工具订阅', '知识付费'],
  photos_files: ['知识付费'],
};

// ── 资产大类定义（静态引导数据）──
// 数据层实际使用 16 个分类（与 platform_data.rs 一致），向导展示层聚合为 11 个大类，
// 每个大类映射到一个或多个数据分类，保证勾选的类别能筛出对应平台。
interface AssetCategory {
  key: string;
  nameKey: string;
  descKey: string;
  cats: string[];
}

const assetCategories: AssetCategory[] = [
  { key: '支付金融', nameKey: 'discover.cat.支付金融.name', descKey: 'discover.cat.支付金融.desc', cats: ['支付金融'] },
  { key: '社交通讯', nameKey: 'discover.cat.社交通讯.name', descKey: 'discover.cat.社交通讯.desc', cats: ['社交通讯'] },
  { key: '游戏', nameKey: 'discover.cat.游戏.name', descKey: 'discover.cat.游戏.desc', cats: ['游戏'] },
  { key: '金融证券', nameKey: 'discover.cat.金融证券.name', descKey: 'discover.cat.金融证券.desc', cats: ['金融证券'] },
  { key: '知识付费', nameKey: 'discover.cat.知识付费.name', descKey: 'discover.cat.知识付费.desc', cats: ['知识付费', '音视频', '存储云服务'] },
  { key: '工具订阅', nameKey: 'discover.cat.工具订阅.name', descKey: 'discover.cat.工具订阅.desc', cats: ['工具订阅'] },
  { key: '电商', nameKey: 'discover.cat.电商.name', descKey: 'discover.cat.电商.desc', cats: ['电商'] },
  { key: '内容创作', nameKey: 'discover.cat.内容创作.name', descKey: 'discover.cat.内容创作.desc', cats: ['内容创作'] },
  { key: '出行住宿', nameKey: 'discover.cat.出行住宿.name', descKey: 'discover.cat.出行住宿.desc', cats: ['出行住宿'] },
  { key: '生活工具', nameKey: 'discover.cat.生活工具.name', descKey: 'discover.cat.生活工具.desc', cats: ['生活工具', '生活服务'] },
  { key: '政务服务', nameKey: 'discover.cat.政务服务.name', descKey: 'discover.cat.政务服务.desc', cats: ['政务服务'] },
  { key: '学习与考试', nameKey: 'discover.cat.学习与考试.name', descKey: 'discover.cat.学习与考试.desc', cats: ['学习与考试'] },
];

const selectedCats = ref<Set<string>>(new Set());

function toggleCat(key: string) {
  if (selectedCats.value.has(key)) {
    selectedCats.value.delete(key);
  } else {
    selectedCats.value.add(key);
  }
}

// 按大类的平台列表（用于展示勾选卡片）
const filteredPlatformCount = computed(() => {
  const dataCats = selectedCats.value.size === 0
    ? null
    : new Set(assetCategories.filter(c => selectedCats.value.has(c.key)).flatMap(c => c.cats));
  const terms = platformKeyword.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return platforms.platforms.filter(p => {
    if (dataCats && !dataCats.has(p.category)) return false;
    if (terms.length === 0) return true;
    const haystack = `${p.name} ${p.subtype} ${p.category}`.toLowerCase();
    return terms.every(t => haystack.includes(t));
  }).length;
});

interface CategoryWithPlatforms extends AssetCategory { platforms: Platform[] }

const platformKeyword = ref('');

const selectedCategoriesWithPlatforms = computed((): CategoryWithPlatforms[] => {
  if (selectedCats.value.size === 0) return [];
  const terms = platformKeyword.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return assetCategories
    .filter(c => selectedCats.value.has(c.key))
    .map(c => ({
      ...c,
      platforms: platforms.platforms.filter(p => {
        if (!c.cats.includes(p.category)) return false;
        if (terms.length === 0) return true;
        const haystack = `${p.name} ${p.subtype} ${p.category}`.toLowerCase();
        return terms.every(t => haystack.includes(t));
      }),
    }))
    .filter(c => c.platforms.length > 0);
});

// ── 资产表单 ──
interface AssetForm {
  identifier: string;
  category: string;
  value: number | null;
  disposal: string;
  heirName: string;
  heirContact: string;
}

const assetForms = reactive<Record<number, AssetForm>>({});
const saving = ref(false);
const saveError = ref('');

const selectedPlatformList = computed(() =>
  platforms.platforms.filter(p => platforms.isSelected(p.id))
);

const registeredPlatformIds = computed(() =>
  new Set(assets.assets.map(a => a.platform_id))
);

function goToStep(step: number) {
  currentStep.value = step;
}

/** 依据问卷答案预选大类（不清空用户已手动选过的项）。 */
function prefillCategoriesFromChecklist() {
  const next = new Set(selectedCats.value);
  for (const item of checklistItems) {
    if (!checkedItems[item.value]) continue;
    for (const cat of CHECK_TO_CATEGORIES[item.value] ?? []) next.add(cat);
  }
  selectedCats.value = next;
}

const prefillCount = ref(0);

function goToCategoriesFromChecklist() {
  prefillCategoriesFromChecklist();
  prefillCount.value = selectedCats.value.size;
  currentStep.value = 1;
}

function clearCategorySelection() {
  selectedCats.value = new Set();
  prefillCount.value = 0;
}

async function saveAllAssets() {
  if (saving.value) return;
  // 过滤掉已登记过的平台，避免重复登记
  const pending = selectedPlatformList.value.filter(p => !registeredPlatformIds.value.has(p.id));
  if (pending.length === 0) {
    goToStep(4);
    savedCount.value = 0;
    return;
  }
  // 批量登记前先整体校验：中途失败会留下「部分已写入」的半个批次，
  // 用户无法判断哪些成功了。宁可在写入前一次性拦下。
  for (const platform of pending) {
    const form = assetForms[platform.id] || { identifier: '', category: 'mixed', value: null, disposal: 'no_action', heirName: '', heirContact: '' };
    if (form.disposal === 'inherit' && !form.heirName.trim()) {
      saveError.value = `${platform.name}：${t('detail.heirRequired')}`;
      return;
    }
  }
  saving.value = true;
  saveError.value = '';
  let count = 0;
  try {
    for (const platform of pending) {
      const form = assetForms[platform.id] || { identifier: '', category: 'mixed', value: null, disposal: 'no_action', heirName: '', heirContact: '' };
      await assets.addAsset({
        platform_id: platform.id,
        account_identifier: form.identifier || t('discover.notFilled'),
        account_description: platform.subtype || '',
        category: form.category,
        estimated_value: form.value,
        disposal_method: form.disposal,
        heir_name: form.heirName || undefined,
        heir_contact: form.heirContact || undefined,
        notes: platform.notes || '',
      });
      count++;
    }
    savedCount.value = count;
    platforms.clearSelection();
    goToStep(4);
  } catch (e) {
    saveError.value = t('discover.saveError', { ok: count, total: pending.length }) + String(e);
  } finally {
    saving.value = false;
  }
}

onMounted(async () => {
  await Promise.all([platforms.loadPlatforms(), assets.loadAssets()]);
  // 初始化表单
  platforms.platforms.forEach(p => {
    if (!assetForms[p.id]) {
      assetForms[p.id] = {
        identifier: '',
        category: p.has_property && p.has_personality ? 'mixed' : (p.has_property ? 'property' : 'personality'),
        value: null,
        // 默认「待设定」而不是「指定继承」：默认继承会让新建资产静默算作
        // 「已规划」，同时因为没填继承人而无法保存——两个问题一起消失。
        disposal: 'no_action',
        heirName: '',
        heirContact: '',
      };
    }
  });
});
</script>

<style scoped>
.step-indicator { display: flex; gap: 16px; align-items: center; }
.search-box { position: relative; width: 220px; }
.search-icon { position: absolute; left: 10px; top: 50%; transform: translateY(-50%); font-size: 13px; opacity: 0.7; pointer-events: none; }
.search-input { padding-left: 30px; }
.step-item { flex: 1; display: flex; align-items: center; gap: 8px; }
.step-circle {
  width: 32px; height: 32px; border-radius: 50%; display: flex; align-items: center;
  justify-content: center; font-weight: 700; font-size: 14px; background: var(--gray-200); color: var(--gray-700);
}
.step-item.active .step-circle { background: var(--solid-primary); color: var(--on-solid); }

.category-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 16px;
}
.category-card {
  background: var(--surface);
  border: 2px solid var(--gray-200);
  border-radius: 12px;
  padding: 24px 20px;
  text-align: center;
  cursor: pointer;
  transition: all 0.2s;
  position: relative;
  /* 由 div 改为 button 后抵消浏览器默认样式，保持视觉不变 */
  font: inherit;
  color: inherit;
  width: 100%;
}
.category-card:hover, .category-card:focus-visible { border-color: var(--primary); transform: translateY(-2px); box-shadow: 0 4px 12px rgba(0,0,0,0.08); }
.category-card.selected { border-color: var(--primary); background: var(--primary-light); }
.category-icon { font-size: 36px; margin-bottom: 8px; }
.category-name { font-size: 15px; font-weight: 600; margin-bottom: 4px; }
.category-desc { font-size: 12px; color: var(--gray-600); line-height: 1.4; }
.category-check {
  position: absolute; top: 8px; right: 8px; width: 28px; height: 28px;
  border-radius: 50%; background: var(--solid-primary); color: var(--on-solid); display: flex;
  align-items: center; justify-content: center; font-size: 14px; font-weight: 700;
}

.platform-icon { font-size: 24px; }
.platform-check {
  width: 28px; height: 28px; border-radius: 6px; display: flex; align-items: center;
  justify-content: center; font-size: 14px; background: var(--gray-100);
}
.platform-card.selected .platform-check { background: var(--solid-primary); color: var(--on-solid); }
</style>
