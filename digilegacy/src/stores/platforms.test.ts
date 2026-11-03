import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import type { Mock } from 'vitest';

// platforms store 的平台选择集直接决定「资产发现」写入哪些平台，
// 这里锁定去重加载、选择集语义与按 id 查找。
vi.mock('../api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => false,
}));

import { invoke } from '../api/invoke';
import { usePlatformsStore } from './platforms';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();

const mockedInvoke = invoke as unknown as Mock;

const platform = (id: number, name: string) => ({
  id, name,
  category: '支付金融', subtype: '', has_property: true, has_personality: false,
  inheritable: true, required_documents: '', process_description: '',
  estimated_difficulty: '', official_policy_url: '', estimated_time_days: '', notes: '',
});

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
});

describe('加载', () => {
  it('首次加载写入 data 并置 loaded', async () => {
    const s = usePlatformsStore();
    mockedInvoke.mockResolvedValueOnce({ success: true, message: '', data: [platform(1, '支付宝')] });

    await s.loadPlatforms();

    expect(s.platforms).toHaveLength(1);
    expect(s.loaded).toBe(true);
    expect(s.loading).toBe(false);
  });

  it('已加载后再次调用不再请求（缓存语义）', async () => {
    const s = usePlatformsStore();
    mockedInvoke.mockResolvedValueOnce({ success: true, message: '', data: [platform(1, '支付宝')] });
    await s.loadPlatforms();

    await s.loadPlatforms();

    expect(mockedInvoke).toHaveBeenCalledTimes(1);
  });

  it('失败时不置 loaded，允许下次重试', async () => {
    const s = usePlatformsStore();
    mockedInvoke.mockResolvedValueOnce({ success: false, message: 'db closed', data: null });

    await s.loadPlatforms();

    expect(s.loaded).toBe(false);
    expect(s.platforms).toHaveLength(0);
    expect(s.loading).toBe(false);
  });

  it('异常路径也必须复位 loading', async () => {
    const s = usePlatformsStore();
    mockedInvoke.mockRejectedValueOnce(new Error('IPC 断开'));

    await s.loadPlatforms();

    expect(s.loading).toBe(false);
  });

  it('loadPlatformsExtended 独立于基础列表', async () => {
    const s = usePlatformsStore();
    mockedInvoke.mockResolvedValueOnce({
      success: true, message: '', data: [{ ...platform(2, '微信'), name_en: 'WeChat' }],
    });

    await s.loadPlatformsExtended();

    expect(s.platformsExtended).toHaveLength(1);
    expect(s.getExtendedById(2)?.name_en).toBe('WeChat');
  });

  it('扩展列表也有缓存，force 才重取', async () => {
    const s = usePlatformsStore();
    mockedInvoke.mockResolvedValue({
      success: true, message: '', data: [{ ...platform(2, '微信'), name_en: 'WeChat' }],
    });
    await s.loadPlatformsExtended();
    await s.loadPlatformsExtended();
    expect(mockedInvoke).toHaveBeenCalledTimes(1);

    await s.loadPlatformsExtended(true);
    expect(mockedInvoke).toHaveBeenCalledTimes(2);
  });

  it('refreshPlatforms 清掉扩展缓存并重取（导入补丁后不显示旧政策）', async () => {
    const s = usePlatformsStore();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_all_platforms') {
        return { success: true, message: '', data: [platform(1, '支付宝')] };
      }
      return { success: true, message: '', data: [{ ...platform(1, '支付宝'), name_en: 'Alipay' }] };
    });
    await s.loadPlatforms();
    await s.loadPlatformsExtended();
    expect(s.platformsExtended).toHaveLength(1);

    await s.refreshPlatforms();

    expect(s.platformsExtended).toHaveLength(0);
    expect(mockedInvoke.mock.calls.filter(c => c[0] === 'get_all_platforms')).toHaveLength(2);
  });
});

describe('选择集', () => {
  it('toggle 在选中与取消之间切换', async () => {
    const s = usePlatformsStore();
    expect(s.isSelected(5)).toBe(false);

    s.toggle(5);
    expect(s.isSelected(5)).toBe(true);

    s.toggle(5);
    expect(s.isSelected(5)).toBe(false);
  });

  it('clearSelection 清空全部选择', () => {
    const s = usePlatformsStore();
    s.toggle(1);
    s.toggle(2);

    s.clearSelection();

    expect(s.selectedIds.size).toBe(0);
    expect(s.isSelected(1)).toBe(false);
  });

  it('getById 命中与未命中', async () => {
    const s = usePlatformsStore();
    mockedInvoke.mockResolvedValueOnce({ success: true, message: '', data: [platform(3, 'B站')] });
    await s.loadPlatforms();

    expect(s.getById(3)?.name).toBe('B站');
    expect(s.getById(999)).toBeUndefined();
  });
});
