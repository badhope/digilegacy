import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';

// HelpDrawer 是帮助中心：Esc 关闭、Tab 焦点困留、重播引导。
// 焦点困留写错会让键盘用户 Tab 到背景内容，属于无障碍阻断级缺陷。
import HelpDrawer from './HelpDrawer.vue';

function mountDrawer() {
  return mount(HelpDrawer, { attachTo: document.body });
}

beforeEach(() => {
  document.body.innerHTML = '';
});

describe('渲染', () => {
  it('展示帮助内容与关闭按钮', () => {
    const w = mountDrawer();
    expect(w.text().length).toBeGreaterThan(100);
    const close = w.findAll('button').find(b => b.text().includes('关闭'));
    expect(close, '关闭按钮必须存在').toBeTruthy();
  });

  it('面板具备 dialog 语义', () => {
    const w = mountDrawer();
    const panel = w.find('[role="dialog"]');
    expect(panel.exists()).toBe(true);
    expect(panel.attributes('aria-modal')).toBe('true');
  });
});

describe('关闭', () => {
  it('点关闭按钮触发 close 事件', async () => {
    const w = mountDrawer();
    await w.findAll('button').find(b => b.text().includes('关闭'))!.trigger('click');
    await flushPromises();
    expect(w.emitted('close')).toBeTruthy();
  });

  it('Esc 触发 close 事件', async () => {
    const w = mountDrawer();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await flushPromises();
    expect(w.emitted('close')).toBeTruthy();
  });

  it('点遮罩关闭', async () => {
    const w = mountDrawer();
    const mask = w.find('.drawer-mask');
    expect(mask.exists(), '遮罩必须存在').toBe(true);
    await mask.trigger('click');
    await flushPromises();
    expect(w.emitted('close')).toBeTruthy();
  });
});

describe('重播引导', () => {
  it('点「重新播放新手引导」发出 replay-tour 事件', async () => {
    const w = mountDrawer();
    const btn = w.findAll('button').find(b => b.text().includes('重新播放'));
    expect(btn, '重播按钮必须存在').toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();
    expect(w.emitted('replay-tour')).toBeTruthy();
  });
});

describe('焦点管理', () => {
  it('挂载后焦点在面板内（关闭按钮）', async () => {
    const w = mountDrawer();
    await flushPromises();
    const panel = w.find('[role="dialog"]').element;
    expect(panel.contains(document.activeElement), '焦点必须在面板内').toBe(true);
  });

  it('Tab 在面板内循环：末元素 Tab 回到首元素，首元素 Shift+Tab 回到末元素', async () => {
    // jsdom 不实现浏览器默认的 Tab 焦点移动，只有 trapTab 会主动改焦点。
    // 因此这里断言的是「循环语义」（末→首、首→末），它只可能由 trapTab 产生，
    // 删掉监听时这组断言必然失败。
    const w = mountDrawer();
    await flushPromises();

    const panel = w.find('[role="dialog"]').element;
    const focusables = Array.from(
      panel.querySelectorAll<HTMLElement>('button:not([disabled]), input, [href], textarea, select')
    );
    expect(focusables.length, '面板内应有可聚焦元素').toBeGreaterThan(1);

    const last = focusables[focusables.length - 1];
    last.focus();
    expect(document.activeElement).toBe(last);

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
    await flushPromises();
    expect(document.activeElement, '末元素 Tab 后必须回到首元素').toBe(focusables[0]);

    focusables[0].focus();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true }));
    await flushPromises();
    expect(document.activeElement, '首元素 Shift+Tab 后必须回到末元素').toBe(last);
  });

  it('卸载时移除 document 级捕获监听', async () => {
    const w = mountDrawer();
    const removeSpy = vi.spyOn(document, 'removeEventListener');
    w.unmount();
    expect(removeSpy).toHaveBeenCalledWith('keydown', expect.any(Function), true);
    removeSpy.mockRestore();
  });

  it('卸载时把焦点归还给之前的元素', async () => {
    const trigger = document.createElement('button');
    trigger.textContent = '触发';
    document.body.appendChild(trigger);
    trigger.focus();

    const w = mountDrawer();
    await flushPromises();
    w.unmount();
    expect(document.activeElement).toBe(trigger);
  });
});
