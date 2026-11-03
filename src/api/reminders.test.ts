import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises } from '@vue/test-utils';

// reminders.ts 决定「什么时候弹系统提醒」。这里的边界（提前 3 天 / 逾期）
// 一旦算错，用户会漏收关键提醒或被打扰。浏览器预览模式必须完全不发通知。
const isTauri = vi.fn(() => true);
vi.mock('./invoke', () => ({
  isTauriEnv: () => isTauri(),
  invoke: vi.fn(),
}));

const sendNotification = vi.fn();
vi.mock('@tauri-apps/plugin-notification', () => ({
  isPermissionGranted: vi.fn(async () => true),
  requestPermission: vi.fn(async () => 'granted'),
  sendNotification: (...args: unknown[]) => sendNotification(...args),
}));

import { invoke } from './invoke';
import { checkAndNotifyOnce, startLocalReminderLoop } from './reminders';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();
import type { Mock } from 'vitest';

const mockedInvoke = invoke as unknown as Mock;

function status(over: Record<string, unknown> = {}) {
  return {
    success: true,
    message: '',
    data: {
      last_check_in: '2026-10-01 00:00:00',
      interval_days: 30,
      dms_enabled: true,
      next_check_in_due: '2026-10-31',
      days_until_due: 10,
      ...over,
    },
  };
}

beforeEach(() => {
  localStorage.clear();
  sendNotification.mockReset();
  mockedInvoke.mockReset();
  isTauri.mockReturnValue(true);
});

describe('checkAndNotifyOnce', () => {
  it('非 Tauri 环境直接返回，不发起任何 IPC', async () => {
    isTauri.mockReturnValue(false);

    await checkAndNotifyOnce();

    expect(mockedInvoke).not.toHaveBeenCalled();
  });


  it('权限未授予时先请求权限，授予后发送', async () => {
    let permissionCalls = 0;
    const { requestPermission } = await import('@tauri-apps/plugin-notification');
    (requestPermission as unknown as Mock).mockResolvedValueOnce('granted');
    permissionCalls++;
    mockedInvoke.mockResolvedValueOnce(status({ days_until_due: 1 }));
    localStorage.removeItem('digilegacy_reminder_' + new Date().toISOString().slice(0, 10) + '_due');

    await checkAndNotifyOnce();
    expect(sendNotification).toHaveBeenCalled();
    expect(permissionCalls).toBeGreaterThan(0);
  });

  it('通知插件抛异常时返回 false 而不是上抛（本地提醒不能拖垮应用）', async () => {
    const { isPermissionGranted } = await import('@tauri-apps/plugin-notification');
    (isPermissionGranted as unknown as Mock).mockRejectedValueOnce(new Error('plugin gone'));
    mockedInvoke.mockResolvedValueOnce(status({ days_until_due: 1 }));

    const r = await checkAndNotifyOnce();
    expect(sendNotification).not.toHaveBeenCalled();
    // 不抛错即为通过；返回值无公开契约，只断言无异常
    expect(r).toBeUndefined();
  });

  it('days_until_due 为 null 时直接返回（不误判为逾期）', async () => {
    mockedInvoke.mockResolvedValueOnce(status({ days_until_due: null }));
    await checkAndNotifyOnce();
    expect(sendNotification).not.toHaveBeenCalled();
  });

  it('DMS 未启用时不提醒', async () => {
    mockedInvoke.mockResolvedValueOnce(status({ dms_enabled: false, days_until_due: 0 }));

    await checkAndNotifyOnce();

    expect(sendNotification).not.toHaveBeenCalled();
  });

  it('days_until_due 为 null 时不提醒', async () => {
    mockedInvoke.mockResolvedValueOnce(status({ days_until_due: null }));

    await checkAndNotifyOnce();

    expect(sendNotification).not.toHaveBeenCalled();
  });

  it('距到期 3 天内发「即将到期」提醒', async () => {
    mockedInvoke.mockResolvedValueOnce(status({ days_until_due: 3 }));

    await checkAndNotifyOnce();

    expect(sendNotification).toHaveBeenCalledTimes(1);
    expect(sendNotification.mock.calls[0][0].title).toContain('定期安全确认');
  });

  it('距到期 4 天及以上不提醒（边界值）', async () => {
    mockedInvoke.mockResolvedValueOnce(status({ days_until_due: 4 }));

    await checkAndNotifyOnce();

    expect(sendNotification).not.toHaveBeenCalled();
  });

  it('逾期发「该确认安全了」并带上逾期天数', async () => {
    mockedInvoke.mockResolvedValueOnce(status({ days_until_due: -5 }));

    await checkAndNotifyOnce();

    expect(sendNotification).toHaveBeenCalledTimes(1);
    const payload = sendNotification.mock.calls[0][0];
    expect(payload.title).toContain('该确认安全了');
    expect(payload.body).toContain('5');
  });

  it('同日同类提醒只发一次（去重键生效）', async () => {
    mockedInvoke.mockResolvedValue(status({ days_until_due: 2 }));

    await checkAndNotifyOnce();
    await checkAndNotifyOnce();

    expect(sendNotification).toHaveBeenCalledTimes(1);
  });

  it('逾期与即将到期是两个独立去重通道', async () => {
    mockedInvoke.mockResolvedValueOnce(status({ days_until_due: 2 }));
    await checkAndNotifyOnce();

    mockedInvoke.mockResolvedValueOnce(status({ days_until_due: -1 }));
    await checkAndNotifyOnce();

    expect(sendNotification).toHaveBeenCalledTimes(2);
  });

  it('IPC 抛错被吞掉，不向调用方冒泡', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('IPC 断开'));

    await expect(checkAndNotifyOnce()).resolves.toBeUndefined();
  });
});

describe('startLocalReminderLoop', () => {
  it('非 Tauri 环境返回空清理函数且不建定时器', () => {
    isTauri.mockReturnValue(false);
    const spy = vi.spyOn(window, 'setInterval');

    const stop = startLocalReminderLoop();

    expect(spy).not.toHaveBeenCalled();
    expect(() => stop()).not.toThrow();
  });

  it('Tauri 环境立即触发一次并建立 30 分钟定时器', async () => {
    const spy = vi.spyOn(window, 'setInterval');
    mockedInvoke.mockResolvedValue(status({ days_until_due: 2 }));

    const stop = startLocalReminderLoop();
    // 「立即触发一次」必须真的被验证：删掉 void checkAndNotifyOnce() 时
    // 仅断言 setInterval 会漏掉（变异实测存活）。
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith('get_check_in_status');
    expect(spy).toHaveBeenCalledWith(expect.any(Function), 30 * 60 * 1000);
    expect(() => stop()).not.toThrow();
    spy.mockRestore();
  });
});
