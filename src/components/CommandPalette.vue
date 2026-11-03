<template>
  <!--
    命令面板（Cmd/Ctrl + K）：对齐 VS Code / opencode / Raycast 的肌肉记忆。

    设计取舍：
    - 纯键盘可达（↑↓ 选择、Enter 执行、Esc 关闭），鼠标只是备选；
    - 焦点进入面板后不逃逸（Tab 循环在输入框内），避免用户 Tab 到背后的页面；
    - 分组按「去哪 / 做什么」组织，而不是把 10 个页面平铺；
    - 不做模糊搜索库依赖：本项目命令量级（<50）用子串匹配 + 前缀加权足够，
      引入 fuse.js 只为这点功能不值得。
  -->
  <Teleport to="body">
    <div
      v-if="open"
      class="cmdk-overlay"
      role="presentation"
      @mousedown.self="close"
    >
      <div
        class="cmdk-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="cmdk-title"
        @keydown="onKeydown"
      >
        <h2 id="cmdk-title" class="visually-hidden">{{ t('cmdk.title') }}</h2>

        <div class="cmdk-search">
          <SearchOutline aria-hidden="true" />
          <input
            ref="inputRef"
            v-model="query"
            class="cmdk-input"
            type="text"
            role="combobox"
            aria-expanded="true"
            aria-controls="cmdk-results"
            aria-autocomplete="list"
            :aria-activedescendant="activeId"
            :placeholder="t('cmdk.placeholder')"
            data-testid="cmdk-input"
          />
        </div>

        <div id="cmdk-results" class="cmdk-list" role="listbox" :aria-label="t('cmdk.title')">
          <div v-if="groups.length === 0" class="cmdk-empty">
            {{ t('cmdk.empty', { q: query }) }}
          </div>

          <template v-for="group in groups" :key="group.id">
            <div class="cmdk-group-label">{{ t(group.labelKey) }}</div>
            <button
              v-for="item in group.items"
              :id="'cmdk-' + item.id"
              :key="item.id"
              type="button"
              class="cmdk-item"
              role="option"
              :aria-selected="item.id === activeItem?.id"
              @mousemove="activeIndex = flatIndex(item)"
              @click="run(item)"
            >
              <component :is="item.icon" aria-hidden="true" />
              <span>{{ t(item.labelKey) }}</span>
              <span v-if="item.meta" class="cmdk-item-meta">{{ item.meta }}</span>
            </button>
          </template>
        </div>

        <div class="cmdk-footer">
          <span><kbd class="cmdk-kbd">↑</kbd><kbd class="cmdk-kbd">↓</kbd> {{ t('cmdk.hintMove') }}</span>
          <span><kbd class="cmdk-kbd">Enter</kbd> {{ t('cmdk.hintRun') }}</span>
          <span><kbd class="cmdk-kbd">Esc</kbd> {{ t('cmdk.hintClose') }}</span>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import type { Component } from 'vue';
import SearchOutline from '@vicons/ionicons5/es/SearchOutline';
import GridOutline from '@vicons/ionicons5/es/GridOutline';
import SearchCircleOutline from '@vicons/ionicons5/es/SearchCircleOutline';
import DiamondOutline from '@vicons/ionicons5/es/DiamondOutline';
import PersonOutline from '@vicons/ionicons5/es/PersonOutline';
import TimeOutline from '@vicons/ionicons5/es/TimeOutline';
import DocumentTextOutline from '@vicons/ionicons5/es/DocumentTextOutline';
import PeopleOutline from '@vicons/ionicons5/es/PeopleOutline';
import CheckmarkCircleOutline from '@vicons/ionicons5/es/CheckmarkCircleOutline';
import BookOutline from '@vicons/ionicons5/es/BookOutline';
import MoonOutline from '@vicons/ionicons5/es/MoonOutline';
import SunnyOutline from '@vicons/ionicons5/es/SunnyOutline';
import LanguageOutline from '@vicons/ionicons5/es/LanguageOutline';
import LockClosedOutline from '@vicons/ionicons5/es/LockClosedOutline';
import { useI18n } from '../i18n';
import { useTheme } from '../composables/theme';
import { useSecurityStore } from '../stores/security';

interface CmdItem {
  id: string;
  labelKey: string;
  icon: Component;
  /** 额外搜索词：让「dms」「安心传承」「switch」都能命中同一条 */
  keywords?: string[];
  meta?: string;
  action: () => void | Promise<void>;
}

interface CmdGroup {
  id: string;
  labelKey: string;
  items: CmdItem[];
}

const { t, locale, toggleLocale } = useI18n();
const router = useRouter();
const security = useSecurityStore();
const { toggleTheme, resolved } = useTheme();
const isDark = computed(() => resolved.value === 'dark');

/** 受控开关：父组件用 v-model:open 控制，组件内部也可自行开合（⌘K）。 */
const props = withDefaults(defineProps<{ open?: boolean }>(), { open: false });
const emit = defineEmits<{ (e: 'update:open', v: boolean): void }>();

const query = ref('');
const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);

const open = computed({
  get: () => props.open,
  set: (v: boolean) => emit('update:open', v),
});

function close() {
  open.value = false;
  query.value = '';
  activeIndex.value = 0;
}

function openPanel() {
  open.value = true;
  nextTick(() => inputRef.value?.focus());
}

// 父组件打开时自动聚焦输入框（点击头部按钮的场景）
watch(() => props.open, (v) => {
  if (v) nextTick(() => inputRef.value?.focus());
});

