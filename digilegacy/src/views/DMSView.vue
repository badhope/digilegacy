<template>
  <div class="dms fade-in">
    <h2 style="font-size: 22px; font-weight: 700; margin-bottom: 8px;">{{ t('nav.dms') }} · Dead Man's Switch</h2>
    <p class="text-muted mb-32" style="font-size: 14px;">{{ t('dms.subtitle') }}</p>

    <!-- 状态卡片 -->
    <div class="card mb-32" :class="statusClass">
      <div class="flex items-center gap-12">
        <div class="dms-status-icon" aria-hidden="true"><component :is="statusIcon" /></div>
        <div>
          <div style="font-size: 20px; font-weight: 700;" :style="{ color: statusColor }">{{ statusTitle }}</div>
          <div class="text-muted mt-16" style="font-size: 13px;">{{ statusSubtitle }}</div>
          <!-- 状态「安好」只说明签到计时正常，不等于家人能收到通知。
               仪表板把它算作 blocker；这里必须给出同一结论，否则两个页面自相矛盾。 -->
          <div v-if="!deliveryReady" class="dms-delivery-warn" role="alert">
            <WarningOutline />
            <span>{{ t('dms.deliveryWarning') }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 操作面板 -->
    <div class="grid grid-2 mb-32">
      <div class="card">
        <div class="card-title mb-16">{{ t('dms.checkInTitle') }}</div>
        <button class="btn btn-success btn-lg" @click="handleCheckIn" :disabled="checking">
          {{ checking ? t('dms.checking') : t('dms.checkInBtn') }}
        </button>
        <div v-if="lastCheckIn" class="mt-16 text-muted" style="font-size: 13px;">{{ t('dms.lastCheckIn') }}：{{ lastCheckIn }}</div>
        <!-- 签到历史：让「我上次到底点过没有」可自查，而不是只有一行最新时间 -->
        <div v-if="recentCheckIns.length > 1" class="mt-12">
          <div class="text-muted" style="font-size: 12px; margin-bottom: 4px;">{{ t('dms.recentCheckIns') }}</div>
          <ul class="dms-history-list">
            <li v-for="(ts, i) in recentCheckIns.slice(1)" :key="i">{{ ts }}</li>
          </ul>
        </div>
      </div>

      <div class="card">
        <div class="card-title mb-16">{{ t('dms.settingsTitle') }}</div>
        <div class="form-group">
          <label class="flex items-center gap-8" style="cursor: pointer;">
            <input type="checkbox" id="dms-enabled" v-model="settings.enabled" style="width: 18px; height: 18px;" />
            <span style="font-weight: 500; font-size: 14px;">{{ t('dms.enable') }}</span>
          </label>
          <p class="form-hint">{{ t('dms.disableHint') }}</p>
        </div>
        <div class="form-group">
          <label class="form-label" for="dms-interval">{{ t('dms.intervalLabel') }}</label>
          <select class="form-select" id="dms-interval" v-model.number="settings.interval" :disabled="!settings.enabled">
            <option v-for="opt in INTERVAL_OPTIONS" :key="opt.value" :value="opt.value">{{ t(opt.labelKey) }}</option>
          </select>
        </div>
        <button class="btn btn-primary btn-sm mt-16" @click="saveSettings">{{ t('dms.saveSettings') }}</button>
      </div>
    </div>

    <!-- 本地提醒 -->
    <div class="card mb-32">
      <div class="card-title mb-16">{{ t('dms.localReminders') }}</div>
      <ul class="dms-note-list">
        <li>{{ t('dms.reminder1a') }} <strong>{{ t('dms.reminder1b') }}</strong> {{ t('dms.reminder1c') }}</li>
        <li>{{ t('dms.reminder2a') }}<strong>{{ t('dms.reminder2b') }}</strong>{{ t('dms.reminder2c') }}</li>
        <li>{{ t('dms.reminder3a') }}<strong>{{ t('dms.reminder3b') }}</strong>{{ t('dms.reminder3c') }}</li>
      </ul>
      <div style="display: flex; align-items: center; gap: 12px; margin-top: 14px; flex-wrap: wrap;">
        <button class="btn btn-secondary btn-sm" :disabled="testingNotify" @click="handleTestNotify">{{ testingNotify ? t('dms.sending') : t('dms.testNotify') }}</button>
        <span v-if="testResult" style="font-size: 12px;" aria-live="polite" :style="{ color: testResult === 'ok' ? 'var(--success)' : 'var(--danger)' }">
          {{ testResult === 'ok' ? t('dms.testOk') : t('dms.testFail') }}
        </span>
      </div>
    </div>

    <!-- 承诺边界：写在最前面，避免用户以为「开了就一定会通知到家人」 -->
    <div class="card mb-32 dms-limits" role="note">
      <div class="card-title mb-12">{{ t('dms.limitsTitle') }}</div>
      <ul class="dms-note-list">
        <li>{{ t('dms.limits1') }}</li>
        <li><strong>{{ t('dms.channelLabel') }}：{{ deliveryChannel }}</strong>——{{ t('dms.limits2') }}</li>
        <li>{{ t('dms.limits3') }}</li>
        <li>{{ t('dms.limits4') }}</li>
      </ul>
    </div>

    <!-- 三级确认说明 -->
    <div class="card">
      <div class="card-title mb-16">{{ t('dms.stagesTitle') }}</div>
      <div class="dms-stages">
        <div class="dms-stage" :class="{ 'dms-stage-active': dmsEvaluation?.current_stage === 1 }">
          <span class="dms-stage-num">1</span>
          <div><strong>{{ t('dms.stage1Title') }}</strong><p class="text-muted">{{ t('dms.stage1Desc') }}</p></div>
        </div>
        <div class="dms-stage" :class="{ 'dms-stage-active': dmsEvaluation?.current_stage === 2 }">
          <span class="dms-stage-num">2</span>
          <div><strong>{{ t('dms.stage2Title') }}</strong><p class="text-muted">{{ t('dms.stage2Desc') }}</p></div>
        </div>
        <div class="dms-stage" :class="{ 'dms-stage-active': dmsEvaluation?.current_stage === 3 }">
          <span class="dms-stage-num">3</span>
          <div><strong>{{ t('dms.stage3Title') }}</strong><p class="text-muted">{{ t('dms.stage3Desc') }}</p></div>
        </div>
      </div>
    </div>

    <!-- 通知记录：读本地发件箱历史，而不是「本轮评估结果」。
         旧实现只显示当前这一轮，页面刷新即清空，用户无法确认过去是否真的发出过通知。 -->
    <div class="card" v-if="dmsHistory.length > 0">
      <div class="card-title mb-16">{{ t('dms.logTitle') }}</div>
      <div v-for="(n, i) in dmsHistory" :key="i" class="card mb-16" style="padding: 12px 16px;">
        <div class="flex items-center gap-12">
          <span class="tag" :class="n.stage >= 3 ? 'tag-danger' : n.stage >= 2 ? 'tag-warning' : 'tag-property'">{{ t('dms.stageTag', { stage: n.stage }) }}</span>
          <div style="flex: 1;">
            <div style="font-weight: 600;">{{ n.subject }}</div>
            <div class="text-muted" style="font-size: 12px;">{{ t('dms.recipient') }}：{{ n.recipient || t('dms.notConfigured') }} · {{ t('dms.channel') }}：{{ n.channel }} · {{ n.sent_at }}</div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useI18n } from '../i18n';
