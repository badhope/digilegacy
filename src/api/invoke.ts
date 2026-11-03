// 统一的 invoke 入口：Tauri 桌面环境走真实后端；
// 浏览器环境（npm run dev 或 VITE_DEMO=1 构建的在线演示）走本地 mock。
//
// 在线演示是给别人点开看的静态站点：数据存浏览器 localStorage，
// 明确标注「演示数据」，绝不承载真实资料。

import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export function isTauriEnv(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** 是否为浏览器演示模式（含线上静态演示构建）。 */
export const IS_WEB_PREVIEW =
  typeof window !== 'undefined' && !isTauriEnv() && (import.meta.env.DEV || __DEMO_MODE__);

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriEnv()) {
    return tauriInvoke<T>(cmd, args);
  }
  // 该分支在生产构建里是死代码（DEV=false、__DEMO_MODE__=false），
  // Rollup 会连同 mock 的 import 一起摇掉，因此桌面安装包不含演示后端。
  if (import.meta.env.DEV || __DEMO_MODE__) {
    const { mockInvoke } = await import('./mock');
    return mockInvoke<T>(cmd, args);
  }
  throw new Error('当前为纯前端构建，未连接本地后端。请使用桌面版，或运行 npm run dev 查看演示。');
}
