import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mockInvoke } from './mock';

// mock.ts 是 Web 预览模式的唯一后端（485 行真实分支逻辑），
// 测试定位为「分支回归网」：jsdom 环境提供 localStorage/sessionStorage。

const LS_KEY = 'digilegacy-web-preview-v1';
const SS_UNLOCK = 'digilegacy-web-preview-unlocked';

const sampleReq = {
  platform_id: 1,
  account_identifier: 'user-1',
  account_description: 'desc',
  category: 'property',
  estimated_value: 100,
  disposal_method: 'inherit',
  heir_name: '张三',
  heir_contact: '13800000000',
  notes: 'n',
};

beforeEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  vi.restoreAllMocks();
});

describe('资产增改', () => {
  it('add_asset 缺省回退、新 id 置顶并持久化', async () => {
    const added = await mockInvoke<{ data: number }>('add_asset', { req: { platform_id: 1, category: 'property' } });
    const id = added.data;
    expect(typeof id).toBe('number');

    const listRes = await mockInvoke<{ data: Array<Record<string, unknown>> }>('get_all_assets');
    const list = listRes.data;
    expect(list[0].id).toBe(id);
    expect(list[0].disposal_method).toBe('no_action');
    expect(list[0].heir_name).toBeNull();

    const raw = JSON.parse(localStorage.getItem(LS_KEY) || '{}') as { nextAssetId: number };
    expect(raw.nextAssetId).toBe(id + 1);
  });

  it('非法类别/处置/负金额与 Rust 命令层同样被拒绝', async () => {
    const invalid = [
      { req: { platform_id: 1, category: 'nope', disposal_method: 'inherit' } },
      { req: { platform_id: 1, category: 'property', disposal_method: 'sell' } },
      { req: { platform_id: 1, category: 'property', disposal_method: 'inherit', estimated_value: -1 } },
      { req: { platform_id: 1, category: 'property', disposal_method: 'inherit', estimated_value: Number.POSITIVE_INFINITY } },
      { req: { platform_id: 1, category: 'property', disposal_method: 'inherit', estimated_value: 1e15 } },
    ];
    for (const item of invalid) {
      const res = await mockInvoke<{ success: boolean; message: string }>('add_asset', item);
      expect(res.success, JSON.stringify(item)).toBe(false);
      expect(res.message.length).toBeGreaterThan(0);
    }
    // 合法值仍应成功（防止校验过严）
    const good = await mockInvoke<{ success: boolean }>('add_asset', {
      req: { platform_id: 1, category: 'property', disposal_method: 'inherit', heir_name: '继承人', estimated_value: 0 },
    });
    expect(good.success).toBe(true);
  });

  it('update_asset 不存在的 id 返回失败', async () => {
    const res = await mockInvoke<{ success: boolean; message: string }>('update_asset', { id: 99999, req: sampleReq });
    expect(res.success).toBe(false);
    expect(res.message).toBe('资产不存在或已被删除');
  });
});

describe('签到状态', () => {
  it('未签到时 next_due/days_until_due 均为 null', async () => {
    const res = await mockInvoke<{ data: { next_check_in_due: string | null; days_until_due: number | null } }>('get_check_in_status');
    expect(res.data.next_check_in_due).toBeNull();
    expect(res.data.days_until_due).toBeNull();
  });

  it('签到后计算下次到期', async () => {
    await mockInvoke('update_check_in');
    const res = await mockInvoke<{ data: { last_check_in: string | null; next_check_in_due: string | null; days_until_due: number | null } }>('get_check_in_status');
    expect(res.data.last_check_in).toBeTruthy();
    expect(res.data.next_check_in_due).toBeTruthy();
    expect(res.data.days_until_due).toBeGreaterThan(0);
  });
});

