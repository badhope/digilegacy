import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';

// 反馈层是「替换原生 alert/confirm」的核心：Toast 必须自动消失、可手动关闭；
// 确认框必须返回 Promise<boolean> 并支持危险操作样式。
import FeedbackLayer from './FeedbackLayer.vue';
import { __resetFeedbackForTests, confirmDialog, settleConfirm, toast } from '../composables/feedback';

function mountLayer() {
  return mount(FeedbackLayer, { attachTo: document.body });
}

beforeEach(() => {
  // 全局状态必须在每个用例前重置，否则上一条 toast 会串到下一个断言
  __resetFeedbackForTests();
});

describe('Toast', () => {
  it('调用 toast 后渲染出提示文案', async () => {
    const w = mountLayer();
    toast('已保存');
    await flushPromises();

    const el = w.find('.toast');
    expect(el.exists(), 'toast 必须渲染').toBe(true);
    expect(el.text()).toContain('已保存');
    expect(el.classes()).toContain('toast-success');
  });

  it('错误类型使用 error 样式', async () => {
    const w = mountLayer();
    toast('保存失败', 'error');
    await flushPromises();

    expect(w.find('.toast').classes()).toContain('toast-error');
  });

  it('点关闭按钮移除该条提示', async () => {
    const w = mountLayer();
    toast('可关闭');
    await flushPromises();
    expect(w.find('.toast').exists()).toBe(true);

    await w.find('.toast-close').trigger('click');
    await flushPromises();
    expect(w.find('.toast').exists()).toBe(false);
  });

  it('超过时长自动消失', async () => {
    vi.useFakeTimers();
    try {
      const w = mountLayer();
      toast('短提示', 'info', 1000);
      await flushPromises();
      expect(w.find('.toast').exists()).toBe(true);

      vi.advanceTimersByTime(1200);
      await flushPromises();
      expect(w.find('.toast').exists()).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it('具备 aria-live 以被读屏播报', () => {
    const w = mountLayer();
    expect(w.find('.toast-stack').attributes('aria-live')).toBe('polite');
  });
});

describe('确认框', () => {
  it('confirmDialog 返回 Promise，确认后 resolve(true)', async () => {
    const w = mountLayer();
    const p = confirmDialog({ title: '删除？', message: '不可恢复' });
    await flushPromises();

    expect(w.find('.confirm-card').exists(), '确认框必须渲染').toBe(true);
    expect(w.text()).toContain('删除？');
    expect(w.text()).toContain('不可恢复');

    const buttons = w.findAll('.confirm-actions button');
    await buttons[1].trigger('click'); // 确认按钮
    await expect(p).resolves.toBe(true);
  });

  it('取消后 resolve(false)', async () => {
    const w = mountLayer();
    const p = confirmDialog({ title: '删除？' });
    await flushPromises();

    await w.findAll('.confirm-actions button')[0].trigger('click');
    await expect(p).resolves.toBe(false);
  });

  it('danger 选项让确认按钮变红', async () => {
    const w = mountLayer();
    confirmDialog({ title: '危险操作', danger: true });
    await flushPromises();

    const btn = w.findAll('.confirm-actions button')[1];
    expect(btn.classes()).toContain('btn-danger');
  });

  it('自定义按钮文案生效', async () => {
    const w = mountLayer();
    confirmDialog({ title: 't', confirmText: '覆盖导入', cancelText: '再想想' });
    await flushPromises();

    expect(w.text()).toContain('覆盖导入');
    expect(w.text()).toContain('再想想');
  });

  it('具备 alertdialog 语义与 aria-modal', async () => {
    const w = mountLayer();
    confirmDialog({ title: '语义检查' });
    await flushPromises();

    const card = w.find('.confirm-card');
    expect(card.attributes('role')).toBe('alertdialog');
    expect(card.attributes('aria-modal')).toBe('true');
    expect(card.attributes('aria-labelledby')).toBeTruthy();
  });

  it('点遮罩取消（不留悬挂 Promise）', async () => {
    const w = mountLayer();
    const p = confirmDialog({ title: '点遮罩' });
    await flushPromises();

    await w.find('.confirm-overlay').trigger('click');
    await expect(p).resolves.toBe(false);
  });

  it('连续调用时旧 Promise 按取消处理，不悬挂', async () => {
    mountLayer();
    const first = confirmDialog({ title: '第一个' });
    const second = confirmDialog({ title: '第二个' });
    await flushPromises();

    await expect(first).resolves.toBe(false);
    settleConfirm(true);
    await expect(second).resolves.toBe(true);
  });
});

describe('边界分支', () => {
  it('info 类型 toast 使用默认样式（非 success/error 分支）', async () => {
    const w = mountLayer();
    toast('请稍候', 'info');
    await flushPromises();
    const el = w.find('.toast');
    expect(el.classes()).toContain('toast-info');
  });

  it('点遮罩空白处等价于取消', async () => {
    const w = mountLayer();
    const p = confirmDialog({ title: '遮罩取消' });
    await flushPromises();
    await w.find('.confirm-overlay').trigger('click'); // @click.self
    await expect(p).resolves.toBe(false);
  });

  it('Tab 在确认框两按钮间循环（焦点困留）', async () => {
    const w = mountLayer();
    confirmDialog({ title: '焦点困留' });
    await flushPromises();
    const card = w.find('.confirm-card');
    const buttons = w.findAll('.confirm-actions button');
    (buttons[1].element as HTMLButtonElement).focus();
    await card.trigger('keydown', { key: 'Tab' });
    expect(document.activeElement).toBe(buttons[0].element);
    await card.trigger('keydown', { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(buttons[1].element);
  });

  it('danger 确认框初始焦点落在「取消」，避免回车误执行不可逆操作', async () => {
    const w = mountLayer();
    confirmDialog({ title: '危险', danger: true });
    await flushPromises();
    const buttons = w.findAll('.confirm-actions button');
    expect(document.activeElement).toBe(buttons[0].element);
  });
});
