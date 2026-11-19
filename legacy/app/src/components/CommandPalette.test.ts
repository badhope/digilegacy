import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { nextTick } from 'vue';

// CommandPalette 是全局导航入口：⌘K 打开、↑↓ 选择、Enter 执行、Esc 关闭。
// 这些是纯键盘路径，没有鼠标也要能用，因此必须逐条断言。
import CommandPalette from './CommandPalette.vue';

const routerPush = vi.fn();

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: routerPush }),
}));

function mountPalette(open = true) {
  return mount(CommandPalette, {
    props: { open },
    attachTo: document.body,
    global: {
      mocks: { $router: { push: routerPush } },
      // 组件用 Teleport 挂到 body：单测里禁用 teleport，让内容留在 wrapper 内，
      // 才能用 wrapper.find 断言（否则要被迫用 document 查询，断言会跨用例串味）。
      stubs: { teleport: true },
    },
  });
}

beforeEach(() => {
  localStorage.clear();
  document.body.innerHTML = '';
  routerPush.mockClear();
  setActivePinia(createPinia());
});

describe('开关与渲染', () => {
  it('open=false 时不渲染面板（不占 DOM，也不抢焦点）', () => {
    const w = mountPalette(false);
    expect(document.querySelector('.cmdk-panel')).toBeNull();
    expect(w.exists()).toBe(true);
  });

  it('open=true 时渲染面板与搜索框', () => {
    mountPalette(true);
    expect(document.querySelector('.cmdk-panel')).not.toBeNull();
    expect(document.querySelector('[data-testid="cmdk-input"]')).not.toBeNull();
  });

  it('默认列出全部导航项（9 个页面）', () => {
    mountPalette(true);
    const items = document.querySelectorAll('.cmdk-item');
    expect(items.length).toBeGreaterThanOrEqual(9);
  });
});

describe('搜索', () => {
  it('输入后只保留匹配项', async () => {
    const w = mountPalette(true);
    const input = w.find('[data-testid="cmdk-input"]');
    await input.setValue('dms');
    await nextTick();
    const items = document.querySelectorAll('.cmdk-item');
    // 「安心传承」的 keywords 含 dms，应命中；结果应显著少于全部
    expect(items.length).toBeGreaterThan(0);
    expect(items.length).toBeLessThan(9);
  });

  it('无匹配时给出空态，而不是空白列表', async () => {
    const w = mountPalette(true);
    await w.find('[data-testid="cmdk-input"]').setValue('zzzz-no-match');
    await nextTick();
    expect(document.querySelector('.cmdk-empty')).not.toBeNull();
  });
});

describe('键盘操作', () => {
  it('Esc 关闭面板并通知父组件', async () => {
    const w = mountPalette(true);
    await w.find('.cmdk-panel').trigger('keydown', { key: 'Escape' });
    expect(w.emitted('update:open')?.[0]).toEqual([false]);
  });

  it('Enter 执行当前选中项（第一项是仪表板）', async () => {
    const w = mountPalette(true);
    await w.find('.cmdk-panel').trigger('keydown', { key: 'Enter' });
    expect(routerPush).toHaveBeenCalledWith('/dashboard');
  });

  it('↓ 移动选中项后 Enter 执行第二项', async () => {
    const w = mountPalette(true);
    await w.find('.cmdk-panel').trigger('keydown', { key: 'ArrowDown' });
    await w.find('.cmdk-panel').trigger('keydown', { key: 'Enter' });
    expect(routerPush).toHaveBeenCalledWith('/discover');
  });

  it('↓ 到末尾后回到第一项（循环，不会卡住）', async () => {
    const w = mountPalette(true);
    const total = document.querySelectorAll('.cmdk-item').length;
    for (let i = 0; i < total; i++) {
      await w.find('.cmdk-panel').trigger('keydown', { key: 'ArrowDown' });
    }
    await w.find('.cmdk-panel').trigger('keydown', { key: 'Enter' });
    expect(routerPush).toHaveBeenCalledWith('/dashboard');
  });
});

describe('无障碍', () => {
  it('面板是 dialog 且有可访问名称', () => {
    mountPalette(true);
    const panel = document.querySelector('.cmdk-panel');
    expect(panel?.getAttribute('role')).toBe('dialog');
    expect(panel?.getAttribute('aria-modal')).toBe('true');
    expect(panel?.getAttribute('aria-labelledby')).toBe('cmdk-title');
  });

  it('输入框是 combobox 并关联结果列表', () => {
    mountPalette(true);
    const input = document.querySelector('[data-testid="cmdk-input"]');
    expect(input?.getAttribute('role')).toBe('combobox');
    expect(input?.getAttribute('aria-controls')).toBe('cmdk-results');
  });

  it('选中项通过 aria-selected 暴露给读屏', async () => {
    const w = mountPalette(true);
    const first = document.querySelector('.cmdk-item');
    expect(first?.getAttribute('aria-selected')).toBe('true');
    await w.find('.cmdk-panel').trigger('keydown', { key: 'ArrowDown' });
    const selected = document.querySelectorAll('.cmdk-item[aria-selected="true"]');
    expect(selected.length).toBe(1);
  });
});
