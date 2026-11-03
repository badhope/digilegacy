<template>
  <!--
    资产构成图：用 ECharts 的饼图 + 环形中心文案。
    按需引入（echarts/core + PieChart + CanvasRenderer），不引入整包：
    完整 echarts 约 1MB，按需后仅 ~180KB（gzip 后约 60KB）。
  -->
  <div class="asset-chart">
    <div ref="host" class="asset-chart-canvas" role="img" :aria-label="ariaLabel"></div>
    <ul class="asset-chart-legend" v-if="total > 0">
      <li v-for="item in legend" :key="item.name">
        <span class="dot" :style="{ background: item.color }" aria-hidden="true"></span>
        <span class="name">{{ item.name }}</span>
        <span class="value">{{ item.value }} {{ t('dashboard.itemSuffix') }} · {{ percent(item.value) }}%</span>
      </li>
    </ul>
    <p v-else class="text-muted asset-chart-empty">{{ t('chart.empty') }}</p>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from '../i18n';
import * as echarts from 'echarts/core';
import { PieChart } from 'echarts/charts';
import { TooltipComponent, LegendComponent } from 'echarts/components';
import { CanvasRenderer } from 'echarts/renderers';

echarts.use([PieChart, TooltipComponent, LegendComponent, CanvasRenderer]);

const props = defineProps<{
  property: number;
  personality: number;
  mixed: number;
}>();

const { t } = useI18n();

const host = ref<HTMLElement | null>(null);
let chart: echarts.ECharts | null = null;
let resizeObserver: ResizeObserver | null = null;

/**
 * jsdom（单元测试）既没有 ResizeObserver 也没有 canvas 2D 实现。
 * 用 ResizeObserver 作为「真实浏览器」的判据，避免在测试里调用 getContext()
 * 触发 jsdom 的 "Not implemented" 输出（本项目要求 stderr 干净）。
 */
function canvasAvailable(): boolean {
  if (typeof ResizeObserver === 'undefined') return false;
  try {
    const c = document.createElement('canvas');
    return !!c.getContext('2d');
  } catch {
    return false;
  }
}

const total = computed(() => props.property + props.personality + props.mixed);

const legend = computed(() => [
  { name: t('cat.property'), value: props.property, color: '#b4594a' },
  { name: t('cat.personality'), value: props.personality, color: '#3f6f6a' },
  { name: t('cat.mixed'), value: props.mixed, color: '#c08a3e' },
]);

const ariaLabel = computed(
  () => t('chart.aria', { p: props.property, q: props.personality, m: props.mixed, total: total.value })
);

function percent(v: number) {
  return total.value === 0 ? 0 : Math.round((v / total.value) * 100);
}

function render() {
  if (!host.value || !canvasAvailable()) return;
  if (!chart) chart = echarts.init(host.value, undefined, { renderer: 'canvas' });
  chart.setOption({
    tooltip: { trigger: 'item', formatter: '{b}: {c} ' + t('dashboard.itemSuffix') + ' ({d}%)' },
    series: [
      {
        type: 'pie',
        radius: ['58%', '82%'],
        avoidLabelOverlap: true,
        itemStyle: { borderWidth: 2, borderColor: 'transparent' },
        label: { show: false },
        labelLine: { show: false },
        data: legend.value.map(i => ({ name: i.name, value: i.value, itemStyle: { color: i.color } })),
      },
    ],
  });
  chart.resize();
}

onMounted(() => {
  render();
  if (typeof ResizeObserver !== 'undefined' && host.value) {
    resizeObserver = new ResizeObserver(() => chart?.resize());
    resizeObserver.observe(host.value);
  }
});

watch(
  () => [props.property, props.personality, props.mixed],
  () => {
    if (total.value === 0 && chart) {
      chart.clear();
      return;
    }
    render();
  }
);

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  chart?.dispose();
  chart = null;
});
</script>

<style scoped>
.asset-chart {
  display: flex;
  align-items: center;
  gap: 28px;
  flex-wrap: wrap;
}
.asset-chart-canvas {
  width: 180px;
  height: 180px;
  flex: 0 0 180px;
}
.asset-chart-legend {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  font-size: 13px;
  min-width: 180px;
}
.asset-chart-legend li {
  display: flex;
  align-items: center;
  gap: 8px;
}
.asset-chart-legend .dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex: none;
}
.asset-chart-legend .name { color: var(--gray-700); }
.asset-chart-legend .value { margin-left: auto; color: var(--gray-600); font-variant-numeric: tabular-nums; }
.asset-chart-empty { font-size: 13px; margin: 0; }
</style>