describe('主密码生命周期', () => {
  it('短密码被拒绝', async () => {
    const res = await mockInvoke<{ success: boolean }>('set_master_password', { password: 'short' });
    expect(res.success).toBe(false);
  });

  it('重复设置拒绝；解锁成败分支；锁定清除解锁态', async () => {
    const enabled = await mockInvoke<{ success: boolean; message: string }>('set_master_password', { password: 'master-pass-1' });
    expect(enabled.message).toContain('演示模式未加密/仅用于展示');

    const dup = await mockInvoke<{ success: boolean; message: string }>('set_master_password', { password: 'master-pass-2' });
    expect(dup.success).toBe(false);

    const wrong = await mockInvoke<{ success: boolean; message: string }>('unlock', { password: 'nope' });
    expect(wrong.success).toBe(false);
    expect(wrong.message).toBe('主密码错误');

    const good = await mockInvoke<{ success: boolean }>('unlock', { password: 'master-pass-1' });
    expect(good.success).toBe(true);
    expect(sessionStorage.getItem(SS_UNLOCK)).toBe('1');

    const locked = await mockInvoke<{ data: { unlocked: boolean } }>('lock');
    expect(locked.data.unlocked).toBe(false);
    expect(sessionStorage.getItem(SS_UNLOCK)).toBeNull();
  });
});

describe('演示模式安全与档案契约', () => {
  it('set_user_profile 按 profile 参数保存并可完整读回', async () => {
    const current = await mockInvoke<{ data: Record<string, unknown> }>('get_user_profile');
    const profile = { ...current.data, full_name: '回读测试用户' };

    const saved = await mockInvoke<{ success: boolean }>('set_user_profile', { profile });
    expect(saved.success).toBe(true);

    const loaded = await mockInvoke<{ data: Record<string, unknown> }>('get_user_profile');
    expect(loaded.data.full_name).toBe('回读测试用户');
    expect(loaded.data.profile).toBeUndefined();
  });

  it('演示模式拒绝恢复密钥验证且不创建恢复解锁态', async () => {
    await mockInvoke('set_master_password', { password: 'master-pass-1' });

    const started = await mockInvoke<{ success: boolean; message: string }>('start_recovery', {
      recoveryCode: '任意恢复码',
    });
    expect(started.success).toBe(false);
    expect(started.message).toContain('在线演示不提供');
    expect(sessionStorage.getItem('digilegacy-web-preview-recovery')).toBeNull();

    const completed = await mockInvoke<{ success: boolean; message: string }>('complete_recovery', {
      newPassword: 'new-master-pass',
      confirmPassword: 'new-master-pass',
    });
    expect(completed.success).toBe(false);
    expect(completed.message).toContain('在线演示不提供');
  });

  it('演示模式的更新、处置和触发间隔拒绝桌面端同样的非法边界', async () => {
    const added = await mockInvoke<{ data: number }>('add_asset', {
      req: { platform_id: 1, category: 'property', disposal_method: 'no_action' },
    });
    const invalidUpdate = await mockInvoke<{ success: boolean }>('update_asset', {
      id: added.data,
      req: { platform_id: 1, category: 'invalid', disposal_method: 'no_action' },
    });
    expect(invalidUpdate.success).toBe(false);

    const invalidDisposal = await mockInvoke<{ success: boolean }>('set_disposal_wish', {
      asset_id: added.data,
      disposal_method: 'inherit',
      heir_name: '',
    });
    expect(invalidDisposal.success).toBe(false);

    const invalidInterval = await mockInvoke<{ success: boolean }>('set_dead_mans_switch', {
      enabled: true,
      intervalDays: 0,
    });
    expect(invalidInterval.success).toBe(false);
  });
});

describe('凭据门禁', () => {
  it('已设主密码但锁定时拒绝保存凭据', async () => {
    await mockInvoke('set_master_password', { password: 'master-pass-1' });
    await mockInvoke('lock');
    const res = await mockInvoke<{ success: boolean; message: string }>('set_asset_credential', { id: 1, credential: 'secret' });
    expect(res.success).toBe(false);
    expect(res.message).toContain('请先解锁');
  });

  it('空凭据清除并同步 has_credential；删除资产同时清凭据', async () => {
    const added = await mockInvoke<{ data: number }>('add_asset', { req: { platform_id: 1, category: 'property' } });
    const id = added.data;

    const saved = await mockInvoke<{ success: boolean; message: string }>('set_asset_credential', { id, credential: 'abc' });
    expect(saved.message).toContain('演示模式未加密/仅用于展示');
    let detail = await mockInvoke<{ data: { has_credential: boolean } }>('get_asset_by_id', { id });
    expect(detail.data.has_credential).toBe(true);

    await mockInvoke('set_asset_credential', { id, credential: '' });
    detail = await mockInvoke<{ data: { has_credential: boolean } }>('get_asset_by_id', { id });
    expect(detail.data.has_credential).toBe(false);

    await mockInvoke('set_asset_credential', { id, credential: 'abc' });
    await mockInvoke('delete_asset', { id });
    const cred = await mockInvoke<{ data: string }>('get_asset_credential', { id });
    expect(cred.data).toBe('');
  });
});

