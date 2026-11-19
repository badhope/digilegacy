import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '../api/invoke';

import type { ApiResponse, SecurityStatus } from '../types';

export const useSecurityStore = defineStore('security', () => {
  const hasMasterPassword = ref(false);
  const unlocked = ref(true);
  const busy = ref(false);

  async function refresh() {
    try {
      const res = await invoke<ApiResponse<SecurityStatus>>('security_status');
      if (res.success && res.data) {
        hasMasterPassword.value = res.data.has_master_password;
        unlocked.value = res.data.has_master_password ? res.data.unlocked : true;
      }
    } catch (e) {
      console.error('读取安全状态失败:', e);
    }
  }

  async function setMasterPassword(password: string): Promise<{ ok: boolean; message: string; recoveryCode: string }> {
    busy.value = true;
    try {
      const res = await invoke<ApiResponse<{ status: SecurityStatus; recovery_code: string }>>('set_master_password', { password });
      if (res.success && res.data) {
        hasMasterPassword.value = true;
        unlocked.value = true;
        // 恢复密钥只在此响应中出现一次，必须原样交给 UI 强制展示
        return { ok: true, message: res.message, recoveryCode: res.data.recovery_code };
      }
      return { ok: false, message: res.message, recoveryCode: '' };
    } catch (e) {
      return { ok: false, message: String(e), recoveryCode: '' };
    } finally {
      busy.value = false;
    }
  }

  async function unlockWith(password: string): Promise<string> {
    busy.value = true;
    try {
      const res = await invoke<ApiResponse<SecurityStatus>>('unlock', { password });
      if (res.success) {
        unlocked.value = true;
        return '';
      }
      return res.message;
    } catch (e) {
      return String(e);
    } finally {
      busy.value = false;
    }
  }

  async function lockNow() {
    try {
      await invoke<ApiResponse<SecurityStatus>>('lock');
    } catch (e) {
      console.error('锁定失败:', e);
    }
    unlocked.value = false;
  }

  return { hasMasterPassword, unlocked, busy, refresh, setMasterPassword, unlockWith, lockNow };
});