import AlertCircleOutline from '@vicons/ionicons5/es/AlertCircleOutline';
import CheckmarkCircleOutline from '@vicons/ionicons5/es/CheckmarkCircleOutline';
import RemoveCircleOutline from '@vicons/ionicons5/es/RemoveCircleOutline';
import WarningOutline from '@vicons/ionicons5/es/WarningOutline';
import { useDashboardStore } from '../stores/dashboard';
import { sendTestNotification } from '../api/reminders';
import { toast } from '../composables/feedback';
import { INTERVAL_OPTIONS } from '../constants/dms';

const { t } = useI18n();
const dashboard = useDashboardStore();
const checking = ref(false);
const settings = ref({ interval: 30, enabled: false });
const testingNotify = ref(false);
const testResult = ref<'' | 'ok' | 'fail'>('');

async function handleTestNotify() {
  testingNotify.value = true;
  try {
    const ok = await sendTestNotification();
    testResult.value = ok ? 'ok' : 'fail';
  } finally {
    testingNotify.value = false;
  }
}

const status = computed(() => dashboard.checkInStatus);
const lastCheckIn = computed(() => status.value?.last_check_in);
const daysUntilDue = computed(() => status.value?.days_until_due);
const recentCheckIns = computed(() => status.value?.recent_check_ins ?? []);
const dmsEvaluation = computed(() => dashboard.dmsStatus);
const dmsHistory = computed(() => dashboard.dmsHistory);

/** 实际投递通道：界面必须告诉用户「家人到底会不会收到邮件」，
 *  而不是让人以为开了 DMS 就等于通知到手。 */
