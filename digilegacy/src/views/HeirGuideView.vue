<template>
  <div class="heir fade-in">
    <h2 style="font-size: 22px; font-weight: 700; margin-bottom: 8px;">{{ t('heir.title') }}</h2>
    <p class="text-muted mb-32" style="font-size: 14px;">
      {{ t('heir.subtitle') }}
    </p>

    <!-- 使用前提：用 token 而非硬编码浅黄，深色主题下仍保证正文可读 -->
    <div class="card mb-32 heir-prereq">
      <div class="flex items-center gap-12">
        <span class="heir-warn-icon" aria-hidden="true"><WarningOutline /></span>
        <div>
          <strong style="font-size: 16px;">{{ t('heir.prereqTitle') }}</strong>
          <p style="font-size: 13px; margin-top: 4px; color: var(--gray-800);">
            {{ t('heir.prereqDesc') }}
          </p>
        </div>
      </div>
    </div>

    <!-- 继承步骤总览 -->
    <div class="card mb-32">
      <div class="card-title mb-16">{{ t('heir.flowTitle') }}</div>
      <div style="display: flex; flex-direction: column; gap: 12px;">
        <div class="flex items-center gap-12" style="padding: 10px 16px; background: var(--gray-50); border-radius: 8px;">
          <span style="width: 28px; height: 28px; border-radius: 50%; background: var(--solid-primary); color: var(--on-solid); display: flex; align-items: center; justify-content: center; font-size: 13px; font-weight: 700;">1</span>
          <span>{{ t('heir.flow1') }}</span>
        </div>
        <div class="flex items-center gap-12" style="padding: 10px 16px; background: var(--gray-50); border-radius: 8px;">
          <span style="width: 28px; height: 28px; border-radius: 50%; background: var(--solid-primary); color: var(--on-solid); display: flex; align-items: center; justify-content: center; font-size: 13px; font-weight: 700;">2</span>
          <span>{{ t('heir.flow2') }}</span>
        </div>
        <div class="flex items-center gap-12" style="padding: 10px 16px; background: var(--gray-50); border-radius: 8px;">
          <span style="width: 28px; height: 28px; border-radius: 50%; background: var(--solid-primary); color: var(--on-solid); display: flex; align-items: center; justify-content: center; font-size: 13px; font-weight: 700;">3</span>
          <span>{{ t('heir.flow3') }}</span>
        </div>
        <div class="flex items-center gap-12" style="padding: 10px 16px; background: var(--gray-50); border-radius: 8px;">
          <span style="width: 28px; height: 28px; border-radius: 50%; background: var(--solid-primary); color: var(--on-solid); display: flex; align-items: center; justify-content: center; font-size: 13px; font-weight: 700;">4</span>
          <span>{{ t('heir.flow4') }}</span>
        </div>
      </div>
    </div>

    <!-- Tab 切换 -->
    <div class="card mb-24">
      <div class="card-title mb-16">
        <div class="flex gap-4" role="tablist" :aria-label="t('heir.tabLabel')">
          <button class="tab-btn" :class="{ active: activeTab === 'guide' }" role="tab" :aria-selected="activeTab === 'guide'" @click="activeTab = 'guide'">{{ t('heir.tabGuide') }}</button>
          <button class="tab-btn" :class="{ active: activeTab === 'monetization' }" role="tab" :aria-selected="activeTab === 'monetization'" @click="activeTab = 'monetization'">{{ t('heir.tabMonetization') }}</button>
        </div>
      </div>

      <!-- Tab 1: 继承指引 (原内容) -->
      <div v-if="activeTab === 'guide'">
        <div class="flex items-center justify-between flex-wrap gap-12 mb-16">
          <div class="flex gap-8" role="group" :aria-label="t('heir.difficultyLabel')">
            <button class="btn btn-sm" :class="difficultyFilter === 'all' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="difficultyFilter === 'all'" @click="difficultyFilter = 'all'">{{ t('heir.all') }}</button>
            <button class="btn btn-sm" :class="difficultyFilter === '简单' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="difficultyFilter === '简单'" @click="difficultyFilter = '简单'">{{ t('heir.diffEasy') }}</button>
            <button class="btn btn-sm" :class="difficultyFilter === '中等' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="difficultyFilter === '中等'" @click="difficultyFilter = '中等'">{{ t('heir.diffMedium') }}</button>
            <button class="btn btn-sm" :class="difficultyFilter === '困难' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="difficultyFilter === '困难'" @click="difficultyFilter = '困难'">{{ t('heir.diffHard') }}</button>
          </div>
          <button class="collapse-all" @click="toggleAllGuide">{{ guideAllOpen ? t('heir.collapseAll') : t('heir.expandAll') }}</button>
        </div>

        <CollapseGroup
          v-for="g in groupedPlatforms"
          :key="g.key"
          :title="categoryLabel(g.key)"
          :count="g.platforms.length"
          v-model="openGroups[g.key]"
        >
          <div v-for="platform in g.platforms" :key="platform.id" class="mb-16 minw-0" style="border: 1px solid var(--gray-200); border-radius: 10px; overflow: hidden;">
            <div style="padding: 12px 16px; background: var(--gray-50); display: flex; justify-content: space-between; align-items: center; gap: 12px;">
              <div class="flex items-center gap-8 minw-0">
                <strong class="ellipsis">{{ platform.name }}</strong>
                <span class="text-muted" style="font-size: 12px;">{{ categoryLabel(platform.category) }}</span>
              </div>
              <span class="tag" :class="{
                'tag-success': platform.inheritable,
                'tag-danger': !platform.inheritable
              }">{{ platform.inheritable ? t('heir.inheritable') : t('heir.hardInherit') }}</span>
            </div>
            <div style="padding: 16px;">
              <div class="grid grid-2" style="gap: 12px; font-size: 13px;">
                <div>
                  <strong>{{ t('heir.requiredDocs') }}</strong>
                  <span class="text-muted">{{ platform.required_documents || t('heir.consultSupport') }}</span>
                </div>
                <div>
                  <strong>{{ t('heir.estimatedTime') }}</strong>
                  <span class="text-muted">{{ platform.estimated_time_days || t('heir.depends') }}</span>
                </div>
              </div>
              <div class="mt-12">
                <strong style="font-size: 13px;">{{ t('heir.process') }}：</strong>
                <p class="text-muted clamp-3" style="font-size: 13px; margin-top: 4px; line-height: 1.5;">
                  {{ platform.process_description || t('heir.noGuide') }}
                </p>
              </div>
            </div>
          </div>
        </CollapseGroup>
      </div>

      <!-- Tab 2: 变现/过户路径卡 (新增) -->
      <div v-if="activeTab === 'monetization'">
        <div class="flex items-center justify-between flex-wrap gap-12 mb-8">
          <div class="flex gap-8 flex-wrap" role="group" :aria-label="t('heir.categoryFilterLabel')">
            <button class="btn btn-sm" :class="categoryFilter === 'all' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="categoryFilter === 'all'" @click="categoryFilter = 'all'">{{ t('heir.all') }}</button>
            <button class="btn btn-sm" v-for="cat in categories" :key="cat" :class="categoryFilter === cat ? 'btn-primary' : 'btn-secondary'" :aria-pressed="categoryFilter === cat" @click="categoryFilter = cat">{{ categoryLabel(cat) }}</button>
          </div>
          <button class="collapse-all" @click="toggleAllExt">{{ extAllOpen ? t('heir.collapseAll') : t('heir.expandAll') }}</button>
        </div>

        <CollapseGroup
          v-for="g in groupedExtended"
          :key="g.key"
          :title="categoryLabel(g.key)"
          :count="g.platforms.length"
          v-model="openExtGroups[g.key]"
        >
          <div v-for="platform in g.platforms" :key="platform.id" class="mb-16 minw-0" style="border: 1px solid var(--gray-200); border-radius: 10px; overflow: hidden;">
          <div style="padding: 12px 16px; background: linear-gradient(135deg, var(--primary-light) 0%, var(--secondary-light) 100%); display: flex; justify-content: space-between; align-items: center;">
            <div class="flex items-center gap-8">
              <strong>{{ platform.name }}</strong>
              <span class="text-muted" style="font-size: 12px;">{{ categoryLabel(platform.category) }}</span>
            </div>
            <div class="flex gap-8">
              <span class="tag" :class="{
                'tag-success': platform.inheritable,
                'tag-danger': !platform.inheritable
              }">{{ platform.inheritable ? t('heir.inheritable') : t('heir.hardInherit') }}</span>
              <span v-if="platform.monetization_path" class="tag tag-warning">{{ t('heir.hasPath') }}</span>
            </div>
          </div>
          <div style="padding: 16px;">
            <!-- 变现/过户路径卡 -->
            <div v-if="platform.monetization_path" class="mb-16 p-12" style="background: var(--gray-50); border-radius: 8px; border-left: 3px solid var(--success);">
              <div style="font-size: 13px; color: var(--gray-700); line-height: 1.6;">
                {{ platform.monetization_path }}
              </div>
              <div style="font-size: 11px; color: var(--gray-500); margin-top: 6px; line-height: 1.5;">
                {{ t('heir.monetizationNote') }}
              </div>
            </div>

            <!-- 继承申请表链接：经 open_external 走系统浏览器，与资产详情同一白名单校验。
                 WebView 内直接 target=_blank 会拿到一个空白新窗口，且绕过 http/https 校验。 -->
            <div v-if="platform.inheritance_form_url" class="mb-12 p-12" style="background: var(--primary-light); border-radius: 8px;">
              <strong style="font-size: 13px;">{{ t('heir.officialForm') }}：</strong>
              <button class="btn btn-sm btn-secondary ml-8" @click="openForm(platform.inheritance_form_url)">{{ t('heir.openForm') }}</button>
              <div class="text-muted" style="font-size: 11px; margin-top: 6px; word-break: break-all;">{{ platform.inheritance_form_url }}</div>
            </div>

            <!-- 溯源状态：让家人知道这条政策是「有出处」还是「未核实」，
                 而不是把所有条目都当成同等权威。 -->
            <p class="mb-12" style="font-size: 12px;" :style="{ color: isUnverified(platform.data_source) ? 'var(--warning-strong)' : 'var(--gray-500)' }">
              <span v-if="isUnverified(platform.data_source)">{{ t('heir.policyUnverified') }}</span>
              <span v-else>{{ t('heir.policySource') }}{{ platform.data_source }}<span v-if="platform.last_reviewed"> · {{ t('heir.policyReviewed') }}{{ platform.last_reviewed }}</span></span>
            </p>

            <!-- 风险提示 -->
            <div v-if="platform.risk_warnings" class="mb-12 p-12 heir-risk">
              <strong style="font-size: 13px;">{{ t('heir.riskTitle') }}：</strong>
              <p style="font-size: 13px; margin-top: 4px;">{{ platform.risk_warnings }}</p>
            </div>

            <!-- 关联法律模板 -->
            <div v-if="platform.legal_template_ids" class="mb-12 p-12" style="background: var(--gray-50); border-radius: 8px;">
              <strong style="font-size: 13px;">{{ t('heir.legalTemplates') }}：</strong>
              <span class="text-muted" style="font-size: 12px; margin-left: 8px;">{{ platform.legal_template_ids }}</span>
            </div>

            <!-- 标签 -->
            <div v-if="platform.tags" class="mb-12" style="display: flex; flex-wrap: wrap; gap: 6px;">
              <span v-for="tag in platform.tags.split(',')" :key="tag" class="tag tag-secondary" style="font-size: 11px;">{{ tag.trim() }}</span>
            </div>

            <!-- 基础信息 -->
            <div class="grid grid-2" style="gap: 12px; font-size: 13px;">
              <div>
                <strong>{{ t('heir.requiredDocs') }}</strong>
                <span class="text-muted">{{ platform.required_documents || t('heir.consultSupport') }}</span>
              </div>
              <div>
                <strong>{{ t('heir.estimatedTime') }}</strong>
                <span class="text-muted">{{ platform.estimated_time_days || t('heir.depends') }}</span>
              </div>
            </div>
            <div class="mt-12">
              <strong style="font-size: 13px;">{{ t('heir.process') }}：</strong>
              <p class="text-muted" style="font-size: 13px; margin-top: 4px; line-height: 1.5;">
                {{ platform.process_description || t('heir.noGuide') }}
              </p>
            </div>
          </div>
        </div>
        </CollapseGroup>

        <div v-if="filteredExtendedPlatforms.length === 0" class="text-center text-muted py-32">
          {{ t('heir.emptyExt') }}
        </div>
      </div>
    </div>

    <!-- 常见困难解决方案 -->
    <div class="card mb-32">
      <div class="card-title mb-16">{{ t('heir.faqTitle') }}</div>
      <div style="display: flex; flex-direction: column; gap: 16px;">
        <div style="padding: 16px; border: 1px solid var(--gray-200); border-radius: 8px;">
          <strong>{{ t('heir.faq1Title') }}</strong>
          <p class="text-muted" style="font-size: 13px; margin-top: 4px; line-height: 1.5;">
            {{ t('heir.faq1a') }}<br>
            <strong>{{ t('heir.solution') }}：</strong>{{ t('heir.faq1b') }}
          </p>
        </div>
        <div style="padding: 16px; border: 1px solid var(--gray-200); border-radius: 8px;">
          <strong>{{ t('heir.faq2Title') }}</strong>
          <p class="text-muted" style="font-size: 13px; margin-top: 4px; line-height: 1.5;">
            <strong>{{ t('heir.solution') }}：</strong>{{ t('heir.faq2a') }}
          </p>
        </div>
        <div style="padding: 16px; border: 1px solid var(--gray-200); border-radius: 8px;">
          <strong>{{ t('heir.faq3Title') }}</strong>
          <p class="text-muted" style="font-size: 13px; margin-top: 4px; line-height: 1.5;">
            <strong>{{ t('heir.solution') }}：</strong>{{ t('heir.faq3a') }}
          </p>
        </div>
      </div>
    </div>

    <!-- 免责声明 -->
    <div class="card" style="background: var(--gray-50);">
      <div class="card-title mb-16">{{ t('heir.legalTitle') }}</div>
      <ul style="font-size: 13px; color: var(--gray-600); line-height: 2; padding-left: 20px;">
        <li>{{ t('heir.legal1') }}</li>
        <li>{{ t('heir.legal2') }}</li>
        <li>{{ t('heir.legal3') }}</li>
        <li>{{ t('heir.legal4') }}</li>
      </ul>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, onMounted, watch } from 'vue';
