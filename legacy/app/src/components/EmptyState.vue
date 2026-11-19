<template>
  <!--
    空态插画：用品牌色系绘制的轻量 SVG，替代 emoji。
    纯内联、无外部依赖；尺寸随容器缩放；装饰性元素对读屏隐藏。
  -->
  <svg
    class="empty-illustration"
    :width="size"
    :height="size * 0.75"
    viewBox="0 0 160 120"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden="true"
    focusable="false"
  >
    <defs>
      <linearGradient :id="`${uid}-paper`" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="#ffffff" />
        <stop offset="1" stop-color="#f4ece4" />
      </linearGradient>
      <linearGradient :id="`${uid}-accent`" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#c46a5c" />
        <stop offset="1" stop-color="#9a473b" />
      </linearGradient>
    </defs>

    <!-- 底部柔和阴影 -->
    <ellipse cx="80" cy="103" rx="46" ry="6" fill="#232019" opacity="0.06" />

    <template v-if="kind === 'assets'">
      <!-- 空清单：卡片 + 虚线占位行 -->
      <rect x="38" y="26" width="84" height="70" rx="8" fill="url(#paper)" stroke="#e4ddd3" stroke-width="1.5" />
      <rect x="48" y="38" width="30" height="6" rx="3" fill="#cfc6ba" />
      <rect x="48" y="52" width="64" height="5" rx="2.5" fill="#e4ddd3" />
      <rect x="48" y="63" width="52" height="5" rx="2.5" fill="#e4ddd3" />
      <rect x="48" y="74" width="58" height="5" rx="2.5" fill="#efe9e1" />
      <!-- 品牌环（浮在卡片右下，标记归属） -->
      <circle cx="112" cy="84" r="15" fill="url(#accent)" />
      <circle cx="112" cy="84" r="7.5" stroke="#fff" stroke-width="1.6" opacity="0.92" />
      <circle cx="112" cy="84" r="2" fill="#fff" />
    </template>

    <template v-else-if="kind === 'search'">
      <!-- 搜索：放大镜扫过清单 -->
      <rect x="30" y="24" width="66" height="76" rx="8" fill="url(#paper)" stroke="#e4ddd3" stroke-width="1.5" />
      <rect x="40" y="36" width="26" height="5" rx="2.5" fill="#cfc6ba" />
      <rect x="40" y="48" width="44" height="4" rx="2" fill="#e4ddd3" />
      <rect x="40" y="57" width="38" height="4" rx="2" fill="#e4ddd3" />
      <rect x="40" y="66" width="42" height="4" rx="2" fill="#efe9e1" />
      <circle cx="98" cy="66" r="21" fill="#f4e3df" opacity="0.75" />
      <circle cx="98" cy="66" r="15" stroke="url(#accent)" stroke-width="3" />
      <circle cx="98" cy="66" r="5" stroke="url(#accent)" stroke-width="2.4" opacity="0.6" />
      <line x1="109" y1="77" x2="121" y2="89" stroke="#9a473b" stroke-width="4" stroke-linecap="round" />
    </template>

    <template v-else-if="kind === 'report'">
      <!-- 报告：文档 + 折角 + 印章环 -->
      <path d="M44 20h48l20 20v60H44z" fill="url(#paper)" stroke="#e4ddd3" stroke-width="1.5" />
      <path d="M92 20l20 20H92z" fill="#f4e3df" stroke="#e4ddd3" stroke-width="1.5" />
      <rect x="54" y="50" width="42" height="5" rx="2.5" fill="#cfc6ba" />
      <rect x="54" y="62" width="48" height="4" rx="2" fill="#e4ddd3" />
      <rect x="54" y="71" width="40" height="4" rx="2" fill="#e4ddd3" />
      <rect x="54" y="80" width="46" height="4" rx="2" fill="#efe9e1" />
      <circle cx="112" cy="90" r="14" stroke="url(#accent)" stroke-width="2.4" opacity="0.85" />
      <circle cx="112" cy="90" r="7" stroke="url(#accent)" stroke-width="1.4" opacity="0.5" />
    </template>

    <template v-else>
      <!-- 兜底：品牌环 -->
      <circle cx="80" cy="58" r="30" fill="#f4e3df" />
      <circle cx="80" cy="58" r="18" stroke="url(#accent)" stroke-width="3" />
      <circle cx="80" cy="58" r="6" fill="url(#accent)" />
    </template>
  </svg>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(defineProps<{
  /** 场景：assets 资产清单 / search 发现 / report 报告 / blank 兜底 */
  kind?: 'assets' | 'search' | 'report' | 'blank';
  size?: number;
}>(), {
  kind: 'blank',
  size: 160,
});

// 多个实例在同页时，渐变 id 必须唯一，否则会互相覆盖
const uid = computed(() => `empty-${props.kind}-${Math.random().toString(36).slice(2, 8)}`);
</script>

<style scoped>
.empty-illustration {
  display: block;
  margin: 0 auto;
  max-width: 100%;
  height: auto;
}
</style>
