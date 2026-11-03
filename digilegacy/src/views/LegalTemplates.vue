<template>
  <div class="legal fade-in">
    <h2 style="font-size: 22px; font-weight: 700; margin-bottom: 8px;">{{ t('legal.title') }}</h2>
    <p class="text-muted mb-32" style="font-size: 14px;">
      {{ t('legal.subtitle') }}
    </p>
    <div v-if="isEn" class="card mb-24" style="background: var(--warning-light); border-color: var(--warning);">
      <p class="legal-notice" style="font-size: 13px; color: var(--warning-strong);"><WarningOutline /> {{ t('legal.enNotice') }}</p>
    </div>

    <!-- 分类筛选 -->
    <div class="card mb-24">
      <div class="card-title mb-16">{{ t('legal.filterTitle') }}</div>
      <div class="flex gap-8 flex-wrap" role="group" :aria-label="t('legal.filterLabel')">
        <button class="btn btn-sm" :class="typeFilter === 'all' ? 'btn-primary' : 'btn-secondary'" :aria-pressed="typeFilter === 'all'" @click="typeFilter = 'all'">{{ t('legal.all') }}</button>
        <button class="btn btn-sm" v-for="typ in templateTypes" :key="typ.value" :class="typeFilter === typ.value ? 'btn-primary' : 'btn-secondary'" :aria-pressed="typeFilter === typ.value" @click="typeFilter = typ.value">{{ t(typ.labelKey) }}</button>
      </div>
    </div>

    <!-- 模板列表 -->
    <div class="card mb-24">
      <div class="card-title mb-16">{{ t('legal.listTitle') }}</div>
      <div v-for="tmpl in filteredTemplates" :key="tmpl.id" class="mb-16" style="border: 1px solid var(--gray-200); border-radius: 8px; overflow: hidden;">
        <div style="padding: 12px 16px; background: var(--gray-50); display: flex; justify-content: space-between; align-items: center;">
          <div class="flex items-center gap-8">
            <strong>{{ tmpl.name }}</strong>
            <span class="tag tag-secondary">{{ tmpl.type }}</span>
          </div>
          <div class="flex gap-8">
            <button class="btn btn-sm btn-secondary" @click="previewTemplate = tmpl">{{ t('legal.preview') }}</button>
            <button class="btn btn-sm btn-primary" @click="downloadTemplate(tmpl)">{{ t('legal.download') }}</button>
          </div>
        </div>
        <div style="padding: 16px;">
          <p class="text-muted" style="font-size: 13px; line-height: 1.5;">{{ tmpl.description }}</p>
          <div class="mt-12" style="display: flex; flex-wrap: wrap; gap: 6px;">
            <span v-for="tag in tmpl.tags" :key="tag" class="tag tag-secondary" style="font-size: 11px;">{{ tag }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 预览弹窗 -->
    <div v-if="previewTemplate" class="modal-overlay" @click.self="closePreview">
      <div class="modal-content" role="dialog" aria-modal="true" aria-labelledby="legal-preview-title" ref="previewRef" @keydown.tab="trapPreviewFocus">
        <div class="modal-header">
          <h3 id="legal-preview-title" style="font-size: 18px; font-weight: 600; margin: 0;">{{ previewTemplate.name }}</h3>
          <button class="btn btn-sm btn-secondary" ref="previewCloseRef" @click="closePreview">{{ t('common.close') }}</button>
        </div>
        <div class="modal-body">
          <pre style="white-space: pre-wrap; font-size: 13px; line-height: 1.6; color: var(--gray-700);">{{ previewTemplate.content }}</pre>
        </div>
      </div>
    </div>

    <!-- 使用说明 -->
    <div class="card" style="background: var(--gray-50);">
      <div class="card-title mb-16">{{ t('legal.usageTitle') }}</div>
      <ul style="font-size: 13px; color: var(--gray-600); line-height: 2; padding-left: 20px;">
        <li>{{ t('legal.usage1') }}</li>
        <li>{{ t('legal.usage2') }}</li>
        <li>{{ t('legal.usage3') }}</li>
        <li>{{ t('legal.usage4') }}</li>
      </ul>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onBeforeUnmount } from 'vue';