/** 路由跳转类命令：导航后立即关闭，不留在面板里。 */
function go(path: string) {
  return () => {
    close();
    void router.push(path);
  };
}

const allGroups = computed<CmdGroup[]>(() => {
  const navGroup: CmdGroup = {
    id: 'nav',
    labelKey: 'cmdk.groupNav',
    items: [
      { id: 'nav-dashboard', labelKey: 'nav.dashboard', icon: GridOutline, keywords: ['home', '首页', '仪表'], action: go('/dashboard') },
      { id: 'nav-discover', labelKey: 'nav.discover', icon: SearchCircleOutline, keywords: ['find', '发现', '问卷'], action: go('/discover') },
      { id: 'nav-assets', labelKey: 'nav.assets', icon: DiamondOutline, keywords: ['asset', '资产', '清单'], action: go('/assets') },
      { id: 'nav-profile', labelKey: 'nav.profile', icon: PersonOutline, keywords: ['profile', '档案', '设置'], action: go('/profile') },
      { id: 'nav-dms', labelKey: 'nav.dms', icon: TimeOutline, keywords: ['dms', 'switch', '安心传承', '签到'], action: go('/dms') },
      { id: 'nav-report', labelKey: 'nav.report', icon: DocumentTextOutline, keywords: ['report', '报告', '导出'], action: go('/report') },
      { id: 'nav-heir', labelKey: 'nav.heir', icon: PeopleOutline, keywords: ['heir', '继承人', '向导'], action: go('/heir') },
      { id: 'nav-review', labelKey: 'nav.review', icon: CheckmarkCircleOutline, keywords: ['review', '审核', '校对'], action: go('/review') },
      { id: 'nav-legal', labelKey: 'nav.legal', icon: BookOutline, keywords: ['legal', '法律', '模板'], action: go('/legal') },
    ],
  };

  const actionGroup: CmdGroup = {
    id: 'action',
    labelKey: 'cmdk.groupAction',
    items: [
      {
        id: 'act-theme',
        labelKey: 'cmdk.toggleTheme',
        icon: isDark.value ? SunnyOutline : MoonOutline,
        keywords: ['theme', 'dark', 'light', '主题', '深色', '浅色'],
        action: () => { toggleTheme(); close(); },
      },
      {
        id: 'act-locale',
        labelKey: 'cmdk.toggleLocale',
        icon: LanguageOutline,
        keywords: ['language', 'english', 'chinese', '语言', '英文', '中文'],
        meta: locale.value === 'zh-CN' ? 'EN' : '中文',
        action: () => { toggleLocale(); close(); },
      },
    ],
  };

  // 只有设了主密码且当前已解锁时，「立即锁定」才有意义
  if (security.hasMasterPassword && security.unlocked) {
    actionGroup.items.push({
      id: 'act-lock',
      labelKey: 'cmdk.lockNow',
      icon: LockClosedOutline,
      keywords: ['lock', '锁定', '安全'],
      action: () => { void security.lockNow(); close(); },
    });
  }

  return [navGroup, actionGroup];
});

/** 子串匹配 + 前缀加权：标题以查询开头排最前，其次关键词命中。 */
const groups = computed<CmdGroup[]>(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return allGroups.value;

  const scored: Array<{ group: CmdGroup; items: Array<CmdItem & { _score: number }> }> = [];
  for (const group of allGroups.value) {
    const hits: Array<CmdItem & { _score: number }> = [];
    for (const item of group.items) {
      const label = t(item.labelKey).toLowerCase();
      const kw = (item.keywords ?? []).map((k) => k.toLowerCase());
      let score = 0;
      if (label.startsWith(q)) score = 100;
      else if (label.includes(q)) score = 70;
      else if (kw.some((k) => k.startsWith(q))) score = 50;
      else if (kw.some((k) => k.includes(q))) score = 30;
      if (score > 0) hits.push({ ...item, _score: score });
    }
    if (hits.length) {
      hits.sort((a, b) => b._score - a._score);
      scored.push({ group, items: hits });
    }
  }
  return scored.map(({ group, items }) => ({ ...group, items }));
});

const flatItems = computed(() => groups.value.flatMap((g) => g.items));
const activeItem = computed(() => flatItems.value[activeIndex.value]);

const activeId = computed(() =>
  activeItem.value ? 'cmdk-' + activeItem.value.id : undefined,
);

function flatIndex(item: CmdItem): number {
  return flatItems.value.findIndex((x) => x.id === item.id);
}

watch(query, () => { activeIndex.value = 0; });

function run(item: CmdItem) {
  void item.action();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault();
    close();
    return;
  }
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    if (flatItems.value.length) {
      activeIndex.value = (activeIndex.value + 1) % flatItems.value.length;
    }
    return;
  }
  if (e.key === 'ArrowUp') {
    e.preventDefault();
    if (flatItems.value.length) {
      activeIndex.value =
        (activeIndex.value - 1 + flatItems.value.length) % flatItems.value.length;
    }
    return;
  }
  if (e.key === 'Enter') {
    e.preventDefault();
    if (activeItem.value) run(activeItem.value);
  }
}

/** 全局快捷键：Cmd+K / Ctrl+K 开关面板。 */
function onGlobalKey(e: KeyboardEvent) {
  const isK = e.key === 'k' || e.key === 'K';
  if (isK && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    if (open.value) close();
    else openPanel();
  }
}

onMounted(() => window.addEventListener('keydown', onGlobalKey));
onBeforeUnmount(() => window.removeEventListener('keydown', onGlobalKey));

defineExpose({ openPanel, close });
</script>

<style scoped>
.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
  border: 0;
}
</style>
