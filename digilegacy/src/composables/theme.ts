// 主题：浅色 / 深色 / 跟随系统。
//
// 只切换 <html data-theme>，颜色全部由 CSS token 决定（见 main.css）。
// 选择持久化到 localStorage；未选择时跟随系统 prefers-color-scheme。

import { readonly, ref } from 'vue';

export type ThemeMode = 'light' | 'dark' | 'system';

const STORAGE_KEY = 'digilegacy_theme';
const mode = ref<ThemeMode>('system');
const resolved = ref<'light' | 'dark'>('light');

const media = typeof window !== 'undefined' ? window.matchMedia?.('(prefers-color-scheme: dark)') : undefined;

function systemPrefersDark() {
  return !!media?.matches;
}

function apply(next: 'light' | 'dark') {
  resolved.value = next;
  document.documentElement.setAttribute('data-theme', next);
  // 同步浏览器/WebView 的控件配色（滚动条、下拉箭头等原生部件）
  document.documentElement.style.colorScheme = next;
}

function resolve() {
  apply(mode.value === 'system' ? (systemPrefersDark() ? 'dark' : 'light') : mode.value);
}

export function initTheme() {
  const saved = localStorage.getItem(STORAGE_KEY) as ThemeMode | null;
  if (saved === 'light' || saved === 'dark' || saved === 'system') mode.value = saved;
  resolve();
  // 系统主题变化时，若为「跟随系统」则实时切换
  media?.addEventListener?.('change', () => {
    if (mode.value === 'system') apply(systemPrefersDark() ? 'dark' : 'light');
  });
}

export function setTheme(next: ThemeMode) {
  mode.value = next;
  localStorage.setItem(STORAGE_KEY, next);
  resolve();
}

/** 在浅/深之间切换（「跟随系统」时按当前实际外观取反）。 */
export function toggleTheme() {
  setTheme(resolved.value === 'dark' ? 'light' : 'dark');
}

export function useTheme() {
  return { mode: readonly(mode), resolved: readonly(resolved), setTheme, toggleTheme };
}
