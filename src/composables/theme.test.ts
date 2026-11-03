import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// 主题状态机：浅色/深色/跟随系统，持久化，toggle 取反语义，以及
// 「跟随系统」时监听系统主题变化实时切换。jsdom 默认没有 matchMedia，
// 这里在动态导入模块前打桩，验证系统切换分支。

const STORAGE_KEY = 'digilegacy_theme';

// matchMedia 桩：matches 可调，addEventListener 记录回调，触发时调用
function stubMatchMedia(matches: boolean) {
  const listeners = new Set<() => void>();
  const mm = {
    matches,
    media: '(prefers-color-scheme: dark)',
    addEventListener: (_: string, cb: () => void) => listeners.add(cb),
    removeEventListener: (_: string, cb: () => void) => listeners.delete(cb),
    addListener: (_: () => void) => {},
    removeListener: (_: () => void) => {},
    dispatchChange(next: boolean) {
      mm.matches = next;
      for (const cb of listeners) cb();
    },
  };
  vi.stubGlobal('matchMedia', vi.fn(() => mm));
  return mm;
}

type ThemeModule = typeof import('./theme');

let themeModule: ThemeModule | null = null;
let mm: ReturnType<typeof stubMatchMedia> | null = null;

async function loadTheme(withMedia: boolean | null) {
  vi.resetModules();
  if (withMedia === null) {
    vi.unstubAllGlobals();
  } else {
    mm = stubMatchMedia(withMedia);
  }
  themeModule = await import('./theme');
  return themeModule;
}

function state() {
  const t = themeModule!;
  const { mode, resolved } = t.useTheme();
  return { mode: mode.value, resolved: resolved.value };
}

beforeEach(async () => {
  localStorage.clear();
  document.documentElement.removeAttribute('data-theme');
  await loadTheme(false);
});

afterEach(() => {
  localStorage.clear();
  vi.unstubAllGlobals();
  themeModule = null;
});

describe('theme', () => {
  it('未保存过任何选择时默认跟随系统（系统为浅色 → 解析浅色）', () => {
    themeModule!.initTheme();
    expect(state().mode).toBe('system');
    expect(state().resolved).toBe('light');
  });

  it('系统偏好深色时跟随系统解析为深色', async () => {
    await loadTheme(true);
    themeModule!.initTheme();
    expect(state().resolved).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('保存的 dark/light 直接生效并写入 data-theme', async () => {
    await loadTheme(false);
    localStorage.setItem(STORAGE_KEY, 'dark');
    themeModule!.initTheme();
    expect(state().resolved).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');

    localStorage.setItem(STORAGE_KEY, 'light');
    themeModule!.initTheme();
    expect(state().resolved).toBe('light');
  });

  it('保存的非法值被忽略（保持当前模式）', () => {
    localStorage.setItem(STORAGE_KEY, 'neon');
    themeModule!.initTheme();
    expect(state().mode).toBe('system');
    expect(state().resolved).toBe('light');
  });

  it('setTheme 切换模式并持久化', () => {
    themeModule!.initTheme();
    themeModule!.setTheme('dark');
    expect(state().mode).toBe('dark');
    expect(localStorage.getItem(STORAGE_KEY)).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('toggleTheme 在浅/深之间取反', () => {
    themeModule!.initTheme();
    expect(state().resolved).toBe('light');
    themeModule!.toggleTheme();
    expect(state().resolved).toBe('dark');
    themeModule!.toggleTheme();
    expect(state().resolved).toBe('light');
  });

  it('跟随系统时系统主题变化实时切换（深→浅）', async () => {
    await loadTheme(true);
    themeModule!.initTheme();
    expect(state().resolved).toBe('dark');

    mm!.dispatchChange(false);
    expect(state().resolved).toBe('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });

  it('明确指定浅色后系统主题变化不再影响（仅跟随系统模式监听）', async () => {
    await loadTheme(true);
    themeModule!.initTheme();
    themeModule!.setTheme('light');
    mm!.dispatchChange(true);
    expect(state().resolved).toBe('light');
  });
});