import WarningOutline from '@vicons/ionicons5/es/WarningOutline';
import { useI18n } from '../i18n';
import { usePlatformsStore } from '../stores/platforms';
import { toast } from '../composables/feedback';
import { invoke } from '../api/invoke';
import CollapseGroup from '../components/CollapseGroup.vue';
import type { ApiResponse, PlatformExtended } from '../types';

const { t } = useI18n();
const platforms = usePlatformsStore();

/** 平台分类的中文标识 → 当前语言的展示名。
 *  分类值是平台库里的中文串（数据契约的一部分，不能改），
 *  但界面必须按语言渲染——否则英文界面会整片出现中文分组标题。
 *  词典缺失时回退原值，保证新增分类不会白屏。 */
function categoryLabel(category: string): string {
  const key = `discover.cat.${category}.name`;
  const label = t(key);
  return label === key ? category : label;
}

/** 与后端 `UNVERIFIED_SOURCE` 常量一致：把「未核实」当状态展示，而不是当出处。 */
function isUnverified(source: string | undefined | null): boolean {
  return !source || source.includes('未核实');
}
const activeTab = ref<'guide' | 'monetization'>('guide');
const difficultyFilter = ref('all');
const categoryFilter = ref('all');

/** 平台申请表外链：与资产详情一致，统一经 Rust 侧 http/https 白名单后再交给系统浏览器。 */
async function openForm(url: string) {
  try {
    const res = await invoke<ApiResponse<null>>('open_external', { url });
    if (!res.success) toast(res.message, 'error');
  } catch (e) {
    toast(t('heir.openFormFailed', { e: String(e) }), 'error');
  }
}

