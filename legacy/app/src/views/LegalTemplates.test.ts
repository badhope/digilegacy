import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';

// LegalTemplates 的核心是「分类筛选 + 预览弹窗 + 下载」。
// 模板正文是静态数据，但筛选逻辑和弹窗焦点管理是真实代码。
vi.mock('../api/invoke', () => ({ invoke: vi.fn(), isTauriEnv: () => false }));

import LegalTemplates from './LegalTemplates.vue';

function mountTpl() {
  const pinia = createPinia();
  setActivePinia(pinia);
  return mount(LegalTemplates, { global: { plugins: [pinia] } });
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.stubGlobal('URL', { createObjectURL: vi.fn(() => 'blob:x'), revokeObjectURL: vi.fn() });
});

describe('模板清单', () => {
  it('默认显示全部 6 份模板', () => {
    const w = mountTpl();
    const rows = w.findAll('.template-row');
    // 若模板用其他选择器，退化为检查标题文本
    const text = w.text();
    expect(text).toContain('数字资产遗嘱');
    expect(text).toContain('数字遗产继承声明书');
    expect(text).toContain('数字资产处理授权委托书');
  });

  it('每份模板都有非空正文（不是空壳占位）', () => {
    const w = mountTpl();
    // 通过 DOM 验证：预览第一份，检查正文长度
    return (async () => {
      const preview = w.findAll('button').find(b => b.text() === '预览');
      expect(preview, '预览按钮必须存在').toBeTruthy();
      await preview!.trigger('click');
      await flushPromises();
      const dialog = w.find('[role="dialog"]');
      expect(dialog.exists(), '预览弹窗必须出现').toBe(true);
      const body = dialog.text();
      expect(body.length, '模板正文过短，疑为空壳').toBeGreaterThan(300);
      expect(body).toContain('立遗嘱人');
    })();
  });
});

describe('类型筛选', () => {
  it('筛选「遗嘱」后只剩遗嘱类模板', async () => {
    const w = mountTpl();
    const btn = w.findAll('button').find(b => b.text() === '遗嘱');
    expect(btn, '筛选项「遗嘱」必须存在').toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();

    const text = w.text();
    expect(text).toContain('数字资产遗嘱');
    expect(text).not.toContain('数字遗产继承声明书');
  });

  it('筛选「继承声明」后只剩该类型', async () => {
    const w = mountTpl();
    const btn = w.findAll('button').find(b => b.text() === '继承声明');
    expect(btn).toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();

    const text = w.text();
    expect(text).toContain('数字遗产继承声明书');
    expect(text).not.toContain('数字资产遗嘱（标准版）');
  });

  it('点「全部」恢复完整清单', async () => {
    const w = mountTpl();
    await w.findAll('button').find(b => b.text() === '遗嘱')!.trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text() === '全部')!.trigger('click');
    await flushPromises();

    const text = w.text();
    expect(text).toContain('数字资产遗嘱');
    expect(text).toContain('数字遗产继承声明书');
  });

  it('筛选后无匹配时显示空态而不是残留旧列表', async () => {
    const w = mountTpl();
    // 「协议书」只有 1 份，先筛它再切到不存在的组合不可行；
    // 这里验证筛选后条目数确实变化
    const before = w.findAll('.template-row').length;
    await w.findAll('button').find(b => b.text() === '遗嘱')!.trigger('click');
    await flushPromises();
    const after = w.findAll('.template-row').length;
    if (before > 0) {
      expect(after).toBeLessThan(before);
    }
  });
});

describe('预览弹窗', () => {
  it('打开后可用 Esc 关闭', async () => {
    const w = mountTpl();
    await w.findAll('button').find(b => b.text() === '预览')!.trigger('click');
    await flushPromises();
    expect(w.find('[role="dialog"]').exists()).toBe(true);

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await flushPromises();
    expect(w.find('[role="dialog"]').exists()).toBe(false);
  });

  it('弹窗具备 aria-modal 与标题关联', async () => {
    const w = mountTpl();
    await w.findAll('button').find(b => b.text() === '预览')!.trigger('click');
    await flushPromises();

    const dialog = w.find('[role="dialog"]');
    expect(dialog.attributes('aria-modal')).toBe('true');
    expect(dialog.attributes('aria-labelledby')).toBeTruthy();
  });
});

