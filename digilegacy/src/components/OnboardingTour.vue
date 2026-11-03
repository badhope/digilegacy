<template>
  <div class="tour-root">
    <!-- 遮罩可点击关闭：避免用户找不到出口时被"困"在引导里 -->
    <div class="tour-blocker" @click="finish" :title="t('tour.close')" :aria-label="t('tour.close')"></div>

    <div v-if="rect" class="tour-spotlight" :style="spotlightStyle"></div>

    <div class="tour-card" ref="cardRef" tabindex="-1" :style="cardStyle" role="dialog" aria-modal="true" aria-labelledby="tour-title" @click.stop>
      <div class="tour-progress" aria-hidden="true">
        <span v-for="(_, i) in steps" :key="i" class="tour-dot" :class="{ active: i === index }"></span>
      </div>
      <!-- 切步时读屏自动播报（4.1.3 状态消息） -->
      <div aria-live="polite">
        <div class="tour-title" id="tour-title">{{ t(current.titleKey) }}</div>
        <div class="tour-desc">{{ t(current.descKey) }}</div>
      </div>
      <div class="tour-actions">
        <button class="btn btn-secondary btn-sm" @click="skip">{{ t('tour.skip') }}</button>
        <div style="display: flex; gap: 8px;">
          <button v-if="index > 0" class="btn btn-secondary btn-sm" @click="prev">{{ t('tour.prev') }}</button>
          <button class="btn btn-primary btn-sm" @click="next">{{ index === steps.length - 1 ? t('tour.done') : t('tour.next') }}</button>
        </div>
      </div>
      <div class="tour-step-text">{{ index + 1 }} / {{ steps.length }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from '../i18n';

interface TourStep {
  target: string | null;
  titleKey: string;
  descKey: string;
}

const { t } = useI18n();
const emit = defineEmits<{ (e: 'close'): void }>();

const cardRef = ref<HTMLElement | null>(null);
let previouslyFocused: HTMLElement | null = null;

const steps: TourStep[] = [
  { target: null, titleKey: 'tour.welcome', descKey: 'tour.welcomeDesc' },
  { target: '[data-tour="nav-discover"]', titleKey: 'tour.discover', descKey: 'tour.discoverDesc' },
  { target: '[data-tour="nav-assets"]', titleKey: 'tour.assets', descKey: 'tour.assetsDesc' },
  { target: '[data-tour="nav-profile"]', titleKey: 'tour.profile', descKey: 'tour.profileDesc' },
  { target: '[data-tour="nav-dms"]', titleKey: 'tour.dms', descKey: 'tour.dmsDesc' },
  { target: '[data-tour="nav-report"]', titleKey: 'tour.report', descKey: 'tour.reportDesc' },
  { target: '[data-tour="nav-heir"]', titleKey: 'tour.heir', descKey: 'tour.heirDesc' },
  { target: '[data-tour="checkin"]', titleKey: 'tour.checkin', descKey: 'tour.checkinDesc' },
  { target: '[data-tour="help"]', titleKey: 'tour.help', descKey: 'tour.helpDesc' },
];

const index = ref(0);
const rect = ref<DOMRect | null>(null);

const current = computed(() => steps[index.value]);

const spotlightStyle = computed(() => {
  if (!rect.value) return {};
  const pad = 6;
  return {
    left: `${rect.value.left - pad}px`,
    top: `${rect.value.top - pad}px`,
    width: `${rect.value.width + pad * 2}px`,
    height: `${rect.value.height + pad * 2}px`,
  };
});

const cardStyle = computed(() => {
  const CARD_W = 340;
  if (!rect.value) {
    return { left: '50%', top: '50%', transform: 'translate(-50%, -50%)' };
  }
  const r = rect.value;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  if (r.left < 320) {
    return { left: `${r.right + 16}px`, top: `${Math.min(r.top, vh - 260)}px` };
  }
  if (r.bottom + 240 < vh) {
    return { left: `${Math.max(16, Math.min(r.left, vw - CARD_W - 16))}px`, top: `${r.bottom + 14}px` };
  }
  return { left: `${Math.max(16, Math.min(r.left, vw - CARD_W - 16))}px`, top: `${Math.max(16, r.top - 230)}px` };
});

function measure() {
  const step = steps[index.value];
  if (!step.target) {
    rect.value = null;
    return;
  }
  const el = document.querySelector(step.target);
  rect.value = el ? el.getBoundingClientRect() : null;
}

function next() {
  if (index.value >= steps.length - 1) {
    finish();
    return;
  }
  index.value += 1;
  measure();
}

function prev() {
  if (index.value > 0) {
    index.value -= 1;
    measure();
  }
}

function skip() {
  finish();
}

function finish() {
  localStorage.setItem('digilegacy_onboarding_done', '1');
  emit('close');
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') finish();
  if (e.key === 'ArrowRight' || e.key === 'Enter') next();
  if (e.key === 'ArrowLeft') prev();
}

onMounted(() => {
  measure();
  window.addEventListener('resize', measure);
  window.addEventListener('keydown', onKey);
  // 焦点移入引导卡片（tabindex=-1 容器，避免 Enter 触发按钮与全局快捷键双重响应）；
  // 关闭时归还焦点给打开引导的按钮
  previouslyFocused = document.activeElement as HTMLElement | null;
  cardRef.value?.focus();
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', measure);
  window.removeEventListener('keydown', onKey);
  previouslyFocused?.focus?.();
});
</script>

<style scoped>
.tour-root {
  position: fixed;
  inset: 0;
  z-index: 1200;
}
.tour-blocker {
  position: absolute;
  inset: 0;
}
.tour-spotlight {
  position: fixed;
  border-radius: 12px;
  box-shadow: 0 0 0 9999px rgba(32, 26, 21, 0.6);
  border: 2px solid var(--primary, #4a7c8c);
  pointer-events: none;
  transition: all 0.18s ease;
}
.tour-card {
  position: fixed;
  width: 340px;
  background: var(--surface);
  border-radius: 14px;
  padding: 18px 20px 14px;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.28);
  transition: left 0.18s ease, top 0.18s ease;
}
.tour-progress {
  display: flex;
  gap: 5px;
  margin-bottom: 10px;
}
.tour-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--gray-200, #e8e4de);
}
.tour-dot.active {
  background: var(--primary, #4a7c8c);
}
.tour-title {
  font-size: 16px;
  font-weight: 700;
  color: var(--gray-900, #24323a);
}
.tour-desc {
  font-size: 13px;
  color: var(--gray-600, #5c6f79);
  line-height: 1.65;
  margin-top: 6px;
}
.tour-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 14px;
}
.tour-step-text {
  text-align: right;
  font-size: 11px;
  color: var(--gray-400, #9aa8b0);
  margin-top: 6px;
}
</style>
