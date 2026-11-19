// 本地到期提醒：应用运行期间定时评估签到状态，到期/逾期发送系统通知（按日去重）
// 仅 Tauri 桌面环境生效；浏览器预览模式不发送系统通知。

import { invoke, isTauriEnv } from './invoke';
import type { ApiResponse, CheckInStatus } from '../types';

const LS_PREFIX = 'digilegacy_reminder_';
const LOOP_MS = 30 * 60 * 1000;

function todayKey(level: string): string {
  return `${LS_PREFIX}${new Date().toISOString().slice(0, 10)}_${level}`;
}

async function sendSystemNotification(title: string, body: string): Promise<boolean> {
  try {
    const mod = await import('@tauri-apps/plugin-notification');
    let granted = await mod.isPermissionGranted();
    if (!granted) {
      granted = (await mod.requestPermission()) === 'granted';
    }
    if (!granted) return false;
    mod.sendNotification({ title, body });
    return true;
  } catch (e) {
    console.error('系统通知发送失败:', e);
    return false;
  }
}

export async function checkAndNotifyOnce(): Promise<void> {
  if (!isTauriEnv()) return;
  try {
    const res = await invoke<ApiResponse<CheckInStatus>>('get_check_in_status');
    const s = res.data;
    if (!res.success || !s || !s.dms_enabled) return;
    const days = s.days_until_due;
    if (days === null || days === undefined) return;

    if (days < 0) {
      const key = todayKey('overdue');
      if (!localStorage.getItem(key)) {
        const ok = await sendSystemNotification(
          '归零 · 该确认安全了',
          `已超过约定检查时间 ${Math.abs(days)} 天。请打开归零，点击「我还安好」完成确认。`,
        );
        if (ok) localStorage.setItem(key, '1');
      }
    } else if (days <= 3) {
      const key = todayKey('due');
      if (!localStorage.getItem(key)) {
        const ok = await sendSystemNotification(
          '归零 · 定期安全确认',
          `距离下次安全确认还有 ${days} 天，记得打开归零点一下「我还安好」。`,
        );
        if (ok) localStorage.setItem(key, '1');
      }
    }
  } catch (e) {
    console.error('本地提醒检查失败:', e);
  }
}

export function startLocalReminderLoop(): () => void {
  if (!isTauriEnv()) {
    return () => {};
  }
  void checkAndNotifyOnce();
  const timer = window.setInterval(checkAndNotifyOnce, LOOP_MS);
  return () => window.clearInterval(timer);
}

export async function sendTestNotification(): Promise<boolean> {
  return sendSystemNotification(
    '归零 · 测试提醒',
    '如果你看到了这条系统通知，说明本地提醒工作正常。正式提醒会在安全确认到期前 3 天与你见面。',
  );
}
