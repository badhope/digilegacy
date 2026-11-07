// 统一的 invoke 入口：三种运行方式共用同一套前端代码。
//
//   1. Tauri 桌面环境            → IPC（生产路径）
//   2. 自托管 Web 服务（HTTP）    → POST /api/invoke，与桌面同一份核心命令
//   3. 静态演示站（VITE_DEMO=1）  → 浏览器 localStorage mock，明确标注「演示数据」
//
// 判定顺序是关键：先认 Tauri，再认 HTTP 后端，最后才回落 mock。
// 反过来会把「真的连了后端」误判成演示，用户看到的就不是自己的数据。

import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export function isTauriEnv(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** 部署在 HTTP 服务下时，前端与 API 同源；可用 VITE_API_BASE 指向别处。 */
const API_BASE = String(import.meta.env.VITE_API_BASE ?? '').replace(/\/$/, '');

/** 本地 `npm run dev` 想连真后端时的显式开关：VITE_HTTP_BACKEND=1。 */
const FORCE_HTTP = import.meta.env.VITE_HTTP_BACKEND === '1';

/**
 * 是否连接了真实的 HTTP 后端。
 *
 * - 生产构建（`vite build`，非演示）：一定是 HTTP 后端。
 * - 开发服务器：默认仍走 mock（改界面不用起服务），除非显式开启 FORCE_HTTP。
 * - 演示构建：永远 mock，因为它被打包成纯静态站点上传，同源不会有 /api。
 */
export const HAS_HTTP_BACKEND =
  typeof window !== 'undefined' && !isTauriEnv() && !__DEMO_MODE__ && (import.meta.env.PROD || FORCE_HTTP);

/** 是否为浏览器演示模式（开发预览或线上静态演示）。 */
export const IS_WEB_PREVIEW =
  typeof window !== 'undefined' && !isTauriEnv() && !HAS_HTTP_BACKEND && (import.meta.env.DEV || __DEMO_MODE__);

const TOKEN_KEY = 'digilegacy_api_token';
let memoryToken = '';

/** 服务端开了鉴权时，让用户填一次 token；只存在 sessionStorage。 */
export function setApiToken(token: string): void {
  memoryToken = token;
  try {
    if (token) sessionStorage.setItem(TOKEN_KEY, token);
    else sessionStorage.removeItem(TOKEN_KEY);
  } catch {
    /* 隐私模式下 sessionStorage 可能不可用：内存兜底即可 */
  }
}

export function getApiToken(): string {
  if (memoryToken) return memoryToken;
  try {
    return sessionStorage.getItem(TOKEN_KEY) ?? '';
  } catch {
    return '';
  }
}

function authHeaders(): Record<string, string> {
  const token = getApiToken();
  return token ? { Authorization: `Bearer ${token}` } : {};
}

interface InvokeEnvelope<T> {
  success: boolean;
  message: string;
  data: T | null;
}

/**
 * 通过 HTTP 后端调用核心命令。
 *
 * 关键点：**返回完整信封** `{success, message, data}`，与 Tauri IPC 一字不差。
 * 早期实现解包成裸 `data`，结果所有 store 里的 `res.data` 都变成 undefined，
 * 界面显示「0 个资产」而接口明明返回了数据——安静的错误比报错更难查。
 * 因此这里不做任何形状转换；只有传输层失败（非 2xx、非 JSON、网络不可达）
 * 才抛错，与 Tauri 侧 `Result::Err` 的语义对齐。
 */
async function httpInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${API_BASE}/api/invoke`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...authHeaders() },
      body: JSON.stringify({ cmd, args: args ?? {} }),
    });
  } catch (e) {
    throw new Error(
      `无法连接本地后端（${e instanceof Error ? e.message : String(e)}）。` +
        '请确认 digilegacy-server 已启动，或使用桌面版。'
    );
  }

  if (res.status === 401) {
    throw new Error('需要访问令牌：请在设置中填入服务端启动时配置的 token');
  }

  let payload: unknown;
  try {
    payload = await res.json();
  } catch {
    throw new Error(`服务端返回了非 JSON 响应（HTTP ${res.status}）`);
  }

  const env = payload as Partial<InvokeEnvelope<T>> & { message?: string };
  if (env === null || typeof env !== 'object') {
    throw new Error(`服务端返回了无法识别的响应（HTTP ${res.status}）`);
  }

  // 有信封就按信封走，**不看 HTTP 状态码**。
  //
  // 关键点：业务失败（success:false）在 Tauri 侧是正常返回值，store 靠
  // res.success/res.message 分支处理；抛错会让界面走 catch，把后端那句
  // 「继承人姓名不能为空」换成笼统的「加载失败」。同理，只读模式、命令被禁、
  // 未知命令这些传输层拒绝也带完整信封和可读原因，交给 store 展示更有用。
  if (typeof env.success === 'boolean') {
    return {
      success: env.success,
      message: env.message ?? '',
      data: (env.data ?? null) as T,
    } as unknown as T;
  }

  // 没有信封：2xx 视为宽松成功（反向代理或旧服务可能只回裸 data），
  // 非 2xx 视为真正的传输失败并带上状态码与后端消息。
  if (!res.ok) {
    throw new Error(env.message || `请求失败（HTTP ${res.status}）`);
  }
  return {
    success: true,
    message: env.message ?? '',
    data: (env.data ?? null) as T,
  } as unknown as T;
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriEnv()) {
    return tauriInvoke<T>(cmd, args);
  }
  if (HAS_HTTP_BACKEND) {
    return httpInvoke<T>(cmd, args);
  }
  // 该分支在桌面生产构建里是死代码（DEV=false、__DEMO_MODE__=false），
  // Rollup 会连同 mock 的 import 一起摇掉，因此桌面安装包不含演示后端。
  if (import.meta.env.DEV || __DEMO_MODE__) {
    const { mockInvoke } = await import('./mock');
    return mockInvoke<T>(cmd, args);
  }
  // 兜底：既不是 Tauri、也没有 HTTP 后端、又不是 dev/demo。
  // 只有「把 dist 直接丢给浏览器且未配反向代理」时才会命中，必须说清怎么修。
  throw new Error(
    '当前为纯前端构建，未连接本地后端。请使用桌面版、启动 digilegacy-server 托管该页面，或运行 npm run dev 查看演示。'
  );
}
