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

/**
 * 卡片定位。
 *
 * 关键约束：**永不盖住高亮目标**。早期实现只按视口高度粗略常量（260/240）判断，
 * 卡片实测高度约 190-210px 且随文案换行变化，结果在资产发现页把「继续」按钮
 * 压住、在资产页压住首条资产——用户第一次打开产品就点不到主操作。
 *
 * 现在的规则：
 * 1. 没有目标（欢迎页）→ 居中，但整体上移，避免压住首屏主按钮。
 * 2. 算完期望位置后，统一夹到视口内，并与目标矩形做一次避让。
 */
const CARD_W = 340;
/** 卡片实测高度上限（标题+3 行描述+按钮+步数）。用保守值做避让计算。 */
const CARD_H = 230;
const GAP = 14;
const EDGE = 16;

const cardStyle = computed(() => {
  if (!rect.value) {
    // 欢迎页：居中但抬高，给首屏主 CTA 让出可见区域。
    return {
      left: '50%',
      top: '42%',
      transform: 'translate(-50%, -50%)',
    };
  }
  const r = rect.value;
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  const clampX = (x: number) => Math.max(EDGE, Math.min(x, vw - CARD_W - EDGE));
  const clampY = (y: number) => Math.max(EDGE, Math.min(y, vh - CARD_H - EDGE));

  // 侧栏目标（导航项）必须放右侧：上下放置会盖住相邻导航项，
  // 用户看不全「自己在学的是哪一项」。
  if (r.left < 320) {
    return { left: `${r.right + GAP}px`, top: `${clampY(r.top)}px` };
  }

  // 主内容区：优先放在目标下方；下方空间不足就放上方；两侧都不够则放右侧/左侧。
  const spaceBelow = vh - r.bottom - GAP - EDGE;
  const spaceAbove = r.top - GAP - EDGE;

  if (spaceBelow >= CARD_H) {
    return { left: `${clampX(r.left)}px`, top: `${r.bottom + GAP}px` };
  }
  if (spaceAbove >= CARD_H) {
    return { left: `${clampX(r.left)}px`, top: `${r.top - CARD_H - GAP}px` };
  }
  // 上下都放不下（矮窗口或长目标）：贴右侧，仍保证不压目标。
  const spaceRight = vw - r.right - GAP - EDGE;
  if (spaceRight >= CARD_W) {
    return { left: `${r.right + GAP}px`, top: `${clampY(r.top)}px` };
  }
  const spaceLeft = r.left - GAP - EDGE;
  if (spaceLeft >= CARD_W) {
    return { left: `${r.left - CARD_W - GAP}px`, top: `${clampY(r.top)}px` };
  }
  // 实在无处可放（极小窗口）：夹到视口内，至少不越界。
  return { left: `${clampX(r.left)}px`, top: `${clampY(vh - CARD_H - EDGE)}px` };
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
  /* border-box：padding 与边框算进 340px。否则内容宽 340 + 40 padding = 380px，
     卡片的实际边界比定位计算宽 40px，圆点行会溢出到卡片外（截图上很明显）。 */
  box-sizing: border-box;
  width: 340px;
  background: var(--surface);
  border-radius: 14px;
  padding: 18px 20px 14px;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.28);
  transition: left 0.18s ease, top 0.18s ease;
}
.tour-progress {
  display: flex;
  flex-wrap: wrap;
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
  font-size: var(--text-md);
  font-weight: 700;
  color: var(--gray-900, #24323a);
}
.tour-desc {
  font-size: var(--text-sm);
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
  font-size: var(--text-2xs);
  color: var(--gray-400, #9aa8b0);
  margin-top: 6px;
}
</style>
