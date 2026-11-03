<template>
  <div class="drawer-mask" aria-hidden="true" @click="emit('close')"></div>
  <aside class="help-drawer" role="dialog" aria-modal="true" aria-labelledby="help-title" ref="drawerRef">
    <div class="help-header">
      <h2 id="help-title" style="font-size: 17px; font-weight: 700; margin: 0;">{{ t('help.title') }}</h2>
      <button class="btn btn-secondary btn-sm" ref="closeBtnRef" @click="emit('close')">{{ t('common.close') }}</button>
    </div>

    <div class="help-body">
      <div class="help-section">
        <div class="help-section-title">{{ t('help.quickStart') }}</div>
        <ol class="help-list">
          <li>{{ t('help.quick1') }}</li>
          <li>{{ t('help.quick2') }}</li>
          <li>{{ t('help.quick3') }}</li>
          <li>{{ t('help.quick4') }}</li>
          <li>{{ t('help.quick5') }}</li>
        </ol>
        <button class="btn btn-secondary btn-sm" @click="emit('replay-tour')">{{ t('help.replayTour') }}</button>
      </div>

      <div class="help-section">
        <div class="help-section-title">{{ t('help.securityTitle') }}</div>
        <ul class="help-list">
          <li>{{ t('help.sec1') }}</li>
          <li>{{ t('help.sec2') }}</li>
          <li>{{ t('help.sec3') }}</li>
          <li>{{ t('help.sec4') }}</li>
        </ul>
      </div>

      <div class="help-section">
        <div class="help-section-title">{{ t('help.dmsTitle') }}</div>
        <ul class="help-list">
          <li>{{ t('help.dms1') }}</li>
          <li>{{ t('help.dms2') }}</li>
          <li>{{ t('help.dms3') }}</li>
          <li>{{ t('help.dms4') }}</li>
          <li>{{ t('help.dms5') }}</li>
        </ul>
      </div>

      <div class="help-section">
        <div class="help-section-title">{{ t('help.faqTitle') }}</div>
        <ul class="help-list">
          <li><strong>{{ t('help.faq1q') }}</strong>{{ t('help.faq1a') }}</li>
          <li><strong>{{ t('help.faq2q') }}</strong>{{ t('help.faq2a') }}</li>
          <li><strong>{{ t('help.faq3q') }}</strong>{{ t('help.faq3a') }}</li>
          <li><strong>{{ t('help.faq4q') }}</strong>{{ t('help.faq4a') }}</li>
        </ul>
      </div>

      <div class="help-section about-section">
        <div class="help-section-title">{{ t('help.aboutTitle') }}</div>
        <p class="about-text">
          {{ t('help.aboutName') }} <span class="about-ver">v{{ appVersion }}</span><br>
          {{ t('help.aboutLine2') }}<br>
          {{ t('help.aboutLine3') }}
        </p>
        <p class="about-shortcut">{{ t('help.shortcut') }}</p>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from '../i18n';

const { t } = useI18n();
const emit = defineEmits<{ (e: 'close'): void; (e: 'replay-tour'): void }>();

const appVersion = __APP_VERSION__;
const drawerRef = ref<HTMLElement | null>(null);
const closeBtnRef = ref<HTMLElement | null>(null);
let previouslyFocused: HTMLElement | null = null;

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('close');
}

// Tab 困留在面板内：aria-modal 面板不允许键盘穿透到背景内容
function trapTab(e: KeyboardEvent) {
  if (e.key !== 'Tab') return;
  const panel = drawerRef.value;
  if (!panel) return;
  const focusables = Array.from(
    panel.querySelectorAll<HTMLElement>('button:not([disabled]), input, [href], textarea, select')
  );
  if (focusables.length === 0) return;
  const active = document.activeElement as HTMLElement | null;
  if (!active || !panel.contains(active)) {
    e.preventDefault();
    focusables[0].focus();
    return;
  }
  const idx = focusables.indexOf(active);
  let next = idx + (e.shiftKey ? -1 : 1);
  if (next < 0) next = focusables.length - 1;
  if (next >= focusables.length) next = 0;
  e.preventDefault();
  focusables[next].focus();
}

onMounted(() => {
  window.addEventListener('keydown', onKey);
  document.addEventListener('keydown', trapTab, true);
  // 打开时焦点移入面板；关闭时归还给触发按钮
  previouslyFocused = document.activeElement as HTMLElement | null;
  closeBtnRef.value?.focus();
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  document.removeEventListener('keydown', trapTab, true);
  previouslyFocused?.focus?.();
});
</script>

<style scoped>
.drawer-mask {
  position: fixed;
  inset: 0;
  background: rgba(32, 26, 21, 0.35);
  z-index: 1100;
}
.help-drawer {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  width: 430px;
  max-width: calc(100vw - 48px);
  background: var(--surface);
  z-index: 1110;
  box-shadow: -12px 0 40px rgba(0, 0, 0, 0.18);
  display: flex;
  flex-direction: column;
}
.help-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 18px 20px;
  border-bottom: 1px solid var(--gray-100, #f0ece6);
}
.help-body {
  padding: 16px 20px 28px;
  overflow-y: auto;
}
.help-section {
  margin-bottom: 22px;
}
.help-section-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--gray-900, #24323a);
  margin-bottom: 8px;
}
.help-list {
  margin: 0;
  padding-left: 18px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 13px;
  color: var(--gray-600, #5c6f79);
  line-height: 1.65;
}
.help-list strong {
  color: var(--gray-800, #37464f);
}
.about-section {
  padding-top: 14px;
  border-top: 1px solid var(--gray-100, #f0ece6);
}
.about-text {
  margin: 0;
  font-size: 12.5px;
  color: var(--gray-600, #5c6f79);
  line-height: 1.75;
}
.about-ver {
  display: inline-block;
  padding: 0 6px;
  border-radius: 4px;
  background: var(--gray-100, #f0ece6);
  font-size: 11px;
  color: var(--gray-700, #4a5560);
}
.about-shortcut {
  margin: 8px 0 0;
  font-size: 11.5px;
  color: var(--gray-400, #9aa8b0);
}
</style>
