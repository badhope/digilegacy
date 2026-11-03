import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// invoke.ts 是所有 IPC 的唯一入口，决定三件事：
//   1. Tauri 桌面环境 → 走真实后端（生产路径）
//   2. 浏览器 dev  → 走 mock（仅预览）
//   3. 其他        → 明确抛错
// 此前这条文件 0% 覆盖：Tauri 分支（生产唯一路径）从未被执行过。
// 若 isTauriEnv 判定写错，整个应用在所有平台上都会静默走 mock 或直接抛错。

const tauriInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => tauriInvoke(...args),
}));

const mockInvoke = vi.fn();
vi.mock('./mock', () => ({
  mockInvoke: (...args: unknown[]) => mockInvoke(...args),
}));

async function loadModule() {
  vi.resetModules();
  return import('./invoke');
}

beforeEach(() => {
  tauriInvoke.mockReset();
  mockInvoke.mockReset();
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

describe('isTauriEnv', () => {
  it('window 上没有 __TAURI_INTERNALS__ 时判定为非 Tauri', async () => {
    const { isTauriEnv } = await loadModule();
    expect(isTauriEnv()).toBe(false);
  });

  it('存在 __TAURI_INTERNALS__ 时判定为 Tauri', async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    const { isTauriEnv } = await loadModule();
    expect(isTauriEnv()).toBe(true);
  });
});

describe('invoke 路由', () => {
  it('Tauri 环境：透传给真实后端，不碰 mock', async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    tauriInvoke.mockResolvedValue({ success: true, message: '', data: 'real' });
    const { invoke } = await loadModule();

    const res = await invoke('get_all_assets', { a: 1 });

    expect(res).toEqual({ success: true, message: '', data: 'real' });
    expect(tauriInvoke).toHaveBeenCalledWith('get_all_assets', { a: 1 });
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('Tauri 环境：后端异常原样向上抛（不被吞掉）', async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    tauriInvoke.mockRejectedValue(new Error('IPC 断开'));
    const { invoke } = await loadModule();

    await expect(invoke('lock')).rejects.toThrow('IPC 断开');
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('浏览器 dev 环境：走 mock 后端', async () => {
    mockInvoke.mockResolvedValue({ success: true, message: '', data: 'mocked' });
    const { invoke } = await loadModule();

    const res = await invoke('get_all_assets');

    expect(res).toEqual({ success: true, message: '', data: 'mocked' });
    expect(mockInvoke).toHaveBeenCalledWith('get_all_assets', undefined);
    expect(tauriInvoke).not.toHaveBeenCalled();
  });

  it('纯前端生产构建（非 Tauri、非 dev、非 demo）：明确报错而不是静默返回 undefined', async () => {
    // 这条分支只有「把 dist 直接丢给浏览器、又没有演示开关」时才会命中。
    // 静默失败会让用户以为数据为空；必须给出可操作的提示。
    vi.stubEnv('DEV', false);
    try {
      const { invoke } = await loadModule();
      await expect(invoke('get_all_assets')).rejects.toThrow(/未连接本地后端/);
      expect(mockInvoke).not.toHaveBeenCalled();
      expect(tauriInvoke).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllEnvs();
    }
  });
});

describe('IS_WEB_PREVIEW', () => {
  it('dev 环境且非 Tauri 时为 true', async () => {
    const { IS_WEB_PREVIEW } = await loadModule();
    expect(IS_WEB_PREVIEW).toBe(true);
  });

  it('Tauri 环境永远为 false（桌面版不得加载演示后端）', async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    const { IS_WEB_PREVIEW } = await loadModule();
    expect(IS_WEB_PREVIEW).toBe(false);
  });
});
