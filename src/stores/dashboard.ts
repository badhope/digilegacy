import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '../api/invoke';
import { t } from '../i18n';

import type { DashboardStats, CheckInStatus, UserProfile, ApiResponse, DmsStatus, DmsNotification } from '../types';

export const useDashboardStore = defineStore('dashboard', () => {
  const stats = ref<DashboardStats | null>(null);
  const checkInStatus = ref<CheckInStatus | null>(null);
  const dmsStatus = ref<DmsStatus | null>(null);
  const dmsHistory = ref<DmsNotification[]>([]);
  /** @deprecated 仅保留给旧调用点；新代码请用 dmsStatus + dmsHistory。 */
  const dmsEvaluation = ref<DmsStatus | null>(null);
  const user = ref<UserProfile | null>(null);
  /** 心跳是否已配置（界面据此说明「失联后是否还有人会发现」）。 */
  const heartbeatConfigured = ref(false);
  const loading = ref(false);

  async function loadStats() {
    try {
      const res = await invoke<ApiResponse<DashboardStats>>('get_dashboard_stats');
      if (res.success && res.data) {
        stats.value = res.data;
      }
    } catch (e) {
      console.error('加载统计数据失败:', e);
    }
  }

  async function loadCheckInStatus() {
    try {
      const res = await invoke<ApiResponse<CheckInStatus>>('get_check_in_status');
      if (res.success && res.data) {
        checkInStatus.value = res.data;
      }
    } catch (e) {
      console.error('加载签到状态失败:', e);
    }
  }

  /** 只读状态：打开界面**不得**触发 SMTP 投递（后端 evaluate_dms 有副作用）。 */
  async function evaluateDms() {
    try {
      const res = await invoke<ApiResponse<DmsStatus>>('get_dms_status');
      if (res.success) {
        const data = res.data || null;
        dmsStatus.value = data;
        dmsEvaluation.value = data;
      }
    } catch (e) {
      console.error('读取 DMS 状态失败:', e);
    }
  }

  /** 心跳配置只暴露布尔：令牌永不出后端（见 commands::get_heartbeat_config）。 */
  async function loadHeartbeatState() {
    try {
      const res = await invoke<ApiResponse<{ enabled: boolean; has_token: boolean; endpoint: string }>>(
        'get_heartbeat_config',
      );
      heartbeatConfigured.value = Boolean(res.success && res.data?.enabled && res.data?.endpoint);
    } catch (e) {
      console.error('读取心跳状态失败:', e);
    }
  }

  async function loadDmsHistory() {
    try {
      const res = await invoke<ApiResponse<DmsNotification[]>>('get_dms_history');
      if (res.success && res.data) {
        dmsHistory.value = res.data;
      }
    } catch (e) {
      console.error('读取通知历史失败:', e);
    }
  }

  async function loadUser() {
    try {
      const res = await invoke<ApiResponse<UserProfile | null>>('get_user_profile');
      if (res.success) {
        user.value = res.data || null;
      }
    } catch (e) {
      console.error('加载用户档案失败:', e);
    }
  }

  async function checkIn() {
    try {
      const res = await invoke<ApiResponse<string>>('update_check_in');
      if (res.success) {
        await Promise.all([loadStats(), loadCheckInStatus(), evaluateDms()]);
      }
      return res;
    } catch (e) {
      console.error('签到失败:', e);
      throw e;
    }
  }

  async function saveUser(profile: UserProfile) {
    try {
      const res = await invoke<ApiResponse<void>>('set_user_profile', { profile });
      if (!res.success) {
        // 写路径失败必须以异常上抛：ProfileView 依赖它来提示失败，
        // 否则锁定态拒写会被 UI 报成「保存成功」。
        throw new Error(res.message || t('err.saveProfile'));
      }
      user.value = { ...user.value, ...profile };
      return res;
    } catch (e) {
      console.error('保存用户档案失败:', e);
      throw e;
    }
  }

  async function configureDms(enabled: boolean, intervalDays: number) {
    try {
      const res = await invoke<ApiResponse<void>>('set_dead_mans_switch', {
        enabled,
        intervalDays,
      });
      if (!res.success) {
        throw new Error(res.message || t('err.saveSettings'));
      }
      await loadCheckInStatus();
      return res;
    } catch (e) {
      console.error('配置 DMS 失败:', e);
      throw e;
    }
  }

  async function refreshAll() {
    loading.value = true;
    try {
      await Promise.all([loadStats(), loadCheckInStatus(), evaluateDms(), loadUser()]);
    } finally {
      loading.value = false;
    }
  }

  return {
    stats,
    checkInStatus,
    dmsStatus,
    dmsHistory,
    dmsEvaluation,
    heartbeatConfigured,
    user,
    loading,
    loadStats,
    loadCheckInStatus,
    evaluateDms,
    loadDmsHistory,
    loadHeartbeatState,
    loadUser,
    checkIn,
    saveUser,
    configureDms,
    refreshAll,
  };
});