const filteredPlatforms = computed(() => {
  if (difficultyFilter.value === 'all') return platforms.platforms;
  return platforms.platforms.filter(p => p.estimated_difficulty === difficultyFilter.value);
});

const categories = computed(() => {
  const cats = new Set<string>();
  platforms.platformsExtended.forEach(p => cats.add(p.category));
  return Array.from(cats).sort();
});

const filteredExtendedPlatforms = computed((): PlatformExtended[] => {
  let list = platforms.platformsExtended;
  if (categoryFilter.value !== 'all') {
    list = list.filter(p => p.category === categoryFilter.value);
  }
  return list.filter(p => p.monetization_path || p.inheritance_form_url || p.risk_warnings || p.legal_template_ids || p.tags);
});

// ── 按类别分组 + 折叠 ──
const openGroups = reactive<Record<string, boolean>>({});
const openExtGroups = reactive<Record<string, boolean>>({});

function groupByCategory<T extends { id: number; category: string }>(list: T[]) {
  const groups: { key: string; platforms: T[] }[] = [];
  for (const p of list) {
    let g = groups.find(x => x.key === p.category);
    if (!g) {
      g = { key: p.category, platforms: [] };
      groups.push(g);
    }
    g.platforms.push(p);
  }
  return groups;
}

const groupedPlatforms = computed(() => groupByCategory(filteredPlatforms.value));
const groupedExtended = computed(() => groupByCategory(filteredExtendedPlatforms.value));

