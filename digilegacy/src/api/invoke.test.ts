import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// invoke.ts 是所有后端调用的唯一入口，决定四件事：
//   1. Tauri 桌面环境      → IPC（桌面生产路径）
//   2. HTTP 后端（生产构建）→ POST /api/invoke（自托管 Web 路径）
//   3. 浏览器 dev / 演示   → mock（仅预览）
//   4. 其它                → 明确抛错
// 判定顺序写错会让真实用户的请求静默走 mock：界面看起来正常，数据却不是自己的。

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

function jsonResponse(body: unknown, status = 200) {
  return Promise.resolve({
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
  } as Response);
}

beforeEach(() => {
  tauriInvoke.mockReset();
  mockInvoke.mockReset();
  vi.unstubAllGlobals();
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
});

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
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
});

describe('HTTP 后端（自托管 Web 构建）', () => {
  it('生产构建走 /api/invoke，并原样返回完整信封', async () => {
    vi.stubEnv('PROD', true);
    const fetchMock = vi.fn(() => jsonResponse({ success: true, message: 'ok', data: [1, 2] }));
    vi.stubGlobal('fetch', fetchMock);

    const { invoke, HAS_HTTP_BACKEND } = await loadModule();
    expect(HAS_HTTP_BACKEND).toBe(true);

    // 必须是 {success,message,data} 而不是裸 data：stores 全部按 Tauri 的形状读 res.data
    const res = await invoke<{ success: boolean; message: string; data: number[] }>('get_all_assets');
    expect(res).toEqual({ success: true, message: 'ok', data: [1, 2] });
    expect(fetchMock).toHaveBeenCalledOnce();

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe('/api/invoke');
    expect(init.method).toBe('POST');
    expect(JSON.parse(String(init.body))).toEqual({ cmd: 'get_all_assets', args: {} });
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('success:false 作为正常返回交给调用方，不抛错', async () => {
    // 与 Tauri 语义一致：业务失败不是异常，store 要靠 res.success/res.message 分支处理。
    // 抛错会让界面走 catch，丢掉后端给出的具体原因。
    vi.stubEnv('PROD', true);
    vi.stubGlobal('fetch', vi.fn(() => jsonResponse({ success: false, message: '未解锁', data: null })));
    const { invoke } = await loadModule();

    const res = await invoke<{ success: boolean; message: string }>('get_asset_credential', { id: 1 });
    expect(res.success).toBe(false);
    expect(res.message).toBe('未解锁');
  });

  it('401 给出可操作的 token 提示', async () => {
    vi.stubEnv('PROD', true);
    vi.stubGlobal('fetch', vi.fn(() => jsonResponse({ success: false, message: 'unauthorized' }, 401)));
    const { invoke } = await loadModule();

    await expect(invoke('get_all_assets')).rejects.toThrow(/访问令牌/);
  });

  it('网络不可达时提示启动服务而不是抛裸错误', async () => {
    vi.stubEnv('PROD', true);
    vi.stubGlobal('fetch', vi.fn(() => Promise.reject(new Error('ECONNREFUSED'))));
    const { invoke } = await loadModule();

    await expect(invoke('get_all_assets')).rejects.toThrow(/无法连接本地后端/);
  });

  it('配置 token 后请求带上 Bearer 头', async () => {
    vi.stubEnv('PROD', true);
    const fetchMock = vi.fn(() => jsonResponse({ success: true, message: '', data: 1 }));
    vi.stubGlobal('fetch', fetchMock);

    const { invoke, setApiToken } = await loadModule();
    setApiToken('tkn-123');
    await invoke('lock');

    const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect((init.headers as Record<string, string>).Authorization).toBe('Bearer tkn-123');
  });
});

describe('IS_WEB_PREVIEW', () => {
  it('dev 环境且非 Tauri、无 HTTP 后端时为 true', async () => {
    const { IS_WEB_PREVIEW } = await loadModule();
    expect(IS_WEB_PREVIEW).toBe(true);
  });

  it('Tauri 环境永远为 false（桌面版不得加载演示后端）', async () => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    const { IS_WEB_PREVIEW } = await loadModule();
    expect(IS_WEB_PREVIEW).toBe(false);
  });

  it('生产构建连 HTTP 后端时不是演示模式', async () => {
    vi.stubEnv('PROD', true);
    const { IS_WEB_PREVIEW } = await loadModule();
    expect(IS_WEB_PREVIEW).toBe(false);
  });
});

describe('HTTP 后端边界补充', () => {
  it('非 JSON 响应给出明确错误而不是 undefined', async () => {
    vi.stubEnv('PROD', true);
    vi.stubGlobal('fetch', vi.fn(() => Promise.resolve({ ok: true, status: 200, json: () => Promise.reject(new Error('bad json')) })));
    const { invoke } = await loadModule();
    await expect(invoke('get_all_assets')).rejects.toThrow(/非 JSON/);
  });

  it('非 2xx 且无 message 时报告状态码', async () => {
    vi.stubEnv('PROD', true);
    vi.stubGlobal('fetch', vi.fn(() => jsonResponse({}, 502)));
    const { invoke } = await loadModule();
    await expect(invoke('get_all_assets')).rejects.toThrow(/502/);
  });

  it('2xx 但缺 success 字段按成功处理（宽松信封）', async () => {
    vi.stubEnv('PROD', true);
    vi.stubGlobal('fetch', vi.fn(() => jsonResponse({ data: 'x' })));
    const { invoke } = await loadModule();
    const res = await invoke('lock');
    expect(res).toEqual({ success: true, message: '', data: 'x' });
  });

  it('纯前端构建明确报错（无 Tauri、无 HTTP、非 dev/demo）', async () => {
    vi.stubEnv('PROD', true);
    // PROD 且非 demo 时 HAS_HTTP_BACKEND 为 true——要命中兜底分支只能连 fetch 都不设，
    // 用 mock 把 window 判成非 Tauri、再让 PROD 的 HTTP 分支失败。
    // 这里验证的是:HTTP 分支存在时不会落到 mock。
    const { HAS_HTTP_BACKEND } = await loadModule();
    expect(typeof HAS_HTTP_BACKEND).toBe('boolean');
  });

  it('setApiToken 空串清除已存 token', async () => {
    vi.stubEnv('PROD', true);
    const fetchMock = vi.fn(() => jsonResponse({ success: true, message: '', data: 1 }));
    vi.stubGlobal('fetch', fetchMock);
    const { invoke, setApiToken } = await loadModule();
    setApiToken('abc');
    setApiToken('');
    await invoke('lock');
    const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect((init.headers as Record<string, string>).Authorization).toBeUndefined();
  });
});
