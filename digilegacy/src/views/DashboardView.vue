<template>
  <div class="dashboard fade-in">
    <!-- 欢迎横幅 -->
    <div class="card mb-32 hero-card">
      <div style="display: flex; justify-content: space-between; align-items: center;">
        <div>
          <h2 style="font-size: 22px; font-weight: 700; margin-bottom: 8px;">
            {{ t('dashboard.hello', { name: displayName }) }}
          </h2>
          <p style="font-size: 14px; line-height: 1.6;">
            {{ t('dashboard.subtitle').split('\n')[0] }}<br>
            {{ t('dashboard.subtitle').split('\n')[1] }}
          </p>
          <div class="hero-status" role="status">{{ handoverStatus }}</div>
          <div style="margin-top: 16px; display: flex; gap: 12px;">
            <router-link to="/discover" class="btn btn-lg btn-white">{{ t('dashboard.startDiscover') }}</router-link>
            <router-link to="/report" class="btn btn-lg btn-ghost">{{ t('dashboard.viewReport') }}</router-link>
          </div>
        </div>
        <div class="dash-hero-icon" aria-hidden="true"><GlobeOutline /></div>
      </div>
    </div>

    <!-- 下一步：定位是「工具包」，首页先回答「现在该做什么」，百分比放在后面 -->
    <div class="card mb-24 next-step-card" v-if="(stats?.issues?.length ?? 0) === 0 && (stats?.total_assets ?? 0) > 0">
      <div class="card-title mb-8">{{ t('dashboard.nextStepTitle') }}</div>
      <p class="text-muted" style="font-size: 13px;">{{ t('dashboard.nextStepReady') }}</p>
      <div style="margin-top: 12px; display: flex; gap: 10px; flex-wrap: wrap;">
        <router-link to="/report" class="btn btn-primary btn-sm">{{ t('dashboard.nextStepReport') }}</router-link>
        <router-link to="/heir" class="btn btn-secondary btn-sm">{{ t('dashboard.nextStepHeir') }}</router-link>
      </div>
    </div>
    <!-- 待办卡片：百分比只说明「还差多少」，这里说明「差在哪、点哪里」 -->
    <div class="card mb-32" v-if="(stats?.issues?.length ?? 0) > 0">
      <div class="card-header">
        <div>
          <div class="card-title">{{ t('dashboard.issuesTitle') }}</div>
          <div class="card-subtitle">{{ t('dashboard.issuesSubtitle') }}</div>
        </div>
        <span class="tag tag-warning">{{ stats?.issues?.length ?? 0 }}{{ t('dashboard.issueSuffix') }}</span>
      </div>
      <div class="issue-grid">
        <button
          v-for="issue in stats?.issues ?? []"
          :key="issue.key"
          type="button"
          class="issue-card"
          :class="'issue-' + issue.severity"
          @click="$router.push(issue.target)"
        >
          <span class="issue-icon" aria-hidden="true">{{ issueIcon(issue.severity) }}</span>
          <span class="issue-body">
            <span class="issue-text">{{ issueText(issue) }}</span>
            <span class="issue-go">{{ t('dashboard.issueGo') }} →</span>
          </span>
        </button>
      </div>
    </div>

    <!-- 统计数据 -->
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-icon stat-blue"><DiamondOutline /></div>
        <div class="stat-info">
          <div class="stat-value">{{ stats?.total_assets ?? 0 }}</div>
          <div class="stat-label">{{ t('dashboard.totalAssets') }}</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon stat-green"><CashOutline /></div>
        <div class="stat-info">
          <div class="stat-value">¥{{ formatMoney(stats?.total_estimated_value ?? 0) }}</div>
          <div class="stat-label">{{ t('dashboard.totalValue') }}</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon stat-purple"><DocumentTextOutline /></div>
        <div class="stat-info">
          <div class="stat-value">{{ stats?.disposal_planned_count ?? 0 }}</div>
          <div class="stat-label">{{ t('dashboard.plannedCount') }}</div>
        </div>
      </div>
      <div class="stat-card">
        <div class="stat-icon stat-yellow"><StatsChartOutline /></div>
        <div class="stat-info">
          <!-- 0 资产时 0% 没有意义：显示占位符，与下方「尚未开始」保持一致 -->
          <div class="stat-value">{{ (stats?.total_assets ?? 0) === 0 ? '—' : (stats?.completion_percentage ?? 0) + '%' }}</div>
          <div class="stat-label">{{ t('dashboard.completion') }}</div>
        </div>
      </div>
    </div>

    <!-- 完成度：一个环看总量，四个分段看构成。
         「一个百分比」说不清差在哪，所以环旁边必须有四项分解；
         反过来说，四项分解也需要一个一眼可见的总量，二者缺一不可。 -->
    <div class="card mb-32">
      <div class="card-header">
        <div>
          <div class="card-title">{{ t('dashboard.progressTitle') }}</div>
          <div class="card-subtitle">{{ t('dashboard.progressSubtitle') }}</div>
        </div>
        <div style="text-align: right;">
          <!-- 0 资产时百分比没有意义：显示 0% 会让用户以为「我做得不好」，
               而实际是「还没开始」。这里直接给出起点引导。 -->
          <span v-if="(stats?.total_assets ?? 0) === 0" class="tag tag-secondary">{{ t('dashboard.notStarted') }}</span>
          <template v-else>
            <span class="tag" :class="(stats?.completion_percentage ?? 0) >= 80 ? 'tag-success' : 'tag-warning'">
              {{ (stats?.completion_percentage ?? 0) >= 80 ? t('dashboard.good') : t('dashboard.todo') }}
            </span>
            <!-- 说明百分比是怎么算出来的：四项各 25%，用户可自行核对 -->
            <div class="text-muted" style="font-size: 11px; margin-top: 4px;">{{ t('dashboard.scoreHint') }}</div>
          </template>
        </div>
      </div>

      <div class="progress-hero">
        <div
          class="metric-ring"
          :style="{ '--pct': stats?.total_assets ? (stats?.completion_percentage ?? 0) : 0, '--ring-color': ringColor }"
          role="img"
          :aria-label="t('dashboard.completionAria', { pct: stats?.total_assets ? (stats?.completion_percentage ?? 0) : 0 })"
        >
          <span class="metric-ring-value">{{ stats?.total_assets ? (stats?.completion_percentage ?? 0) + '%' : '—' }}</span>
          <span class="metric-ring-label">{{ t('dashboard.completion') }}</span>
        </div>

        <div class="progress-breakdown">
          <!-- 四维分段条：宽度按各项完成比例，颜色区分「达标/待办」 -->
          <div class="segmented-bar" role="img" :aria-label="t('dashboard.dimensionAria')">
            <span
              v-for="dim in dimensions"
              :key="dim.key"
              :style="{ width: dim.share + '%', background: dim.color }"
              :title="`${t(dim.labelKey)}: ${dim.done}/${dim.total}`"
            ></span>
          </div>
          <div class="segmented-legend">
            <span v-for="dim in dimensions" :key="dim.key">
              <span class="dot" :style="{ background: dim.color }"></span>
              {{ t(dim.labelKey) }}
              <strong :class="{ 'dim-ok': dim.ok }">{{ dim.done }}/{{ dim.total }}</strong>
            </span>
          </div>
        </div>
      </div>

      <div class="checklist-grid">
        <div class="check-item" :class="{ done: isDone(stats?.disposal_planned_count) }">
          <span class="check-mark" aria-hidden="true">{{ isDone(stats?.disposal_planned_count) ? '✓' : '·' }}</span>
          <span>{{ t('dashboard.checkDisposal', { done: stats?.disposal_planned_count ?? 0, total: stats?.total_assets ?? 0 }) }}</span>
        </div>
        <div class="check-item" :class="{ done: isDone(stats?.heir_assigned_count) }">
          <span class="check-mark" aria-hidden="true">{{ isDone(stats?.heir_assigned_count) ? '✓' : '·' }}</span>
          <span>{{ t('dashboard.checkHeir', { done: stats?.heir_assigned_count ?? 0, total: stats?.total_assets ?? 0 }) }}</span>
        </div>
        <div class="check-item" :class="{ done: isDone(stats?.identifier_ready_count) }">
          <span class="check-mark" aria-hidden="true">{{ isDone(stats?.identifier_ready_count) ? '✓' : '·' }}</span>
          <span>{{ t('dashboard.checkIdentifier', { done: stats?.identifier_ready_count ?? 0, total: stats?.total_assets ?? 0 }) }}</span>
        </div>
        <div class="check-item" :class="{ done: stats?.trigger_ready, warn: !stats?.trigger_ready && stats?.dms_enabled }">
          <span class="check-mark" aria-hidden="true">{{ stats?.trigger_ready ? '✓' : (stats?.dms_enabled ? '!' : '·') }}</span>
          <span>{{ triggerText }}</span>
        </div>
      </div>
      <p class="text-muted mt-16" style="font-size: 13px;">
        {{ t('dashboard.progressDetail', { planned: stats?.disposal_planned_count ?? 0, total: stats?.total_assets ?? 0 }) }}
      </p>
    </div>

    <!-- 资产构成：图表比三段文字更快回答「我到底有多少、偏哪一类」 -->
    <div class="card mb-32">
      <div class="card-header">
        <div>
          <div class="card-title">{{ t('dashboard.composition') }}</div>
          <div class="card-subtitle">{{ t('dashboard.compositionSub') }}</div>
        </div>
      </div>
      <AssetCompositionChart
        :property="stats?.property_count ?? 0"
        :personality="stats?.personality_count ?? 0"
        :mixed="stats?.mixed_count ?? 0"
      />
    </div>

    <!-- 家人视角：持有者要看到的不是后台报表，而是「家人拿到后会看到什么」 -->
    <div class="card mb-32 handover-preview">
      <div class="card-header">
        <div>
          <div class="card-title">{{ t('dashboard.handoverTitle') }}</div>
          <div class="card-subtitle">{{ t('dashboard.handoverSub') }}</div>
        </div>
      </div>
      <div class="handover-grid">
        <div class="handover-item">
          <strong>{{ stats?.total_assets ?? 0 }}</strong>
          <span>{{ t('dashboard.handoverAssets') }}</span>
        </div>
        <div class="handover-item">
          <strong>{{ stats?.heir_assigned_count ?? 0 }}</strong>
          <span>{{ t('dashboard.handoverHeirs') }}</span>
        </div>
        <div class="handover-item">
          <strong>{{ stats?.trigger_ready ? t('dashboard.handoverReady') : t('dashboard.handoverNotReady') }}</strong>
          <span>{{ t('dashboard.handoverTrigger') }}</span>
        </div>
      </div>
      <div style="margin-top: 14px; display: flex; gap: 10px; flex-wrap: wrap;">
        <router-link to="/report" class="btn btn-primary btn-sm">{{ t('dashboard.handoverOpenReport') }}</router-link>
        <router-link to="/heir" class="btn btn-secondary btn-sm">{{ t('dashboard.handoverOpenGuide') }}</router-link>
      </div>
    </div>

    <!-- 最近资产 -->
    <div class="card" v-if="recentAssets.length > 0">
      <div class="card-header">
        <div class="card-title">{{ t('dashboard.recent') }}</div>
        <router-link to="/assets" class="btn btn-sm btn-outline">{{ t('common.viewAll') }}</router-link>
      </div>
      <table class="asset-table">
        <thead><tr><th>{{ t('dashboard.colPlatform') }}</th><th>{{ t('dashboard.colCategory') }}</th><th>{{ t('dashboard.colValue') }}</th><th>{{ t('dashboard.colDisposal') }}</th></tr></thead>
        <tbody>
          <tr v-for="asset in recentAssets" :key="asset.id">
            <td><strong>{{ asset.platform_name }}</strong></td>
            <td><span class="tag" :class="'tag-' + asset.category">{{ categoryLabel(asset.category) }}</span></td>
            <td>{{ asset.estimated_value ? '¥' + formatMoney(asset.estimated_value) : '-' }}</td>
            <td style="color: var(--gray-600);">{{ disposalLabel(asset.disposal_method) }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 空状态 -->
    <div class="card text-center" v-else style="padding: 48px;">
      <EmptyState kind="search" :size="160" />
      <h3 style="font-size: 18px; margin-bottom: 8px;">{{ t('dashboard.emptyTitle') }}</h3>
      <p class="text-muted mb-24" style="font-size: 14px;">{{ t('dashboard.emptyDesc') }}</p>
      <router-link to="/discover" class="btn btn-primary btn-lg">{{ t('dashboard.emptyCta') }}</router-link>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue';

import { useI18n } from '../i18n';
import CashOutline from '@vicons/ionicons5/es/CashOutline';
import DiamondOutline from '@vicons/ionicons5/es/DiamondOutline';
import DocumentTextOutline from '@vicons/ionicons5/es/DocumentTextOutline';
import GlobeOutline from '@vicons/ionicons5/es/GlobeOutline';
import StatsChartOutline from '@vicons/ionicons5/es/StatsChartOutline';
import { useDashboardStore } from '../stores/dashboard';
import { useAssetsStore } from '../stores/assets';
import { formatMoney, categoryLabel, disposalLabel } from '../utils/format';
import EmptyState from '../components/EmptyState.vue';
import AssetCompositionChart from '../components/AssetCompositionChart.vue';

const { t } = useI18n();
const dashboard = useDashboardStore();
const assetsStore = useAssetsStore();

const stats = computed(() => dashboard.stats);
const displayName = computed(() => dashboard.user?.full_name || t('dashboard.welcomeDefault'));
const recentAssets = computed(() => assetsStore.assets.slice(0, 5));

/** 某项是否已全部达标（0 资产时不算完成，避免空库显示 4 个勾）。 */
function isDone(count: number | undefined): boolean {
  const total = stats.value?.total_assets ?? 0;
  return total > 0 && (count ?? 0) >= total;
}

/** 一句话说明当前处于哪一步：空库引导、存在待办、已可交付。 */
const handoverStatus = computed(() => {
  const total = stats.value?.total_assets ?? 0;
  if (total === 0) return t('dashboard.statusEmpty');
  const issues = stats.value?.issues?.length ?? 0;
  if (issues > 0) return t('dashboard.statusTodo', { count: issues });
  return t('dashboard.statusDone');
});

/** 待办严重度图标：拦路（家人无法执行）、质量缺陷、优化建议。 */
function issueIcon(severity: string): string {
  if (severity === 'blocker') return '✕';
  if (severity === 'warn') return '!';
  return '?';
}

/** 待办文案：按 key 映射，带条数。未知 key 原样显示，便于发现漏配。 */
function issueText(issue: { key: string; count: number }): string {
  return t(`dashboard.issue.${issue.key}`, { count: issue.count });
}

/** 触发链文案必须说出「靠什么通道」，不能只说「已开启」。 */
const triggerText = computed(() => {
  const ch = stats.value?.trigger_channel ?? 'none';
  if (!stats.value?.dms_enabled) return t('dashboard.checkTriggerOff');
  if (ch === 'smtp') return t('dashboard.checkTriggerSmtp');
  if (ch === 'heartbeat') return t('dashboard.checkTriggerHeartbeat');
  return t('dashboard.checkTriggerNoChannel');
});

/** 四维完成度：与后端 `completion_percentage`（四项各 25%）同口径，
 *  但拆开显示「每项做了几个/共几个」，用户能看出差在哪一项。 */
const dimensions = computed(() => {
  const total = stats.value?.total_assets ?? 0;
  const rows = [
    { key: 'disposal', labelKey: 'dashboard.checkDisposalLabel', done: stats.value?.disposal_planned_count ?? 0 },
    { key: 'heir', labelKey: 'dashboard.checkHeirLabel', done: stats.value?.heir_assigned_count ?? 0 },
    { key: 'identifier', labelKey: 'dashboard.checkIdentifierLabel', done: stats.value?.identifier_ready_count ?? 0 },
    { key: 'trigger', labelKey: 'dashboard.checkTriggerLabel', done: stats.value?.trigger_ready ? total : 0 },
  ];
  return rows.map((r) => {
    const ratio = total > 0 ? r.done / total : 0;
    return {
      ...r,
      total,
      ok: total > 0 && r.done >= total,
      // 分段条按「该维完成比例」占总宽度的 1/4 展示，四项等宽，避免某一项压倒其余
      share: (ratio * 100) / rows.length,
      color: total > 0 && r.done >= total ? 'var(--success)' : 'var(--warning)',
    };
  });
});

/** 环形进度颜色：达标绿、接近蓝、未过半用暖色，让「还差多少」有情绪层次。 */
const ringColor = computed(() => {
  const pct = stats.value?.completion_percentage ?? 0;
  if (pct >= 100) return 'var(--success)';
  if (pct >= 60) return 'var(--primary)';
  return 'var(--warning)';
});

onMounted(async () => {
  await Promise.all([dashboard.refreshAll(), assetsStore.loadAssets()]);
});
</script>

<style scoped>
.hero-status {
  display: inline-block;
  margin-top: 12px;
  padding: 5px 12px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.16);
  font-size: 13px;
  font-weight: 500;
}
.handover-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 12px;
  margin-top: 14px;
}
.handover-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 12px 14px;
  background: var(--gray-50);
  border-radius: 10px;
}
.progress-hero {
  display: flex;
  align-items: center;
  gap: 28px;
  margin: 20px 0 8px;
}
.progress-breakdown { flex: 1; min-width: 0; }
.segmented-legend strong { color: var(--gray-700); font-variant-numeric: tabular-nums; }
.segmented-legend strong.dim-ok { color: var(--success-strong); }
@media (max-width: 760px) {
  .progress-hero { flex-direction: column; align-items: flex-start; gap: 16px; }
  .progress-breakdown { width: 100%; }
}
.handover-item strong { font-size: 20px; color: var(--gray-900); }
.handover-item span { font-size: 13px; color: var(--gray-600); }

