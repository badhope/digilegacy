import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import type { Mock } from 'vitest';

// mock 掉 invoke 层：本文件只验证 store 自身的分支逻辑（计算属性 / 失败分支），
// 不重复测 mock.ts 的命令实现。
vi.mock('../api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => false,
}));

import { invoke } from '../api/invoke';
import { useAssetsStore } from './assets';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();

const mockedInvoke = invoke as unknown as Mock;

function asset(over: Record<string, unknown> = {}) {
  return {
    id: 1,
    platform_id: 1,
    platform_name: '支付宝',
    account_identifier: 'a',
    account_description: '',
    category: 'property',
    estimated_value: 100,
    value_currency: 'CNY',
    disposal_method: 'no_action',
    heir_name: null,
    heir_contact: null,
    notes: '',
    has_credential: false,
    is_confirmed: true,
    created_at: '',
    updated_at: '',
    ...over,
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
});

describe('assets store 计算属性', () => {
  it('totalValue 对 null/undefined 估值按 0 求和', () => {
    const s = useAssetsStore();
    s.assets = [
      asset({ id: 1, estimated_value: 100 }),
      asset({ id: 2, estimated_value: null }),
      asset({ id: 3, estimated_value: 50 }),
    ] as never;
    expect(s.totalValue).toBe(150);
  });

  it('disposalPlannedCount 排除 no_action 与空值', () => {
    const s = useAssetsStore();
    s.assets = [
      asset({ id: 1, disposal_method: 'no_action' }),
      asset({ id: 2, disposal_method: 'inherit' }),
      asset({ id: 3, disposal_method: '' }),
    ] as never;
    expect(s.disposalPlannedCount).toBe(1);
  });

  it('类别计数各自独立', () => {
    const s = useAssetsStore();
    s.assets = [
      asset({ id: 1, category: 'property' }),
      asset({ id: 2, category: 'personality' }),
      asset({ id: 3, category: 'personality' }),
    ] as never;
    expect(s.propertyCount).toBe(1);
    expect(s.personalityCount).toBe(2);
    expect(s.mixedCount).toBe(0);
  });
});

describe('assets store 失败分支', () => {
  it('loadAssets 返回 success:false 时不覆盖现有列表', async () => {
    const s = useAssetsStore();
    s.assets = [asset({ id: 42 })] as never;

    mockedInvoke.mockResolvedValueOnce({ success: false, message: '数据库未初始化', data: null });
    await s.loadAssets();

    expect(s.assets.length).toBe(1);
    expect(s.assets[0].id).toBe(42);
  });

  it('addAsset 失败时向上抛错（写路径显式失败）', async () => {
    const s = useAssetsStore();
    mockedInvoke.mockResolvedValueOnce({
      success: false,
      message: '主密码已设置但处于锁定状态，请先解锁后再保存',
      data: null,
    });

    await expect(
      s.addAsset({
        platform_id: 1,
        account_identifier: 'x',
        account_description: '',
        category: 'property',
        estimated_value: 1,
        disposal_method: 'inherit',
        heir_name: undefined,
        heir_contact: undefined,
        notes: '',
      })
    ).rejects.toThrow('请先解锁');
  });

  it('deleteAsset 失败时向上抛错', async () => {
    const s = useAssetsStore();
    mockedInvoke.mockResolvedValueOnce({ success: false, message: '删除失败', data: null });
    await expect(s.deleteAsset(1)).rejects.toThrow('删除失败');
  });
});

describe('assets store 详情与更新', () => {
  it('loadAssetById 成功时写入 currentAsset', async () => {
    const s = useAssetsStore();
    mockedInvoke.mockResolvedValueOnce({ success: true, message: '', data: asset({ id: 7, platform_name: '微信' }) });

    const result = await s.loadAssetById(7);

    expect(result?.id).toBe(7);
    expect(s.currentAsset?.platform_name).toBe('微信');
  });

  it('loadAssetById 返回 null 时置空而不是保留旧值', async () => {
    const s = useAssetsStore();
    await s.loadAssetById(1); // 先设一个
    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValueOnce({ success: true, message: '', data: null });

    const result = await s.loadAssetById(999);

    expect(result).toBeNull();
    expect(s.currentAsset).toBeNull();
  });

  it('loadAssetById 失败时保留旧值', async () => {
    const s = useAssetsStore();
    mockedInvoke.mockResolvedValueOnce({ success: true, message: '', data: asset({ id: 3 }) });
    await s.loadAssetById(3);

    mockedInvoke.mockReset();
    mockedInvoke.mockRejectedValueOnce(new Error('IPC 断开'));
    const result = await s.loadAssetById(3);

    expect(result?.id).toBe(3);
  });

  it('updateAsset 成功后重新加载列表', async () => {
    const s = useAssetsStore();
    mockedInvoke
      .mockResolvedValueOnce({ success: true, message: '', data: null })
      .mockResolvedValueOnce({ success: true, message: '', data: [asset({ id: 1 })] });

    await s.updateAsset(1, {
      platform_id: 1, account_identifier: 'x', account_description: '',
      category: 'property', estimated_value: 1, disposal_method: 'inherit',
      heir_name: undefined, heir_contact: undefined, notes: '',
    });

    expect(mockedInvoke.mock.calls.map(c => c[0])).toEqual(['update_asset', 'get_all_assets']);
    expect(s.assets).toHaveLength(1);
  });

  it('updateAsset 失败时抛错且不刷新', async () => {
    const s = useAssetsStore();
    mockedInvoke.mockResolvedValueOnce({ success: false, message: '锁定状态', data: null });

    await expect(s.updateAsset(1, {
      platform_id: 1, account_identifier: 'x', account_description: '',
      category: 'property', estimated_value: 1, disposal_method: 'inherit',
      heir_name: undefined, heir_contact: undefined, notes: '',
    })).rejects.toThrow('锁定状态');

    expect(mockedInvoke.mock.calls.map(c => c[0])).toEqual(['update_asset']);
  });
});
