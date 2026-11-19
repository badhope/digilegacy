import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import type { Mock } from 'vitest';

// dashboard store 承载「统计 / 签到 / DMS 评估 / 档案」四条真实数据链，
// 这里锁定：失败不污染既有状态、写路径失败向上抛、refreshAll 的 loading 必然复位。
vi.mock('../api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => false,
}));

import { invoke } from '../api/invoke';
import { useDashboardStore } from './dashboard';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();

const mockedInvoke = invoke as unknown as Mock;

function ok<T>(data: T) {
  return { success: true, message: 'ok', data };
}

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
});

describe('只读加载：失败不覆盖既有数据', () => {
  it('loadStats 失败时保留旧值', async () => {
    const s = useDashboardStore();
    s.stats = { total_assets: 7 } as never;

    mockedInvoke.mockResolvedValueOnce({ success: false, message: 'db closed', data: null });
    await s.loadStats();

    expect(s.stats).toEqual({ total_assets: 7 });
  });

  it('loadStats 抛异常（catch 分支）时保留旧值', async () => {
    // 上面那条测的是 success:false（不进 catch）；这里覆盖真正的异常路径，
    // 否则在 catch 里写 stats.value = null 不会被任何用例发现（变异实测存活）。
    const s = useDashboardStore();
    s.stats = { total_assets: 5 } as never;

    mockedInvoke.mockRejectedValueOnce(new Error('IPC 断开'));
    await s.loadStats();

    expect(s.stats).toEqual({ total_assets: 5 });
  });

  it('loadCheckInStatus 失败时保留旧值', async () => {
    const s = useDashboardStore();
    s.checkInStatus = { interval_days: 30 } as never;

    mockedInvoke.mockRejectedValueOnce(new Error('IPC 断开'));
    await s.loadCheckInStatus();

    expect(s.checkInStatus).toEqual({ interval_days: 30 });
  });

  it('loadUser 遇 data:null 时置空而不是保留脏数据', async () => {
    const s = useDashboardStore();
    s.user = { full_name: '旧值' } as never;

    mockedInvoke.mockResolvedValueOnce(ok(null));
    await s.loadUser();

    expect(s.user).toBeNull();
  });

  it('evaluateDms 失败时保留上次评估结果', async () => {
    const s = useDashboardStore();
    s.dmsEvaluation = { enabled: true, current_stage: 1 } as never;

    mockedInvoke.mockResolvedValueOnce({ success: false, message: 'x', data: null });
    await s.evaluateDms();

    expect(s.dmsEvaluation).toEqual({ enabled: true, current_stage: 1 });
  });

  it('evaluateDms 走只读命令，不调用有副作用的 evaluate_dms', async () => {
    const s = useDashboardStore();
    mockedInvoke.mockResolvedValueOnce(ok({ enabled: true, current_stage: 2 }));

    await s.evaluateDms();

    expect(mockedInvoke).toHaveBeenCalledWith('get_dms_status');
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'evaluate_dms')).toBe(false);
    expect(s.dmsStatus).toEqual({ enabled: true, current_stage: 2 });
  });

  it('loadDmsHistory 写入历史列表', async () => {
    const s = useDashboardStore();
    mockedInvoke.mockResolvedValueOnce(ok([{ stage: 1, recipient: 'a@x.com' }]));

    await s.loadDmsHistory();

    expect(s.dmsHistory).toHaveLength(1);
  });
});

describe('签到', () => {
  it('成功后串行刷新统计与状态（三项都被触发）', async () => {
    const s = useDashboardStore();
    mockedInvoke
      .mockResolvedValueOnce(ok('已签到'))
      .mockResolvedValueOnce(ok({ total_assets: 1 }))
      .mockResolvedValueOnce(ok({ interval_days: 30 }))
      .mockResolvedValueOnce(ok({ enabled: true, current_stage: 0 }));

    const res = await s.checkIn();

    expect(res.success).toBe(true);
    expect(s.stats).toEqual({ total_assets: 1 });
    expect(s.checkInStatus).toEqual({ interval_days: 30 });
    // 必须点名三个具体的刷新命令：只断言调用次数的话，
    // 把 evaluateDms 换成 loadUser 也照样通过（变异实测存活）。
    const cmds = mockedInvoke.mock.calls.map(c => c[0]);
    expect(cmds).toEqual(['update_check_in', 'get_dashboard_stats', 'get_check_in_status', 'get_dms_status']);
  });

  it('失败时向上抛错（不静默吞掉签到失败）', async () => {
    const s = useDashboardStore();
    mockedInvoke.mockRejectedValueOnce(new Error('锁定状态'));

    await expect(s.checkIn()).rejects.toThrow('锁定状态');
  });
});

describe('写路径', () => {
  it('saveUser 成功后合并进本地 user', async () => {
    const s = useDashboardStore();
    s.user = { full_name: '旧名', phone: '1' } as never;
    mockedInvoke.mockResolvedValueOnce(ok(null));

    await s.saveUser({ full_name: '新名', phone: '1', email: 'a@b.com' } as never);

    expect((s.user as Record<string, unknown>).full_name).toBe('新名');
    expect((s.user as Record<string, unknown>).email).toBe('a@b.com');
  });

  it('saveUser 失败时向上抛错', async () => {
    const s = useDashboardStore();
    mockedInvoke.mockResolvedValueOnce({ success: false, message: '主密码已设置但处于锁定状态', data: null });

    await expect(s.saveUser({ full_name: 'x' } as never)).rejects.toThrow('锁定状态');
  });

  it('configureDms 成功后重载签到状态', async () => {
    const s = useDashboardStore();
    mockedInvoke
      .mockResolvedValueOnce(ok(null))
      .mockResolvedValueOnce(ok({ interval_days: 14 }));

    await s.configureDms(true, 14);

    expect(s.checkInStatus).toEqual({ interval_days: 14 });
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, 'set_dead_mans_switch', { enabled: true, intervalDays: 14 });
  });
});

describe('refreshAll', () => {
  it('四项并行加载后 loading 复位', async () => {
    const s = useDashboardStore();
    mockedInvoke.mockResolvedValue(ok(null));

    await s.refreshAll();

    expect(s.loading).toBe(false);
    expect(mockedInvoke).toHaveBeenCalledTimes(4);
  });

  it('即使某项抛错 loading 也必须复位', async () => {
    const s = useDashboardStore();
    mockedInvoke.mockRejectedValue(new Error('boom'));

    await s.refreshAll();

    expect(s.loading).toBe(false);
  });
});