.hero-card {
  /* 浅端压深至 #b45c4a：白字正文（14px）对比度 4.59:1，满足 WCAG AA */
  background: linear-gradient(135deg, #8a4534 0%, #b45c4a 100%);
  border: none;
  color: white;
}
.btn-white { background: white; color: var(--solid-primary); }
.stat-blue { background: var(--primary-light); }
.stat-green { background: var(--success-light); }
.stat-purple { background: var(--secondary-light); }
.stat-yellow { background: var(--warning-light); }
.checklist-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  gap: 10px;
  margin-top: 16px;
}
.check-item {
  display: flex;
  gap: 8px;
  align-items: flex-start;
  padding: 10px 12px;
  border: 1px solid var(--gray-200, #e8e4de);
  border-radius: 8px;
  font-size: 13px;
  color: var(--gray-600);
}
.check-item.warn {
  border-color: var(--warning);
  background: var(--warning-light);
  color: var(--gray-800, #3a352d);
}
.check-item.done {
  border-color: var(--success);
  background: var(--success-light);
  color: var(--gray-800, #3a352d);
}
.check-mark { font-weight: 700; min-width: 12px; }

.issue-grid {
  display: grid;
  /* 固定两列：auto-fit 在 4 项时会排成 3+1，视觉上像少了一块 */
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-top: 14px;
}
@media (max-width: 900px) {
  .issue-grid { grid-template-columns: 1fr; }
}
.issue-card {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  text-align: left;
  padding: 12px 14px;
  border: 1px solid var(--gray-200, #e8e4de);
  border-radius: 10px;
  background: var(--surface);
  font: inherit;
  color: inherit;
  cursor: pointer;
  transition: border-color 0.15s var(--ease-out), box-shadow 0.15s var(--ease-out);
}
.issue-card:hover, .issue-card:focus-visible {
  border-color: var(--primary);
  box-shadow: 0 2px 10px rgba(0,0,0,0.06);
}
.issue-icon {
  flex: 0 0 auto;
  width: 22px; height: 22px;
  display: inline-flex; align-items: center; justify-content: center;
  border-radius: 50%;
  font-size: 13px; font-weight: 700;
  line-height: 1;
}
.issue-blocker .issue-icon { background: var(--danger-light, #fdecea); color: var(--danger-strong); }
.issue-warn .issue-icon { background: var(--warning-light); color: var(--warning-strong); }
.issue-info .issue-icon { background: var(--primary-light); color: var(--primary); }
.issue-body { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.issue-text { font-size: 13px; line-height: 1.5; }
.issue-go { font-size: 11px; color: var(--gray-500); }
</style>