const deliveryChannel = computed(() => {
  // 心跳启用 = 至少有一个「应用关闭后也能被发现失联」的通道
  if (dashboard.heartbeatConfigured) return t('dms.channelHeartbeat');
  return t('dms.channelLocal');
});

/** 家人是否真的能收到通知：只有配置了 SMTP 才算。
 *  与后端 `trigger_ready = dms_enabled && trigger_channel == "smtp"` 同一口径。 */
const deliveryReady = computed(() => Boolean(dashboard.stats?.trigger_ready));

const dmsState = computed(() => {
  if (!status.value?.dms_enabled) return 'disabled';
  if ((daysUntilDue.value ?? 999) <= 0) return 'overdue';
  if ((daysUntilDue.value ?? 999) <= 3) return 'warning';
  return 'ok';
});

const statusIcon = computed(() => ({ ok: CheckmarkCircleOutline, warning: WarningOutline, overdue: AlertCircleOutline, disabled: RemoveCircleOutline }[dmsState.value]));
const statusTitle = computed(() => ({
  ok: t('dms.stateOk'), warning: t('dms.stateWarning'), overdue: t('dms.stateOverdue'), disabled: t('dms.stateDisabled'),
}[dmsState.value]));
const statusSubtitle = computed(() => {
  const days = daysUntilDue.value;
  if (dmsState.value === 'ok') {
    return days === null || days === undefined
      ? t('dms.stateOkSub')
      : t('dms.daysLeft', { days });
  }
  if (dmsState.value === 'warning') {
    return days === null || days === undefined
      ? t('dms.confirmSoon')
      : t('dms.daysLeftUrgent', { days });
  }
  return {
    overdue: t('dms.stateOverdueSub'),
    disabled: t('dms.stateDisabledSub'),
  }[dmsState.value] || '';
});
const statusColor = computed(() => ({
  ok: 'var(--success-strong)', warning: 'var(--warning-strong)', overdue: 'var(--danger-strong)', disabled: 'var(--gray-600)',
}[dmsState.value]));
const statusClass = computed(() => ({
  ok: 'dms-ok', warning: 'dms-warning', overdue: 'dms-overdue', disabled: 'dms-disabled',
}[dmsState.value]));

async function handleCheckIn() {
  checking.value = true;
  try {
    await dashboard.checkIn();
  } finally {
    checking.value = false;
  }
}

async function saveSettings() {
  try {
    await dashboard.configureDms(settings.value.enabled, settings.value.interval);
    toast(t('dms.saved'));
  } catch (e) {
    toast(t('dms.saveFailed', { e: String(e) }), 'error');
  }
}

onMounted(async () => {
  await Promise.all([
    dashboard.loadCheckInStatus(),
    dashboard.evaluateDms(),
    dashboard.loadDmsHistory(),
    dashboard.loadHeartbeatState(),
  ]);
  if (status.value) {
    settings.value.interval = status.value.interval_days;
    settings.value.enabled = status.value.dms_enabled;
  }
});
</script>

<style scoped>
.dms-delivery-warn {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-top: 12px;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid var(--warning);
  background: var(--warning-light);
  color: var(--warning-strong);
  font-size: 13px;
  line-height: 1.55;
}
.dms-delivery-warn svg {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  margin-top: 2px;
}
.dms-ok { background: var(--success-light); border-color: var(--success); }
.dms-warning { background: var(--warning-light); border-color: var(--warning); }
.dms-overdue { background: var(--danger-light); border-color: var(--danger); }
.dms-disabled { background: var(--gray-50); border-color: var(--gray-200); }
.dms-stages { display: flex; flex-direction: column; gap: 16px; }
.dms-stage { display: flex; gap: 12px; padding: 12px 16px; border-radius: 8px; }
.dms-stage p { font-size: 13px; margin-top: 2px; }
.dms-note-list { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 6px; font-size: 13px; color: var(--gray-600, #5c6f79); line-height: 1.65; }
.dms-note-list strong { color: var(--gray-800, #37464f); }
.dms-stage-1 { background: var(--success-light); }
.dms-stage-2 { background: var(--warning-light); }
.dms-stage-3 { background: var(--danger-light); }
.dms-stage-active { outline: 2px solid var(--primary); outline-offset: 2px; }
.dms-stage-num { font-size: 24px; }
.dms-limits { background: var(--gray-50); border-color: var(--gray-300); }
.dms-limits .card-title { color: var(--gray-900); }
.dms-history-list { margin: 0; padding-left: 18px; font-size: 12px; color: var(--gray-600); line-height: 1.7; font-variant-numeric: tabular-nums; }
</style>