const guideAllOpen = computed(
  () => groupedPlatforms.value.length > 0 && groupedPlatforms.value.every(g => openGroups[g.key])
);
const extAllOpen = computed(
  () => groupedExtended.value.length > 0 && groupedExtended.value.every(g => openExtGroups[g.key])
);

function toggleAllGuide() {
  const open = !guideAllOpen.value;
  groupedPlatforms.value.forEach(g => {
    openGroups[g.key] = open;
  });
}
function toggleAllExt() {
  const open = !extAllOpen.value;
  groupedExtended.value.forEach(g => {
    openExtGroups[g.key] = open;
  });
}

watch(() => activeTab.value, async (newTab) => {
  if (newTab === 'monetization' && platforms.platformsExtended.length === 0) {
    await platforms.loadPlatformsExtended();
  }
});

onMounted(async () => {
  if (platforms.platforms.length === 0) {
    await platforms.loadPlatforms();
  }
});
</script>

<style scoped>
.tab-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  background: var(--gray-100);
  color: var(--gray-600);
  border: 1px solid var(--gray-200);
  cursor: pointer;
  transition: all 0.2s;
  text-decoration: none;
}
.tab-btn:hover { background: var(--gray-200); }
.tab-btn.active {
  background: var(--solid-primary);
  color: var(--on-solid);
  border-color: var(--solid-primary);
}

/* 重要前提：浅色/深色都保持"琥珀提示"语义，但底色与文字都用 token */
.heir-prereq {
  background: var(--warning-light);
  border-color: var(--warning);
}
.heir-prereq strong { color: var(--warning-strong); }

.heir-risk {
  background: var(--warning-light);
  border-radius: 8px;
  border-left: 3px solid var(--warning);
  color: var(--warning-strong);
}
</style>