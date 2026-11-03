import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';

// OnboardingTour 是首屏引导：9 步、键盘导航、焦点归还、localStorage 标记。
// 这些行为直接影响新用户能否顺利进入产品；键盘路径必须可用。
import OnboardingTour from './OnboardingTour.vue';

function mountTour() {
  return mount(OnboardingTour, {
    attachTo: document.body,
    global: { stubs: {} },
  });
}

beforeEach(() => {
  localStorage.clear();
  document.body.innerHTML = '';
});

describe('步骤渲染', () => {
  it('首步是欢迎页，居中显示（无目标元素）', () => {
    const w = mountTour();
    expect(w.text()).toContain('欢迎使用「归零」');
    expect(w.text()).toContain('跳过');
  });

  it('共 9 步，最后一步是帮助提示', async () => {
    const w = mountTour();
    // 点 8 次下一步到末尾
    for (let i = 0; i < 8; i++) {
      const btn = w.findAll('button').find(b => b.text().includes('下一步'));
      expect(btn, `第 ${i} 步应有下一步按钮`).toBeTruthy();
      await btn!.trigger('click');
      await flushPromises();
    }
    expect(w.text()).toContain('需要帮助');
  });

  it('最后一步按钮文案变为「完成」', async () => {
    const w = mountTour();
    for (let i = 0; i < 8; i++) {
      await w.findAll('button').find(b => b.text().includes('下一步'))!.trigger('click');
      await flushPromises();
    }
    expect(w.findAll('button').some(b => b.text().includes('完成'))).toBe(true);
  });
});

describe('导航', () => {
  it('上一步在首步不可用（不渲染或其 disabled）', () => {
    const w = mountTour();
    const prev = w.findAll('button').find(b => b.text().includes('上一步'));
    expect(prev).toBeUndefined();
  });

  it('前进再后退回到同一标题', async () => {
    const w = mountTour();
    const first = w.find('.tour-card').text();

    await w.findAll('button').find(b => b.text().includes('下一步'))!.trigger('click');
    await flushPromises();
    expect(w.find('.tour-card').text()).not.toBe(first);

    const prev = w.findAll('button').find(b => b.text().includes('上一步'));
    expect(prev, '第二步应出现上一步').toBeTruthy();
    await prev!.trigger('click');
    await flushPromises();
    expect(w.find('.tour-card').text()).toContain('欢迎使用「归零」');
  });


  it('最后一步点「完成」关闭并写 localStorage 标记', async () => {
    const w = mountTour();
    for (let i = 0; i < 8; i++) {
      await w.findAll('button').find(b => b.text().includes('下一步'))!.trigger('click');
      await flushPromises();
    }
    const done = w.findAll('button').find(b => b.text().includes('完成'));
    expect(done, '最后一步应有完成按钮').toBeTruthy();
    await done!.trigger('click');
    await flushPromises();
    expect(w.emitted('close')).toBeTruthy();
    expect(localStorage.getItem('digilegacy_onboarding_done')).toBe('1');
  });

  it('点「跳过」关闭并写入 localStorage 标记', async () => {
    const w = mountTour();
    await w.findAll('button').find(b => b.text().includes('跳过'))!.trigger('click');
    await flushPromises();

    expect(w.emitted('close')).toBeTruthy();
    expect(localStorage.getItem('digilegacy_onboarding_done')).toBe('1');
  });

  it('点遮罩也会关闭（避免用户被困）', async () => {
    const w = mountTour();
    const blocker = w.find('.tour-blocker');
    expect(blocker.exists(), '遮罩层必须存在').toBe(true);
    await blocker.trigger('click');
    await flushPromises();
    expect(w.emitted('close')).toBeTruthy();
  });
});

describe('键盘操作', () => {
  it('ArrowRight 前进、ArrowLeft 后退', async () => {
    const w = mountTour();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    await flushPromises();
    expect(w.find('.tour-card').text()).toContain('资产发现');

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft' }));
    await flushPromises();
    expect(w.find('.tour-card').text()).toContain('欢迎使用「归零」');
  });

  it('Escape 关闭并写标记', async () => {
    const w = mountTour();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await flushPromises();
    expect(w.emitted('close')).toBeTruthy();
    expect(localStorage.getItem('digilegacy_onboarding_done')).toBe('1');
  });

  it('Enter 等价于下一步', async () => {
    const w = mountTour();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    await flushPromises();
    expect(w.find('.tour-card').text()).toContain('资产发现');
  });
});

describe('焦点与清理', () => {
  it('挂载后焦点在引导卡片内', async () => {
    const w = mountTour();
    await flushPromises();
    const card = w.find('.tour-card').element;
    expect(document.activeElement === card || card.contains(document.activeElement)).toBe(true);
  });

  it('卸载时移除全局键盘监听（不残留）', async () => {
    const w = mountTour();
    const removeSpy = vi.spyOn(window, 'removeEventListener');
    w.unmount();
    expect(removeSpy).toHaveBeenCalledWith('keydown', expect.any(Function));
    removeSpy.mockRestore();
  });

  it('卸载后按方向键不再改变任何东西（监听已移除）', async () => {
    const w = mountTour();
    w.unmount();
    // 不应抛错
    expect(() => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }))).not.toThrow();
  });
});

describe('卡片定位分支', () => {
  function injectTarget(left: number, top: number) {
    const el = document.createElement('div');
    el.setAttribute('data-tour', 'nav-discover');
    el.getBoundingClientRect = () => ({
      left, top, right: left + 120, bottom: top + 40, width: 120, height: 40,
      x: left, y: top, toJSON() { return {}; },
    }) as DOMRect;
    document.body.appendChild(el);
    return el;
  }

  async function advanceToNav(w: { vm: { $nextTick: () => Promise<void> } }) {
    // 首步是欢迎页（无目标），前进到第 2 步即 nav-discover
    const btn = document.querySelector('.tour-actions .btn-primary') as HTMLButtonElement;
    btn.click();
    await w.vm.$nextTick();
  }

  it('目标贴近左缘时卡片放在目标右侧', async () => {
    injectTarget(100, 200);
    const w = mountTour();
    await advanceToNav(w);
    const card = document.querySelector('.tour-card') as HTMLElement;
    expect(card.style.left).toBe('236px'); // right(220) + 16
  });

  it('目标在下半区时卡片放在目标下方', async () => {
    injectTarget(500, 200);
    const w = mountTour();
    await advanceToNav(w);
    const card = document.querySelector('.tour-card') as HTMLElement;
    expect(card.style.left).toBe('500px');
    expect(card.style.top).toBe('254px'); // bottom(240) + 14
  });

  it('目标贴近底边时卡片放在目标上方（兜底分支）', async () => {
    injectTarget(500, 700);
    const w = mountTour();
    await advanceToNav(w);
    const card = document.querySelector('.tour-card') as HTMLElement;
    expect(card.style.top).toBe('470px'); // 700 - 230 上移（兜底分支）
  });
});