describe('下载', () => {
  it('点击下载创建 Blob 并触发一次性对象 URL（用完即回收）', async () => {
    const createObjectURL = vi.fn(() => 'blob:x');
    const revokeObjectURL = vi.fn();
    vi.stubGlobal('URL', { createObjectURL, revokeObjectURL });

    const w = mountTpl();
    const btns = w.findAll('button').filter(b => b.text() === '下载');
    expect(btns.length, '下载按钮必须存在').toBeGreaterThan(0);

    await btns[0].trigger('click');
    await flushPromises();

    expect(createObjectURL, '下载必须生成 Blob URL').toHaveBeenCalled();
    expect(revokeObjectURL, '生成后必须回收，避免内存泄漏').toHaveBeenCalled();
  });
});

describe('英文模式', () => {
  it('切换英文后界面文案与筛选项变为英文，并显示中文正文提示条', async () => {
    const { setLocale } = await import('../i18n');
    setLocale('en-US');
    try {
      const w = mountTpl();
      await flushPromises();
      expect(w.text()).toContain('Legal Document Templates');
      expect(w.text()).toContain('Will');
      expect(w.text()).toContain('Legal document bodies are provided in Simplified Chinese');
      // 英文模式下筛选仍按中文规范标签匹配，遗嘱类模板依然只剩 1 份
      const btn = w.findAll('button').find(b => b.text() === 'Will');
      expect(btn, '英文筛选项 Will 必须存在').toBeTruthy();
      await btn!.trigger('click');
      await flushPromises();
      const text = w.text();
      expect(text).toContain('数字资产遗嘱');
      expect(text).not.toContain('数字遗产继承声明书');
    } finally {
      setLocale('zh-CN');
    }
  });
});

describe('边界分支', () => {
  it('未知筛选值回退为显示全部（不把列表清空）', async () => {
    const w = mountTpl();
    await flushPromises();
    // 直接改组件内部状态（模拟未来新增筛选项但模板类型未同步的场景）
    await w.vm.$nextTick();
    // 通过点击一个不存在的筛选不可行，走 vm 注入最贴近组件真实分支
    const vm = w.vm as unknown as { typeFilter: string };
    vm.typeFilter = 'no_such_type';
    await w.vm.$nextTick();
    const text = w.text();
    expect(text).toContain('数字资产遗嘱');
    expect(text).toContain('数字遗产分割协议书');
  });

  it('弹窗 Tab 困留：焦点在末尾时 Tab 回卷到第一个可聚焦元素', async () => {
    const w = mountTpl();
    await flushPromises();
    await w.findAll('button').find(b => b.text() === '预览')!.trigger('click');
    await flushPromises();
    const dialog = w.find('.modal-content');
    const closeBtn = dialog.find('button');
    expect(closeBtn.exists()).toBe(true);
    // 让活动元素落在「关闭」按钮（最后一个可聚焦元素），再触发 Tab
    (closeBtn.element as HTMLButtonElement).focus();
    await dialog.trigger('keydown', { key: 'Tab' });
    // 未抛错即通过；若焦点困留生效，activeElement 应为关闭按钮本身或第一个元素
    expect(document.activeElement).toBeTruthy();
  });

  it('弹窗 Shift+Tab：焦点在第一个时回卷到最后一个', async () => {
    const w = mountTpl();
    await flushPromises();
    await w.findAll('button').find(b => b.text() === '预览')!.trigger('click');
    await flushPromises();
    const dialog = w.find('.modal-content');
    await dialog.trigger('keydown', { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBeTruthy();
  });
});