import WarningOutline from '@vicons/ionicons5/es/WarningOutline';
import { useI18n } from '../i18n';
import legalTemplates from '../data/legal-templates.json';

interface LegalTemplate {
  id: string;
  name: string;
  type: string;
  description: string;
  content: string;
  tags: string[];
}

const { t, isEn } = useI18n();
const typeFilter = ref('all');

const templateTypes = [
  { value: 'will', zhLabel: '遗嘱', labelKey: 'legal.typeWill' },
  { value: 'inheritance', zhLabel: '继承声明', labelKey: 'legal.typeInheritance' },
  { value: 'power_of_attorney', zhLabel: '授权委托书', labelKey: 'legal.typePowerOfAttorney' },
  { value: 'affidavit', zhLabel: '宣誓书/保证书', labelKey: 'legal.typeAffidavit' },
  { value: 'application', zhLabel: '申请书', labelKey: 'legal.typeApplication' },
  { value: 'agreement', zhLabel: '协议书', labelKey: 'legal.typeAgreement' },
];

// 模板内容抽到 JSON：审核场景要在 Rust 侧读同一份，
// 两处各存一份必然漂移（改了一边、审核的还是另一边）。
// 这里用 JSON 作为单一事实源，Rust 侧用 include_str! 内嵌同一个文件。
const templates = legalTemplates as LegalTemplate[];

const filteredTemplates = computed(() => {
  if (typeFilter.value === 'all') return templates;
  // typeFilter 存的是 templateTypes 的 value（英文 key，如 'will'），
  // 而模板的 type 字段是中文标签（如 '遗嘱'）。此处必须先映射成标签再比较，
  // 否则任何分类筛选都会返回空列表（历史缺陷）。
  const hit = templateTypes.find(t => t.value === typeFilter.value);
  if (!hit) return templates;
  // 模板 type 字段存的是中文规范标签，筛选必须按 zhLabel 比较（与界面语言无关）
  return templates.filter(t => t.type === hit.zhLabel);
});

const previewTemplate = ref<LegalTemplate | null>(null);
const previewRef = ref<HTMLElement | null>(null);
const previewCloseRef = ref<HTMLElement | null>(null);
let previouslyFocused: HTMLElement | null = null;

function closePreview() {
  previewTemplate.value = null;
}

// 预览弹窗：Esc 关闭、焦点移入/困留/归还
function onPreviewKey(e: KeyboardEvent) {
  if (e.key === 'Escape') closePreview();
}

function trapPreviewFocus(e: KeyboardEvent) {
  const dialog = previewRef.value;
  if (!dialog) return;
  const focusables = Array.from(dialog.querySelectorAll<HTMLElement>('button:not([disabled]), input, [href], textarea, select'));
  if (focusables.length === 0) return;
  const first = focusables[0];
  const last = focusables[focusables.length - 1];
  const active = document.activeElement;
  if (e.shiftKey && (active === first || !dialog.contains(active))) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && (active === last || !dialog.contains(active))) {
    e.preventDefault();
    first.focus();
  }
}

watch(previewTemplate, async (val) => {
  if (val) {
    previouslyFocused = document.activeElement as HTMLElement | null;
    window.addEventListener('keydown', onPreviewKey);
    await nextTick();
    previewCloseRef.value?.focus();
  } else {
    window.removeEventListener('keydown', onPreviewKey);
    previouslyFocused?.focus?.();
    previouslyFocused = null;
  }
});

onBeforeUnmount(() => window.removeEventListener('keydown', onPreviewKey));

function downloadTemplate(tmpl: LegalTemplate) {
  const blob = new Blob([tmpl.content], { type: 'text/plain;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${tmpl.name}.txt`;
  a.click();
  URL.revokeObjectURL(url);
}
</script>

<style scoped>
.legal-notice {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  line-height: 1.6;
}
.legal-notice svg {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  margin-top: 2px;
}

.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.modal-content {
  background: var(--surface);
  border-radius: 12px;
  max-width: 700px;
  width: 90%;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
}
.modal-header {
  padding: 16px 24px;
  border-bottom: 1px solid var(--gray-200);
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.modal-body {
  padding: 24px;
  overflow-y: auto;
  flex: 1;
}
</style>