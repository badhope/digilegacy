<template>
  <div class="report fade-in">
    <h2 style="font-size: 22px; font-weight: 700; margin-bottom: 24px;">{{ t('report.title') }}</h2>
    <p class="text-muted mb-32" style="font-size: 14px;">
      {{ t('report.subtitle') }}
    </p>

    <div v-if="loading" class="card" aria-busy="true" aria-live="polite">
      <div class="skeleton skeleton-line w-40"></div>
      <div class="skeleton skeleton-line w-60"></div>
      <div class="skeleton skeleton-line w-40"></div>
      <div class="skeleton skeleton-line w-60"></div>
      <p class="text-muted mt-16" style="font-size: 13px;">{{ t('report.generating') }}</p>
    </div>

    <div v-else-if="loadError" class="card" role="alert" style="border-color: var(--danger);">
      <h3 style="font-size: 16px; margin-bottom: 8px; color: var(--danger-strong);">{{ t('report.loadFailed') }}</h3>
      <p class="text-muted mb-16" style="font-size: 13px;">{{ loadError }}</p>
      <button class="btn btn-primary btn-sm" @click="loadReport">{{ t('report.reload') }}</button>
    </div>

    <div v-if="report" class="report-page">
      <!-- 文档页眉：品牌标识 + 文档类型，模拟正式文书的抬头 -->
      <div class="doc-header">
        <div class="doc-brand">
          <svg width="22" height="22" viewBox="0 0 40 40" fill="none" aria-hidden="true">
            <rect width="40" height="40" rx="9" fill="#b4594a"/>
            <circle cx="20" cy="20" r="11.6" stroke="#fff" stroke-width="2.1" opacity="0.95"/>
            <circle cx="20" cy="20" r="7.2" stroke="#fff" stroke-width="1.2" opacity="0.6"/>
            <circle cx="20" cy="20" r="2.9" fill="#fff"/>
          </svg>
          <span>{{ t('common.appName') }}</span>
        </div>
        <div class="doc-no">{{ t('report.docNo', { id: report.generated_at.replace(/[-: ]/g, '') }) }}</div>
      </div>

      <div class="report-title">{{ t('report.docTitle') }}</div>
      <div class="doc-meta">
        <span>{{ t('report.generatedAt', { time: report.generated_at }) }}</span>
        <span class="doc-meta-sep">·</span>
        <span>{{ t('report.internalOnly') }}</span>
      </div>

      <div class="report-section">
        <div class="report-section-title">{{ t('report.section1') }}</div>
        <table class="asset-table">
          <tbody>
            <tr><td><strong>{{ t('report.name') }}</strong></td><td>{{ report.user.full_name || t('report.notFilled') }}</td></tr>
            <tr><td><strong>{{ t('report.phone') }}</strong></td><td>{{ report.user.phone || t('report.notFilled') }}</td></tr>
            <tr><td><strong>{{ t('report.email') }}</strong></td><td>{{ report.user.email || t('report.notFilled') }}</td></tr>
            <tr><td><strong>{{ t('report.heir') }}</strong></td><td>{{ report.user.heir_primary_name || t('report.notFilled') }} ({{ report.user.heir_primary_relation || '-' }})</td></tr>
            <tr><td><strong>{{ t('report.emergency') }}</strong></td><td>{{ report.user.emergency_contact_name || t('report.notFilled') }} ({{ report.user.emergency_contact_relation || '-' }})</td></tr>
          </tbody>
        </table>
      </div>

      <div class="report-section">
        <div class="report-section-title">{{ t('report.section2') }}</div>
        <div class="stats-grid" style="grid-template-columns: repeat(4, 1fr);">
          <div class="text-center"><div style="font-size: 32px; font-weight: 700; color: var(--primary);">{{ report.assets.length }}</div><div class="text-muted" style="font-size: 12px;">{{ t('report.totalCount') }}</div></div>
          <div class="text-center"><div style="font-size: 32px; font-weight: 700; color: var(--success);">{{ report.property_assets_count }}</div><div class="text-muted" style="font-size: 12px;">{{ t('cat.property') }}</div></div>
          <div class="text-center"><div style="font-size: 32px; font-weight: 700; color: var(--secondary);">{{ report.personality_assets_count }}</div><div class="text-muted" style="font-size: 12px;">{{ t('cat.personality') }}</div></div>
          <div class="text-center"><div style="font-size: 32px; font-weight: 700;">¥{{ formatMoney(report.total_estimated_value) }}</div><div class="text-muted" style="font-size: 12px;">{{ t('report.totalValue') }}</div></div>
        </div>
      </div>

      <div class="report-section">
        <div class="report-section-title">{{ t('report.section3') }}</div>
        <table class="asset-table">
          <thead><tr><th>{{ t('report.colNo') }}</th><th>{{ t('report.colPlatform') }}</th><th>{{ t('report.colAccount') }}</th><th>{{ t('report.colCategory') }}</th><th>{{ t('report.colValue') }}</th><th>{{ t('report.colDisposal') }}</th><th>{{ t('report.colHeir') }}</th></tr></thead>
          <tbody>
            <tr v-for="(asset, idx) in report.assets" :key="asset.id">
              <td>{{ idx + 1 }}</td>
              <td>{{ asset.platform_name }}</td>
              <td>{{ asset.account_identifier }}</td>
              <td>{{ categoryLabel(asset.category) }}</td>
              <td>{{ asset.estimated_value ? '¥' + formatMoney(asset.estimated_value) : '-' }}</td>
              <td>{{ disposalLabel(asset.disposal_method) }}</td>
              <td>{{ asset.heir_name || '-' }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="report-section">
        <div class="report-section-title">{{ t('report.section4') }}</div>
        <div v-for="(asset, idx) in inheritableAssets" :key="asset.id" style="margin-bottom: 12px; padding: 12px; background: var(--gray-50); border-radius: 8px; border-left: 3px solid var(--primary);">
          <strong>{{ idx + 1 }}. {{ asset.platform_name }}</strong>
          <p style="font-size: 13px; color: var(--gray-600); margin-top: 4px;">
            {{ t('report.disposalMethod', { method: disposalLabel(asset.disposal_method) }) }} |
            <span v-if="asset.heir_name">{{ heirInfoText(asset) }}</span>
          </p>
        </div>
      </div>

      <!-- 平台办理指引：与导出物来自同一份后端数据（report.policies），
           预览与导出不再各自拼一份，避免「预览有、导出没有」。 -->
      <div class="report-section" v-if="report.policies && report.policies.length > 0">
        <div class="report-section-title">{{ t('report.section5Policies') }}</div>
        <div v-for="p in report.policies" :key="p.platform_id" style="margin-bottom: 12px; padding: 12px; background: var(--gray-50); border-radius: 8px; border-left: 3px solid var(--secondary);">
          <div style="display:flex; gap:8px; align-items:baseline; flex-wrap:wrap; margin-bottom:6px;">
            <strong>{{ p.name }}</strong>
            <span v-if="p.estimated_difficulty" class="tag tag-secondary" style="font-size:11px;">{{ t('report.policyDifficulty') }}：{{ p.estimated_difficulty }}</span>
            <span v-if="p.estimated_time_days" class="tag tag-secondary" style="font-size:11px;">{{ t('report.policyDuration') }}：{{ p.estimated_time_days }}</span>
            <span class="tag" :class="p.has_source ? 'tag-success' : 'tag-warning'" style="font-size:11px;">
              {{ p.has_source ? t('report.policySourced') : t('report.policyUnsourced') }}
            </span>
          </div>
          <p style="font-size: 12px; color: var(--gray-600); margin: 4px 0;">
            <strong>{{ t('report.policyDocs') }}</strong>{{ p.required_documents || t('report.notFilled') }}
          </p>
          <p style="font-size: 12px; color: var(--gray-600); margin: 4px 0;">
            <strong>{{ t('report.policyProcess') }}</strong>{{ p.process_description || t('report.notFilled') }}
          </p>
          <p v-if="p.monetization_path" style="font-size: 12px; color: var(--gray-600); margin: 4px 0;">
            <strong>{{ t('report.policyPath') }}</strong>{{ p.monetization_path }}
          </p>
          <p v-if="p.risk_warnings" style="font-size: 12px; color: var(--warning-strong); margin: 4px 0;">
            <strong>{{ t('report.policyRisks') }}</strong>{{ p.risk_warnings }}
          </p>
          <p v-if="p.official_policy_url" style="font-size: 11px; color: var(--gray-500); margin: 4px 0; word-break: break-all;">
            <strong>{{ t('report.policyOfficial') }}</strong>{{ p.official_policy_url }}
          </p>
          <p v-if="!p.has_source" style="font-size: 11px; color: var(--warning-strong); margin: 4px 0;">
            {{ t('report.policyUnsourcedNote') }}
          </p>
        </div>
      </div>

      <!-- 权威来源：导出里有的，预览也必须有，否则又是一次「导出与预览不一致」 -->
      <div class="report-section" v-if="report.authorities && report.authorities.length > 0">
        <div class="report-section-title">{{ t('report.sectionAuthorities') }}</div>
        <div v-for="a in report.authorities" :key="a.id" style="margin-bottom: 12px; padding: 12px; background: var(--gray-50); border-radius: 8px;">
          <div style="display: flex; gap: 8px; align-items: baseline; flex-wrap: wrap; margin-bottom: 4px;">
            <strong style="font-size: 13px;">{{ isEnLocale ? a.title_en : a.title_zh }}</strong>
            <span class="tag tag-secondary" style="font-size: 11px;">{{ t('report.authorityReachable') }}</span>
            <span class="tag" :class="a.content_matched ? 'tag-success' : 'tag-warning'" style="font-size: 11px;">
              {{ a.content_matched ? t('report.authorityMatched') : t('report.authorityUnmatched') }}
            </span>
            <span v-if="a.kind === 'portal'" class="tag tag-warning" style="font-size: 11px;">{{ t('report.authorityPortal') }}</span>
          </div>
          <p style="font-size: 12px; color: var(--gray-600); margin: 4px 0;">{{ isEnLocale ? a.note_en : a.note_zh }}</p>
          <p v-if="a.kind === 'portal'" style="font-size: 11px; color: var(--warning-strong); margin: 4px 0;">
            {{ isEnLocale ? a.portal_note_en : a.portal_note_zh }}
          </p>
          <p style="font-size: 11px; color: var(--gray-500); margin: 4px 0; word-break: break-all;">
            <span v-if="a.last_checked">{{ t('report.authorityChecked', { date: a.last_checked }) }} · </span>{{ a.url }}
          </p>
        </div>
      </div>

      <div class="report-section">
        <div class="report-section-title">{{ t('report.section6') }}</div>
        <div style="font-size: 13px; color: var(--gray-600); line-height: 1.8;">
          <p style="margin: 4px 0;">{{ t('report.statement1') }}</p>
          <p style="margin: 4px 0;">{{ t('report.statement2') }}</p>
          <p style="margin: 4px 0;">{{ t('report.statement3') }}</p>
          <p style="margin: 4px 0;">{{ t('report.statement4') }}</p>
          <p style="margin: 4px 0;">{{ t('report.statement5') }}</p>
          <p style="margin: 4px 0;">{{ t('report.statement6') }}</p>
        </div>
      </div>

      <div class="report-section" style="margin-top: 48px;">
        <div style="display: flex; justify-content: space-between;">
          <div style="text-align: center;"><div style="width: 200px; border-bottom: 1px solid var(--gray-400); margin-bottom: 8px;">&nbsp;</div><span style="font-size: 13px; color: var(--gray-500);">{{ t('report.signature') }}</span></div>
          <div style="text-align: center;"><div style="width: 200px; border-bottom: 1px solid var(--gray-400); margin-bottom: 8px;">&nbsp;</div><span style="font-size: 13px; color: var(--gray-500);">{{ t('report.date') }}</span></div>
        </div>
      </div>

      <!-- 文档页脚：与页眉呼应，标注来源与保密属性 -->
      <div class="doc-footer">
        <span>{{ t('report.footer') }}</span>
        <span>{{ t('report.pageNo', { page: 1, pages: 1 }) }}</span>
      </div>
    </div>

    <div v-if="exportPath" class="card mt-24 tag-success" style="background: var(--success-light); border-color: var(--success);">
      <div style="display:flex;gap:12px;align-items:center;justify-content:space-between;flex-wrap:wrap;">
        <span>{{ t('report.savedTo') }}<code style="font-size:12px;word-break:break-all;">{{ exportPath }}</code></span>
        <div style="display:flex;gap:8px;">
          <!-- 导出后只给一行路径，普通用户找不到（应用数据目录是隐藏的） -->
          <button v-if="canReveal" class="btn btn-secondary btn-sm" @click="revealExport">{{ t('report.revealFile') }}</button>
          <button class="btn btn-secondary btn-sm" @click="copyExportPath">{{ t('report.copyPath') }}</button>
        </div>
      </div>
    </div>
    <div v-if="exportError" class="card mt-24 tag-danger" style="background: var(--danger-light, #fdecea); border-color: var(--danger);" role="alert">
      {{ exportError }}
    </div>

    <div class="mt-24 text-center" v-if="report">
      <button class="btn btn-primary btn-lg" @click="exportReport"><DownloadOutline /> {{ t('report.exportReport') }}</button>
      <button class="btn btn-secondary btn-lg ml-12" @click="exportZip"><ArchiveOutline /> {{ t('report.exportZip') }}</button>
      <button class="btn btn-secondary btn-lg ml-12" @click="exportActionGuide"><ListOutline /> {{ t('report.exportActionGuide') }}</button>
      <button class="btn btn-secondary btn-lg ml-12" @click="handlePrint"><PrintOutline /> {{ t('report.print') }}</button>
    </div>

    <!-- 内置 Agent：多轮本地工具检索，步骤可见，最终仍是草稿/建议。 -->
    <div class="card mt-24" v-if="assistantAvailable" data-testid="agent-card">
      <div class="card-title mb-8">{{ t('agent.title') }}</div>
      <p class="text-muted" style="font-size: 13px;">{{ t('agent.desc') }}</p>
      <div class="form-group" style="margin-top: 12px;">
        <label class="form-label" for="agent-task">{{ t('agent.taskLabel') }}</label>
        <textarea class="form-textarea" id="agent-task" v-model="agentTask" :placeholder="t('agent.taskPlaceholder')"></textarea>
      </div>
      <button class="btn btn-primary btn-sm" :disabled="agentBusy || !agentTask.trim()" data-testid="agent-run" @click="runAgent">
        {{ agentBusy ? t('agent.running') : t('agent.run') }}
      </button>
      <div v-if="agentSteps.length > 0" class="agent-timeline" style="margin-top: 12px;">
        <div v-for="(step, i) in agentSteps" :key="i" class="agent-step" style="padding: 8px; margin-top: 6px; background: var(--gray-50); border-radius: 6px; font-size: 12px;">
          <strong>{{ step.kind === 'tool_start' ? t('agent.toolStarting') : step.kind === 'tool_end' ? t('agent.toolFinished') : step.kind === 'started' ? t('agent.started') : t('agent.finished') }}</strong>
          <span v-if="step.tool"> · {{ step.tool }}</span>
          <span v-if="step.result" class="form-hint" style="display: block; margin-top: 3px; white-space: pre-wrap;">{{ step.result }}</span>
        </div>
      </div>
      <div v-if="agentAnswer" style="margin-top: 12px; padding: 12px; background: var(--primary-light); border-radius: 8px;">
        <strong style="font-size: 13px;">{{ t('agent.resultTitle') }}</strong>
        <pre style="white-space: pre-wrap; font-size: 13px; margin-top: 6px; font-family: inherit;">{{ agentAnswer }}</pre>
        <p class="form-hint">{{ t('agent.draftNotice') }}</p>
      </div>
      <p v-if="agentError" role="alert" style="font-size: 13px; color: var(--danger-strong); margin-top: 8px;">{{ agentError }}</p>
    </div>

    <!-- AI 助手（可选）：默认关闭，不内置任何端点。提问前必须先看「将发送什么」。 -->
    <div class="card mt-24" v-if="assistantAvailable">
      <div class="card-title mb-8">{{ t('ai.askTitle') }}</div>
      <p class="text-muted" style="font-size: 13px;">{{ t('ai.askDesc') }}</p>
      <div class="form-group" style="margin-top: 12px;">
        <label class="form-label" for="ai-question">{{ t('ai.question') }}</label>
        <textarea class="form-textarea" id="ai-question" v-model="aiQuestion" :placeholder="t('ai.questionPlaceholder')"></textarea>
      </div>
      <div style="display: flex; gap: 12px; flex-wrap: wrap;">
        <button class="btn btn-secondary btn-sm" :disabled="aiBusy" @click="previewAi">{{ t('ai.preview') }}</button>
        <button class="btn btn-primary btn-sm" :disabled="aiBusy || !aiPreview" @click="askAi">{{ aiBusy ? t('ai.asking') : t('ai.ask') }}</button>
      </div>
      <div v-if="aiPreview" style="margin-top: 12px; padding: 12px; background: var(--gray-50); border-radius: 8px;">
        <strong style="font-size: 13px;">{{ t('ai.previewTitle') }}</strong>
        <p style="font-size: 12px; color: var(--gray-600); margin-top: 6px;">{{ t('ai.previewNote', { chars: aiPreview.chars }) }}</p>
        <ul style="font-size: 12px; color: var(--gray-600); margin: 6px 0 0 20px;">
          <li v-for="f in aiPreview.fields" :key="f">{{ f }}</li>
        </ul>
      </div>
      <div v-if="aiAnswer" style="margin-top: 12px; padding: 12px; background: var(--primary-light); border-radius: 8px;">
        <strong style="font-size: 13px;">{{ t('ai.answerTitle') }}</strong>
        <pre style="white-space: pre-wrap; font-size: 13px; margin-top: 6px; font-family: inherit;">{{ aiAnswer }}</pre>
        <p class="form-hint">{{ t('ai.unreviewed') }}</p>
      </div>

      <p v-if="aiError" role="alert" style="font-size: 13px; color: var(--danger-strong); margin-top: 8px;">{{ aiError }}</p>
    </div>

    <!-- 本地问答历史：与助手开关无关——历史是本地留痕，助手未启用也能回看与清除 -->
    <div class="card mt-24" v-if="chatHistory.length > 0">
      <div style="display: flex; justify-content: space-between; align-items: center;">
        <div class="card-title mb-8">{{ t('ai.historyTitle') }}</div>
        <button class="btn btn-secondary btn-sm" data-testid="ai-history-clear" @click="clearChatHistory">{{ t('ai.historyClear') }}</button>
      </div>
      <div v-for="h in chatHistory" :key="h.id" style="margin-top: 10px; padding: 10px; background: var(--gray-50); border-radius: 8px;">
        <div style="font-size: 13px; font-weight: 500;">Q: {{ h.question }}</div>
        <pre style="white-space: pre-wrap; font-size: 12px; margin: 6px 0 0; font-family: inherit; color: var(--gray-600);">A: {{ h.answer }}</pre>
        <div style="font-size: 11px; color: var(--gray-400); margin-top: 4px;">{{ h.model }} · {{ h.created_at }}</div>
      </div>
    </div>

    <!-- 家人演练：只用元数据检查「家人拿到后会卡在哪」，不读取任何凭据 -->
    <div class="card mt-24">
      <div class="card-title mb-8">{{ t('drill.title') }}</div>
      <p class="text-muted" style="font-size: 13px;">{{ t('drill.desc') }}</p>
      <div style="margin-top: 12px;">
        <button class="btn btn-secondary btn-sm" :disabled="drillBusy" @click="runDrill">
          {{ drillBusy ? t('drill.running') : t('drill.run') }}
        </button>
      </div>
      <div v-if="drill" style="margin-top: 14px;">
        <p style="font-size: 13px; color: var(--gray-600);" aria-live="polite">
          {{ t('drill.summary', { platforms: drill.platforms, assets: drill.assets }) }}
        </p>
        <ul class="drill-list">
          <li v-for="c in drill.checks" :key="c.key" :class="{ ok: c.ok }">
            <span aria-hidden="true">{{ c.ok ? '✓' : '✗' }}</span>
            {{ drillCheckText(c) }}
          </li>
        </ul>
        <p v-if="drill.checks.every(c => c.ok)" class="tag-success" style="font-size: 13px; margin-top: 8px;">
          {{ t('drill.allGood') }}
        </p>
      </div>
      <p v-if="drillError" role="alert" style="font-size: 13px; color: var(--danger-strong); margin-top: 8px;">{{ drillError }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { Channel } from '@tauri-apps/api/core';
import { invoke, isTauriEnv } from '../api/invoke';

import { useI18n } from '../i18n';
import { formatMoney, categoryLabel, disposalLabel } from '../utils/format';
import ArchiveOutline from '@vicons/ionicons5/es/ArchiveOutline';
import DownloadOutline from '@vicons/ionicons5/es/DownloadOutline';
import PrintOutline from '@vicons/ionicons5/es/PrintOutline';
import ListOutline from '@vicons/ionicons5/es/ListOutline';
import { toast, confirmDialog } from '../composables/feedback';
import type {
  ApiResponse, AssistantConfig, Authority, DigitalAsset, DrillReport, OutboundPreview,
  PlatformPolicyBrief,
} from '../types';

interface InheritanceReportData {
  user: {
    full_name: string; phone: string; email: string;
    heir_primary_name: string; heir_primary_relation: string;
    emergency_contact_name: string; emergency_contact_relation: string;
  };
  assets: DigitalAsset[];
  /** 平台办理指引：与导出 HTML 来自同一份后端数据 */
  policies: PlatformPolicyBrief[];
  /** 权威来源：与导出的「权威来源」章节同源 */
  authorities?: Authority[];
  total_estimated_value: number;
  property_assets_count: number;
  personality_assets_count: number;
  mixed_assets_count: number;
  generated_at: string;
}

const { t, locale } = useI18n();
/** 报告正文里的来源标题/说明要跟随界面语言，与导出报告同一套数据。 */
const isEnLocale = computed(() => locale.value === 'en-US');
const report = ref<InheritanceReportData | null>(null);
const exportPath = ref('');
const exportError = ref('');
const assistantAvailable = ref(false);
interface ChatHistoryEntry {
  id: number;
  question: string;
  answer: string;
  model: string;
  created_at: string;
}

const aiQuestion = ref('');
const aiPreview = ref<OutboundPreview | null>(null);
const aiAnswer = ref('');
const chatHistory = ref<ChatHistoryEntry[]>([]);
interface AgentStepView { kind: string; round: number; tool?: string; args?: string; result?: string }
const agentTask = ref('');
const agentSteps = ref<AgentStepView[]>([]);
const agentAnswer = ref('');
const agentError = ref('');
const agentBusy = ref(false);
const aiError = ref('');
const aiBusy = ref(false);

const drill = ref<DrillReport | null>(null);
const drillBusy = ref(false);
const drillError = ref('');
const loadError = ref('');
const loading = ref(true);
/** 浏览器预览模式没有 `reveal_exported_file`，按钮不该出现。 */
const canReveal = computed(() => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window);

/** 继承人一行文案：联系方式可能为空（只填了姓名），此时不能渲染成 "(null)"。
 *  历史上 `report.heirInfo` 直接把 null 插值进模板，英文界面会出现
 *  「Heir: 演示继承人 (null)」这种半成品。 */
function heirInfoText(asset: { heir_name?: string | null; heir_contact?: string | null }): string {
  const name = (asset.heir_name || '').trim();
  const contact = (asset.heir_contact || '').trim();
  return contact
    ? t('report.heirInfo', { name, contact })
    : t('report.heirInfoNoContact', { name });
}

const inheritableAssets = computed(() =>
  (report.value?.assets || []).filter(a => a.disposal_method !== 'no_action')
);

async function loadReport() {
  loadError.value = '';
  loading.value = true;
  try {
    const res = await invoke<ApiResponse<InheritanceReportData>>('get_inheritance_report');
    if (res.success && res.data) {
      report.value = res.data;
    } else if (!res.success) {
      loadError.value = res.message || t('report.loadFailed');
    }
  } catch (e) {
    // 页面会一直停在空态；必须让用户知道是「加载失败」而不是「没有资产」
    console.error('加载继承报告失败:', e);
    loadError.value = t('report.loadFailed') + '：' + String(e);
  } finally {
    loading.value = false;
  }
}

/** 由后端渲染报告 HTML：唯一事实源，含转义/打印样式/占位符拦截。
 *  前端不再用 `querySelector('.report-page').outerHTML` 抓 DOM——
 *  那样会把界面占位符与 Naive UI 注入样式一起打进文件。
 *  `locale` 必须随请求发出：导出语言要与界面一致，否则英文用户拿到中文文档。 */
async function fetchStandaloneHtml(): Promise<string> {
  const res = await invoke<ApiResponse<string>>('render_report', { locale: locale.value });
  if (!res.success || !res.data) {
    throw new Error(res.message || t('report.exportFailed'));
  }
  return res.data;
}

/** 生成并保存家人执行清单（自包含 HTML，按平台分组）。 */
async function exportActionGuide() {
  exportError.value = '';
  exportPath.value = '';
  try {
    const res = await invoke<ApiResponse<string>>('render_action_guide', { locale: locale.value });
    if (!res.success || !res.data) {
      exportError.value = res.message || t('report.actionGuideFailed');
      return;
    }
    const saved = await invoke<ApiResponse<string>>('export_report_to_file', { content: res.data });
    if (saved.success && saved.data) {
      exportPath.value = saved.data;
      toast(t('report.actionGuideSaved'));
    } else {
      exportError.value = saved.message || t('report.actionGuideFailed');
    }
  } catch (e) {
    exportError.value = t('report.actionGuideFailed') + '：' + String(e);
  }
}

/** 读助手配置：未启用时不显示提问面板，避免给人「有内置 AI」的错觉。 */
async function loadAssistantState() {
  try {
    const res = await invoke<ApiResponse<AssistantConfig>>('get_assistant_config');
    assistantAvailable.value = Boolean(res.success && res.data?.enabled);
  } catch {
    assistantAvailable.value = false;
  }
}

/** 提问前必须先预览：让用户看见「哪些字段会发出去」。 */
async function previewAi() {
  aiError.value = '';
  aiAnswer.value = '';
  aiBusy.value = true;
  try {
    const res = await invoke<ApiResponse<OutboundPreview>>('preview_assistant_payload', {
      question: aiQuestion.value,
    });
    if (res.success && res.data) aiPreview.value = res.data;
    else {
      aiPreview.value = null;
      aiError.value = res.message;
    }
  } catch (e) {
    aiError.value = String(e);
  } finally {
    aiBusy.value = false;
  }
}

async function askAi() {
  if (!aiPreview.value) return;
  aiBusy.value = true;
  aiError.value = '';
  aiAnswer.value = '';
  try {
    // 流式优先：回答逐字推送（模型支持 stream 时前端立即可读）。
    // 非 Tauri 环境（演示站/单测 jsdom）没有 Channel，直接走非流式；
    // Tauri 环境里老后端不认识该命令时同样回退。
    let streamedError = '';
    if (isTauriEnv()) {
      try {
        const channel = new Channel<string>();
        channel.onmessage = (delta: string) => {
          aiAnswer.value += delta;
        };
        const res = await invoke<ApiResponse<string>>('ask_assistant_streaming', {
          question: aiQuestion.value,
          locale: locale.value,
          onDelta: channel,
        });
        if (res.success && res.data) {
          aiAnswer.value = res.data;
        } else {
          streamedError = res.message;
        }
      } catch {
        // 老后端没有流式命令 → 回退
      }
    }
    if (!aiAnswer.value) {
      const res = await invoke<ApiResponse<string>>('ask_assistant', {
        question: aiQuestion.value,
        locale: locale.value,
      });
      if (res.success && res.data) aiAnswer.value = res.data;
      else if (!streamedError) streamedError = res.message;
      else streamedError = streamedError || res.message;
    }
    if (!aiAnswer.value && streamedError) aiError.value = streamedError;
    await loadChatHistory();
  } catch (e) {
    aiError.value = String(e);
  } finally {
    aiBusy.value = false;
  }
}

async function runAgent() {
  agentBusy.value = true;
  agentError.value = '';
  agentAnswer.value = '';
  agentSteps.value = [];
  try {
    if (isTauriEnv()) {
      const channel = new Channel<AgentStepView>();
      channel.onmessage = (event: AgentStepView) => agentSteps.value.push(event);
      const res = await invoke<ApiResponse<{ steps: AgentStepView[]; final_text: string; truncated: boolean }>>(
        'run_agent_task_streaming',
        { task: agentTask.value.trim(), onEvent: channel },
      );
      if (res.success && res.data) {
        agentSteps.value = res.data.steps ?? agentSteps.value;
        agentAnswer.value = res.data.final_text;
      } else {
        agentError.value = res.message;
      }
    } else {
      agentError.value = t('agent.desktopOnly');
    }
  } catch (e) {
    agentError.value = String(e);
  } finally {
    agentBusy.value = false;
  }
}

/** 本地问答历史（来自 ai_chat_log）。 */
async function loadChatHistory() {
  try {
    const res = await invoke<ApiResponse<ChatHistoryEntry[]>>('list_ai_chat', { limit: 20 });
    if (res.success && res.data) chatHistory.value = res.data;
  } catch {
    chatHistory.value = [];
  }
}

async function clearChatHistory() {
  const ok = await confirmDialog({
    title: t('ai.historyClearTitle'),
    message: t('ai.historyClearMessage'),
    danger: true,
  });
  if (!ok) return;
  try {
    await invoke<ApiResponse<null>>('clear_ai_chat');
    chatHistory.value = [];
  } catch (e) {
    aiError.value = String(e);
  }
}

/** 家人演练：报告「家人会在哪一步卡住」，不读取凭据。 */
async function runDrill() {
  drillBusy.value = true;
  drillError.value = '';
  try {
    const res = await invoke<ApiResponse<DrillReport>>('run_family_drill');
    if (res.success && res.data) drill.value = res.data;
    else drillError.value = res.message;
  } catch (e) {
    drillError.value = String(e);
  } finally {
    drillBusy.value = false;
  }
}

function drillCheckText(c: { key: string; ok: boolean; count: number }): string {
  switch (c.key) {
    case 'heir':
      return c.ok ? t('drill.heirOk') : t('drill.heirFail', { count: c.count });
    case 'policy':
      return c.ok ? t('drill.policyOk') : t('drill.policyFail', { count: c.count });
    case 'verified':
      return c.ok ? t('drill.verifiedOk') : t('drill.verifiedFail', { count: c.count });
    case 'trigger':
      return c.ok ? t('drill.triggerOk') : t('drill.triggerFail');
    default:
      return c.key;
  }
}

/** 在系统文件管理器中定位刚导出的文件（仅桌面端）。 */
async function revealExport() {
  if (!exportPath.value) return;
  try {
    const res = await invoke<ApiResponse<null>>('reveal_exported_file', { path: exportPath.value });
    if (!res.success) exportError.value = res.message;
  } catch (e) {
    exportError.value = String(e);
  }
}

async function exportReport() {
  if (!report.value) return;
  exportError.value = '';
  exportPath.value = '';
  try {
    const fullHtml = await fetchStandaloneHtml();
    const res = await invoke<ApiResponse<string>>('export_report_to_file', { content: fullHtml });
    if (res.success && res.data) {
      exportPath.value = res.data;
    } else {
      // 导出失败必须让用户看见：静默失败会让人以为文件已生成
      exportError.value = res.message || t('report.exportFailed');
    }
  } catch (e) {
    console.error('导出报告失败:', e);
    exportError.value = t('report.exportFailed') + '：' + String(e);
  }
}

async function exportZip() {
  if (!report.value) return;
  exportError.value = '';
  exportPath.value = '';
  try {
    await buildAndSaveZip();
  } catch (e) {
    // JSZip 生成、btoa 或 IPC 任一步失败都必须可见——
    // 旧实现只把 IPC 那一段包了 try，前面的失败会变成未处理拒绝，UI 毫无反应。
    console.error('导出工具包失败:', e);
    exportError.value = t('report.zipFailed', { e: String(e) });
  }
}

async function buildAndSaveZip() {
  if (!report.value) return;
  // @ts-ignore - jszip 需要 new 实例
  const j = new (await import('jszip').then(m => m.default || m));
  // @ts-ignore - 避免类型推断问题
  const zip: any = j;
  // 报告 HTML 与单文件导出同源（后端渲染），ZIP 里不会出现裸片段。
  const htmlContent = await fetchStandaloneHtml();
  zip.file('01-继承报告.html', htmlContent);
  // 添加资产清单 JSON
  zip.file('02-资产清单.json', JSON.stringify(report.value.assets, null, 2));
  // 添加统计摘要 JSON
  zip.file('03-统计摘要.json', JSON.stringify({
    total_assets: report.value.assets.length,
    property_assets: report.value.property_assets_count,
    personality_assets: report.value.personality_assets_count,
    total_value: report.value.total_estimated_value,
    generated_at: report.value.generated_at,
  }, null, 2));
  // 添加使用说明，让家人拿到工具包后知道从哪看起
  zip.file('00-使用说明.txt', [
    t('report.zipTitle'),
    '=================================',
    '',
    t('report.zipIntro'),
    '',
    t('report.zipFiles'),
    '  01-继承报告.html  — ' + t('report.zipFile1'),
    '  02-资产清单.json  — ' + t('report.zipFile2'),
    '  03-统计摘要.json  — ' + t('report.zipFile3'),
    '',
    t('report.zipOrder'),
    '  1) ' + t('report.zipOrder1'),
    '  2) ' + t('report.zipOrder2'),
    '  3) ' + t('report.zipOrder3'),
    '',
    t('report.zipTips'),
    '  · ' + t('report.zipTip1'),
    '  · ' + t('report.zipTip2'),
    '  · ' + t('report.zipTip3'),
    '',
    t('report.generatedAt', { time: report.value.generated_at }),
    t('report.zipFooter'),
  ].join('\n'));
  // 生成 ZIP 字节流
  const content = await zip.generateAsync({ type: 'uint8array' });
  // 转为 base64
  const base64 = btoa(
    Array.from(new Uint8Array(content))
      .map(b => String.fromCharCode(b))
      .join('')
  );
  // 通过 Tauri 命令保存文件
  const res = await invoke<ApiResponse<string>>('export_zip_to_file', { content: base64 });
  if (res.success && res.data) {
    exportPath.value = res.data;
  } else {
    exportError.value = res.message || t('report.zipFailedShort');
  }
}

async function copyExportPath() {
  try {
    await navigator.clipboard.writeText(exportPath.value);
    toast(t('report.pathCopied'));
  } catch {
    toast(t('report.pathCopyFailed'), 'error');
  }
}

function handlePrint() { window.print(); }

onMounted(async () => {
  // 报告页不再依赖前端平台库：政策文案由后端 `report.policies` 提供，
  // 预览与导出读同一份，避免两处各拼一条政策而漂移。
  await Promise.all([loadReport(), loadAssistantState(), loadChatHistory()]);
});
</script>

<style scoped>
/* 报告页：屏幕预览用「文档纸面」观感，打印时去装饰。
   这些类此前完全没有样式（scoped 段只有 4 行），导致页眉/标题挤在一起。 */
.drill-list {
  list-style: none;
  padding: 0;
  margin: 8px 0 0;
  font-size: 13px;
  color: var(--danger-strong);
}
.drill-list li { margin: 4px 0; }
.drill-list li.ok { color: var(--success-strong, #2f7a4f); }

.report-page {
  max-width: 800px;
  margin: 0 auto;
  background: var(--surface);
  padding: 44px 48px;
  border: 1px solid var(--gray-200);
  border-radius: var(--radius);
  box-shadow: var(--shadow-xs);
}

.doc-header {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--gray-200);
  margin-bottom: 40px;
}
.doc-brand {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--gray-600);
  letter-spacing: 0.5px;
}
.doc-no {
  text-align: right;
  font-size: 11px;
  color: var(--gray-400);
  letter-spacing: 0.5px;
  font-variant-numeric: tabular-nums;
}

.report-title {
  text-align: center;
  font-size: 25px;
  font-weight: 700;
  margin-bottom: 12px;
  letter-spacing: 6px;
  color: var(--gray-900);
}

.doc-meta {
  text-align: center;
  color: var(--gray-500);
  font-size: 12px;
  margin-bottom: 40px;
  letter-spacing: 0.3px;
}
.doc-meta-sep { margin: 0 8px; color: var(--gray-300); }

.report-section { margin-bottom: 28px; }
.report-section-title {
  font-size: 15px;
  font-weight: 600;
  padding-bottom: 9px;
  border-bottom: 2px solid var(--gray-200);
  margin-bottom: 14px;
  color: var(--gray-900);
  letter-spacing: 0.5px;
}

.doc-footer {
  display: flex;
  justify-content: space-between;
  margin-top: 44px;
  padding-top: 14px;
  border-top: 1px solid var(--gray-200);
  font-size: 11px;
  color: var(--gray-400);
  letter-spacing: 0.3px;
}

.stats-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 16px; margin: 16px 0; }
.stats-grid .text-center { padding: 12px; border: 1px solid var(--gray-200); border-radius: 8px; }

/* 打印：去卡片装饰，留装订边 */
@media print {
  .report-page {
    border: none;
    border-radius: 0;
    box-shadow: none;
    padding: 0;
    max-width: none;
    background: #fff;
  }
  .report-section { break-inside: avoid; }
}
</style>