describe('import_backup', () => {
  const validBackup = {
    app: 'digilegacy',
    version: 1,
    created_at: '2026-01-01 00:00:00',
    tables: {
      digital_assets: [{
        id: 7, platform_id: 1, platform_name: '支付宝', account_identifier: 'a', account_description: '',
        category: 'property', estimated_value: 5, value_currency: 'CNY', disposal_method: 'inherit',
        heir_name: null, heir_contact: null, notes: '', has_credential: false, is_confirmed: true,
        created_at: '', updated_at: '',
      }],
      user_profile: [],
      check_in_log: [],
      dms_config: { enabled: true, interval: 30, lastCheckIn: null },
    },
  };

  it('非法 JSON → 解析失败', async () => {
    const res = await mockInvoke<{ success: boolean; message: string }>('import_backup', { content: '{not-json' });
    expect(res.success).toBe(false);
    expect(res.message).toContain('解析失败');
  });

  it('非本应用备份 → 拒绝', async () => {
    const res = await mockInvoke<{ success: boolean; message: string }>('import_backup', {
      content: JSON.stringify({ app: 'other', version: 1 }),
    });
    expect(res.success).toBe(false);
    expect(res.message).toBe('不是有效的归零备份文件');
  });

  it('默认合并：追加备份资产、不清空现有数据、清空解锁态、续号正确', async () => {
    await mockInvoke('set_master_password', { password: 'master-pass-1' });
    const before = await mockInvoke<{ data: unknown[] }>('get_all_assets');
    const beforeCount = before.data.length;

    const res = await mockInvoke<{ success: boolean; data: string }>('import_backup', { content: JSON.stringify(validBackup) });
    expect(res.success).toBe(true);
    expect(res.data).toContain('已导入 1 条');
    expect(res.data).toContain('合并');
    expect(sessionStorage.getItem(SS_UNLOCK)).toBeNull();

    const after = await mockInvoke<{ data: unknown[] }>('get_all_assets');
    expect(after.data.length, '合并必须保留现有资产').toBe(beforeCount + 1);

    // 合并时不能复用备份里的 id（可能撞本地主键），应由本地分配
    const added = await mockInvoke<{ data: number }>('add_asset', { req: { platform_id: 1, category: 'mixed' } });
    expect(added.data).toBe(beforeCount + 2);
  });

  it('显式覆盖：清空现有资产后写入备份内容', async () => {
    const res = await mockInvoke<{ success: boolean; data: string }>('import_backup', {
      content: JSON.stringify(validBackup),
      mode: 'replace',
    });
    expect(res.success).toBe(true);
    expect(res.data).toContain('覆盖');

    const after = await mockInvoke<{ data: unknown[] }>('get_all_assets');
    expect(after.data.length, '覆盖后只剩备份里的资产').toBe(1);
  });
});

describe('open_external 协议白名单', () => {
  it('拒绝非 http/https 协议', async () => {
    for (const url of ['javascript:alert(1)', 'file:///etc/passwd', 'ftp://x.com']) {
      const res = await mockInvoke<{ success: boolean; message: string }>('open_external', { url });
      expect(res.success).toBe(false);
      expect(res.message).toBe('仅允许打开 http/https 链接');
    }
  });

  it('https 放行并新窗口打开', async () => {
    const opener = vi.fn();
    vi.stubGlobal('open', opener);
    const res = await mockInvoke<{ success: boolean }>('open_external', { url: 'https://example.com' });
    expect(res.success).toBe(true);
    expect(opener).toHaveBeenCalledWith('https://example.com', '_blank', 'noopener');
  });
});

it('未知命令直接抛错', async () => {
  await expect(mockInvoke('no_such_cmd')).rejects.toThrow('Web 预览模式未实现命令: no_such_cmd');
});
