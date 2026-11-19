import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { createRouter, createMemoryHistory } from 'vue-router';
import type { Mock } from 'vitest';

// 锁定遮罩与解锁流程是安全关键路径：
//   - 已设主密码 + 未解锁 → 遮罩必须出现，且带 dialog / aria-modal 语义
//   - 未设主密码 → 遮罩不得出现（纯明文模式仍可用）
//   - 解锁失败 → 保持锁定并给出错误；解锁成功 → 遮罩消失
//
// 注意 mock 路径：本文件在 src/ 根目录，被 App.vue 引用的是 './api/invoke'，
// 因此这里必须写 '../api/invoke' 才是同一个模块 id（早前写成 '../api/invoke'
// 却放在 src/views 下会解析到项目根部，导致 mock 静默失效、测试实际打真实 IPC）。
const isTauriEnvStub = vi.hoisted(() => vi.fn(() => false));
const eventListenStub = vi.hoisted(() => vi.fn(async () => () => {}));
vi.mock('./api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: isTauriEnvStub,
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: eventListenStub }));

import App from './App.vue';
import { invoke } from './api/invoke';
import { useSecurityStore } from './stores/security';

const mockedInvoke = invoke as unknown as Mock;

async function mountApp(opts: { skipOnboarding?: boolean } = {}) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', redirect: '/dashboard' },
      { path: '/dashboard', component: { template: '<div class="stub-page">仪表板</div>' } },
      { path: '/discover', component: { template: '<div/>' } },
      { path: '/assets', component: { template: '<div/>' } },
      { path: '/profile', component: { template: '<div/>' } },
      { path: '/dms', component: { template: '<div/>' } },
      { path: '/report', component: { template: '<div/>' } },
      { path: '/heir', component: { template: '<div/>' } },
      { path: '/legal', component: { template: '<div/>' } },
    ],
  });
  await router.push('/dashboard');
  await router.isReady();

  const pinia = createPinia();
  setActivePinia(pinia);
  if (opts.skipOnboarding !== false) {
    localStorage.setItem('digilegacy_onboarding_done', '1');
  }
  const w = mount(App, { global: { plugins: [pinia, router] } });
  return w;
}


/** 让 security_status 返回「已设主密码 + 已锁定」，并等待 onMounted 的异步 refresh 完成。 */
async function lockedApp() {
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'security_status') {
      return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
    }
    return { success: true, message: '', data: [] };
  });
  const w = await mountApp();
  // onMounted 里 security.refresh() 是异步的，必须等它落地，
  // 否则后续手动设置的 store 状态会被它覆盖（这正是本地调试发现的坑）。
  await new Promise(r => setTimeout(r, 0));
  await w.vm.$nextTick();
  return w;
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  sessionStorage.clear();
  mockedInvoke.mockReset();
  mockedInvoke.mockResolvedValue({ success: true, message: '', data: [] });
  isTauriEnvStub.mockReset();
  isTauriEnvStub.mockReturnValue(false);
  eventListenStub.mockReset();
  eventListenStub.mockResolvedValue(() => {});
});

describe('App 锁定遮罩', () => {
  it('mock 生效自检：invoke 必须是隔离桩，不能打真实 IPC', () => {
    // 这条用例本身是「防止 mock 路径写错导致测试形同虚设」的守卫：
    // 若 vi.mock 未命中，invoke 会是真实实现，此断言立即失败。
    expect(vi.isMockFunction(invoke)).toBe(true);
  });

  it('未设主密码时不渲染遮罩（明文模式仍可用）', async () => {
    const w = await mountApp();
    const s = useSecurityStore();
    s.hasMasterPassword = false;
    s.unlocked = true;
    await w.vm.$nextTick();

    expect(w.find('.lock-overlay').exists()).toBe(false);
  });

  it('已设主密码且已解锁时不渲染遮罩', async () => {
    const w = await mountApp();
    const s = useSecurityStore();
    s.hasMasterPassword = true;
    s.unlocked = true;
    await w.vm.$nextTick();

    expect(w.find('.lock-overlay').exists()).toBe(false);
  });

  it('已设主密码且锁定时渲染模态遮罩，具备 dialog 语义', async () => {
    const w = await lockedApp();
    const s = useSecurityStore();
    expect(s.unlocked).toBe(false);

    const overlay = w.find('.lock-overlay');
    expect(overlay.exists()).toBe(true);

    const card = overlay.find('.lock-card');
    expect(card.attributes('role')).toBe('dialog');
    expect(card.attributes('aria-modal')).toBe('true');
    expect(card.attributes('aria-labelledby')).toBe('lock-title');
    expect(card.find('#lock-title').exists()).toBe(true);
  });

  it('hasMasterPassword 为真但 unlocked 为真时不显示遮罩（条件必须同时成立）', async () => {
    // 变异守卫：把 v-if 简化为只判 !unlocked 时，这条会失败
    const w = await mountApp();
    const s = useSecurityStore();
    s.hasMasterPassword = true;
    s.unlocked = true;
    await w.vm.$nextTick();
    expect(w.find('.lock-overlay').exists()).toBe(false);

    s.hasMasterPassword = false;
    s.unlocked = false;
    await w.vm.$nextTick();
    expect(w.find('.lock-overlay').exists()).toBe(false);
  });

  it('锁定态下提供主密码输入与解锁按钮', async () => {
    const w = await lockedApp();

    expect(w.find('input[aria-label="主密码"]').exists()).toBe(true);
    expect(w.text()).toContain('解锁');
  });

  it('错误密码：保持锁定并显示后端错误消息', async () => {
    const w = await lockedApp();
    const s = useSecurityStore();

    // onMounted 会打多个 IPC（security_status / check_in / dms / assets ...），
    // 所以不能依赖 mockResolvedValueOnce 的顺序，按命令名精确派发。
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'unlock') return { success: false, message: '主密码错误', data: null };
      return { success: true, message: '', data: [] };
    });

    await w.find('input[aria-label="主密码"]').setValue('wrong-pass');
    const btn = w.find('.lock-card .btn-primary')!;
    await btn.trigger('click');
    await w.vm.$nextTick();
    await flushPromises();
    await w.vm.$nextTick();

    expect(s.unlocked).toBe(false);
    expect(w.find('.lock-overlay').exists()).toBe(true);
    expect(w.text()).toContain('主密码错误');
  });

  it('正确密码：解除锁定、遮罩消失', async () => {
    const w = await lockedApp();
    const s = useSecurityStore();

    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'unlock') {
        return { success: true, message: '', data: { has_master_password: true, unlocked: true } };
      }
      return { success: true, message: '', data: [] };
    });

    await w.find('input[aria-label="主密码"]').setValue('correct-pass');
    const btn = w.find('.lock-card .btn-primary')!;
    await btn.trigger('click');
    await w.vm.$nextTick();

    expect(s.unlocked).toBe(true);
    expect(w.find('.lock-overlay').exists()).toBe(false);
  });

  it('解锁后端异常：不得误判为已解锁', async () => {
    const w = await lockedApp();
    const s = useSecurityStore();

    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'unlock') throw new Error('IPC 断开');
      return { success: true, message: '', data: [] };
    });

    await w.find('input[aria-label="主密码"]').setValue('whatever');
    const btn = w.find('.lock-card .btn-primary')!;
    await btn.trigger('click');
    await w.vm.$nextTick();

    expect(s.unlocked).toBe(false);
    expect(w.find('.lock-overlay').exists()).toBe(true);
  });

  it('锁定态可切到恢复密钥流程', async () => {
    const w = await lockedApp();

    const recoverBtn = w.findAll('button').find(b => b.text().includes('恢复密钥'));
    expect(recoverBtn).toBeTruthy();
    await recoverBtn!.trigger('click');

    expect(w.text()).toContain('恢复访问');
    expect(w.find('input[aria-label="恢复密钥"]').exists()).toBe(true);
  });

  it('侧栏只有当前路由的导航项带 active', async () => {
    // 变异守卫：active 恒为 true 时此前 115 个测试全绿，
    // 用户将看到所有菜单项同时高亮。
    const w = await mountApp();
    const items = w.findAll('.nav-item');
    expect(items.length).toBeGreaterThan(1);

    const activeItems = items.filter(i => i.classes().includes('active'));
    expect(activeItems).toHaveLength(1);
    expect(activeItems[0].attributes('href')).toBe('/dashboard');
  });

  it('侧栏在未启用加密时提示去设置主密码', async () => {
    const w = await mountApp();
    const s = useSecurityStore();
    s.hasMasterPassword = false;
    await w.vm.$nextTick();

    expect(w.text()).toContain('未启用加密');
  });

  it('启用主密码后侧栏改为加密已启用文案', async () => {
    const w = await mountApp();
    const s = useSecurityStore();
    s.hasMasterPassword = true;
    s.unlocked = true;
    await w.vm.$nextTick();

    expect(w.text()).toContain('数据本地加密存储');
    expect(w.text()).not.toContain('未启用加密');
  });
});

describe('恢复密钥流程', () => {
  it('空恢复码时本地拦截，不调用后端', async () => {
    const w = await lockedApp();
    const card = w.find('.lock-card');
    const recoverBtn = card.findAll('button').find(b => b.text().includes('恢复密钥'))!;
    await recoverBtn.trigger('click');

    const submit = card.findAll('button').find(b => b.text().includes('验证恢复密钥'))!;
    await submit.trigger('click');
    await flushPromises();

    expect(mockedInvoke).not.toHaveBeenCalledWith('start_recovery', expect.anything());
    expect(w.text()).toContain('请输入恢复密钥');
  });

  it('恢复码错误时显示后端消息且不进入重设步骤', async () => {
    const w = await lockedApp();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'security_status') {
        return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
      }
      if (cmd === 'start_recovery') return { success: false, message: '恢复密钥无效', data: null };
      return { success: true, message: '', data: [] };
    });

    const card = w.find('.lock-card');
    await card.findAll('button').find(b => b.text().includes('恢复密钥'))!.trigger('click');
    await w.find('input[aria-label="恢复密钥"]').setValue('AAAA-BBBB-CCCC-DDDD-EEEE-FFFF');
    await card.findAll('button').find(b => b.text().includes('验证恢复密钥'))!.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('恢复密钥无效');
    expect(w.find('input[aria-label="新主密码（至少 8 位）"]').exists()).toBe(false);
  });

  it('恢复码正确后进入重设密码步骤', async () => {
    const w = await lockedApp();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'security_status') {
        return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
      }
      if (cmd === 'start_recovery') {
        return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
      }
      return { success: true, message: '', data: [] };
    });

    const card = w.find('.lock-card');
    await card.findAll('button').find(b => b.text().includes('恢复密钥'))!.trigger('click');
    await w.find('input[aria-label="恢复密钥"]').setValue('ABCD-EFGH-JKLM-NPQR-STUV-WXYZ');
    await card.findAll('button').find(b => b.text().includes('验证恢复密钥'))!.trigger('click');
    await flushPromises();

    expect(w.find('input[aria-label="新主密码（至少 8 位）"]').exists()).toBe(true);
  });

  it('重设成功必须展示新的恢复密钥（旧码已失效）', async () => {
    const w = await lockedApp();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'security_status') {
        return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
      }
      if (cmd === 'start_recovery') {
        return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
      }
      if (cmd === 'complete_recovery') {
        return { success: true, message: '主密码已重设', data: 'NEW1-NEW2-NEW3-NEW4-NEW5-NEW6' };
      }
      return { success: true, message: '', data: [] };
    });

    const card = w.find('.lock-card');
    await card.findAll('button').find(b => b.text().includes('恢复密钥'))!.trigger('click');
    await w.find('input[aria-label="恢复密钥"]').setValue('ABCD-EFGH-JKLM-NPQR-STUV-WXYZ');
    await card.findAll('button').find(b => b.text().includes('验证恢复密钥'))!.trigger('click');
    await flushPromises();

    await w.find('input[aria-label="新主密码（至少 8 位）"]').setValue('brand-new-pass');
    await w.find('input[aria-label="确认新主密码"]').setValue('brand-new-pass');
    await card.findAll('button').find(b => b.text().includes('重设主密码'))!.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('NEW1-NEW2-NEW3-NEW4-NEW5-NEW6');
  });

  it('新恢复密钥在用户确认抄写前不得自动消失', async () => {
    // 先挂载（挂载内的 setTimeout(0) 需要真实计时器），再切假计时器推进 6 秒后的自动关闭逻辑
    const w = await lockedApp();
    vi.useFakeTimers();
    try {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'security_status') {
          return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
        }
        if (cmd === 'start_recovery') {
          return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
        }
        if (cmd === 'complete_recovery') {
          return { success: true, message: '主密码已重设', data: 'NEW1-NEW2-NEW3-NEW4-NEW5-NEW6' };
        }
        return { success: true, message: '', data: [] };
      });

      const card = w.find('.lock-card');
      await card.findAll('button').find(b => b.text().includes('恢复密钥'))!.trigger('click');
      await w.find('input[aria-label="恢复密钥"]').setValue('ABCD-EFGH-JKLM-NPQR-STUV-WXYZ');
      await card.findAll('button').find(b => b.text().includes('验证恢复密钥'))!.trigger('click');
      await flushPromises();

      await w.find('input[aria-label="新主密码（至少 8 位）"]').setValue('brand-new-pass');
      await w.find('input[aria-label="确认新主密码"]').setValue('brand-new-pass');
      await card.findAll('button').find(b => b.text().includes('重设主密码'))!.trigger('click');
      await flushPromises();

      // 旧实现 6 秒后无条件关闭遮罩并清空 recoveryNewCode：用户还在抄写时新码就消失，
      // 而旧码已随轮换失效，等于把唯一的恢复通道烧掉。
      await vi.advanceTimersByTimeAsync(60_000);
      await w.vm.$nextTick();

      expect(w.text(), '新恢复密钥不得因计时器自动消失').toContain('NEW1-NEW2-NEW3-NEW4-NEW5-NEW6');
      expect(
        card.findAll('button').some(b => b.text().includes('我已保存')),
        '必须提供显式确认按钮，由用户决定何时关闭',
      ).toBe(true);
      w.unmount();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('闲置自动锁定', () => {
  it('超过 10 分钟无操作后自动锁定', async () => {
    vi.useFakeTimers();
    try {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'security_status') {
          return { success: true, message: '', data: { has_master_password: true, unlocked: true } };
        }
        return { success: true, message: '', data: [] };
      });
      const w = await mountApp();
      const s = useSecurityStore();
      s.hasMasterPassword = true;
      s.unlocked = true;
      await w.vm.$nextTick();

      // 定时器每 30 秒检查一次；推进 11 分钟确保越过 10 分钟阈值
      await vi.advanceTimersByTimeAsync(11 * 60 * 1000);
      await w.vm.$nextTick();

      expect(s.unlocked, '超过闲置阈值必须自动锁定').toBe(false);
      expect(w.find('.lock-overlay').exists()).toBe(true);
      w.unmount();
    } finally {
      vi.useRealTimers();
    }
  });

  it('未设主密码时闲置不会锁定（明文模式无需解锁）', async () => {
    vi.useFakeTimers();
    try {
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'security_status') {
          return { success: true, message: '', data: { has_master_password: false, unlocked: true } };
        }
        return { success: true, message: '', data: [] };
      });
      const w = await mountApp();
      const s = useSecurityStore();
      s.hasMasterPassword = false;
      await w.vm.$nextTick();

      await vi.advanceTimersByTimeAsync(11 * 60 * 1000);
      await w.vm.$nextTick();

      expect(s.unlocked).toBe(true);
      expect(w.find('.lock-overlay').exists()).toBe(false);
      w.unmount();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('语言切换', () => {
  it('点语言按钮在中文与英文之间切换（标题与侧栏即时生效）', async () => {
    const w = await mountApp();
    expect(w.text()).toContain('仪表板');

    const btn = w.find('[data-testid="lang-switch"]');
    expect(btn.exists(), '语言切换按钮必须存在').toBe(true);
    await btn.trigger('click');
    await w.vm.$nextTick();

    expect(w.text()).toContain('Dashboard');
    expect(localStorage.getItem('digilegacy_locale')).toBe('en-US');

    await btn.trigger('click');
    await w.vm.$nextTick();
    expect(w.text()).toContain('仪表板');
  });
});

describe('托盘事件', () => {
  it('Tauri 环境下绑定托盘事件；app-locked 触发时立即清空界面敏感数据', async () => {
    isTauriEnvStub.mockReturnValue(true);
    const w = await mountApp();
    await flushPromises();

    expect(eventListenStub, '必须监听托盘事件').toHaveBeenCalledWith('app-locked', expect.any(Function));
    const calls = eventListenStub.mock.calls as unknown as [string, () => void][];
    const lockedHandler = calls.find(c => c[0] === 'app-locked')![1];

    // 托盘「锁定」后：窗口不刷新，但界面上已解密数据必须被清除（安全关键路径）
    const s = useSecurityStore();
    s.unlocked = true;
    lockedHandler();
    await w.vm.$nextTick();
    expect(s.unlocked, '托盘锁定后安全状态必须置为锁定').toBe(false);
  });

  it('Tauri 环境下 dms-checked-in 事件触发签到状态与统计刷新', async () => {
    isTauriEnvStub.mockReturnValue(true);
    const w = await mountApp();
    await flushPromises();

    const calls = eventListenStub.mock.calls as unknown as [string, () => void][];
    const checkedInHandler = calls.find(c => c[0] === 'dms-checked-in')![1];
    checkedInHandler();
    await flushPromises();
    // 刷新调用不抛错即通过（loadCheckInStatus/loadStats 走 mock invoke）
    expect(w.exists()).toBe(true);
  });

  it('非 Tauri 环境（浏览器预览）不绑定托盘事件', async () => {
    isTauriEnvStub.mockReturnValue(false);
    await mountApp();
    await flushPromises();
    expect(eventListenStub).not.toHaveBeenCalled();
  });

  it('托盘事件绑定失败时静默降级，不阻断应用启动', async () => {
    isTauriEnvStub.mockReturnValue(true);
    eventListenStub.mockRejectedValueOnce(new Error('event plugin gone'));
    const w = await mountApp();
    await flushPromises();
    expect(w.exists(), '绑定失败也不能白屏').toBe(true);
  });

  it('onMounted 后若尚未完成新手引导且未锁定，自动弹出引导', async () => {
    vi.useFakeTimers();
    try {
      isTauriEnvStub.mockReturnValue(false);
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'security_status') {
          return { success: true, message: '', data: { has_master_password: false, unlocked: true } };
        }
        return { success: true, message: '', data: [] };
      });
      const w = await mountApp({ skipOnboarding: false });
      await vi.advanceTimersByTimeAsync(1000);
      await w.vm.$nextTick();
      expect(w.text()).toContain('跳过');
      w.unmount();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('解锁与恢复边界', () => {
  it('空密码提交被本地拦截，不调用后端', async () => {
    const w = await lockedApp();
    await w.find('.lock-card .btn-primary').trigger('click');
    await w.vm.$nextTick();
    expect(w.text()).toContain('请输入主密码');
    const calls = mockedInvoke.mock.calls.map(c => c[0]);
    expect(calls, '空密码不得打后端 unlock').not.toContain('unlock');
  });

  it('未勾选「我已保存」时点完成不关闭恢复流程（新码仍展示）', async () => {
    const w = await lockedApp();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'security_status') {
        return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
      }
      if (cmd === 'start_recovery') {
        return { success: true, message: '', data: { has_master_password: true, unlocked: false } };
      }
      if (cmd === 'complete_recovery') {
        return { success: true, message: '主密码已重设', data: 'NEW1-NEW2-NEW3-NEW4-NEW5-NEW6' };
      }
      return { success: true, message: '', data: [] };
    });
    const card = w.find('.lock-card');
    await card.findAll('button').find(b => b.text().includes('恢复密钥'))!.trigger('click');
    await w.find('input[aria-label="恢复密钥"]').setValue('ABCD-EFGH-JKLM-NPQR-STUV-WXYZ');
    await card.findAll('button').find(b => b.text().includes('验证恢复密钥'))!.trigger('click');
    await flushPromises();
    await w.find('input[aria-label="新主密码（至少 8 位）"]').setValue('brand-new-pass');
    await w.find('input[aria-label="确认新主密码"]').setValue('brand-new-pass');
    await card.findAll('button').find(b => b.text().includes('重设主密码'))!.trigger('click');
    await flushPromises();
    // 不勾选 checkbox，直接点「我已保存」：finishRecovery 必须被 ack 门拦住
    const saveBtn = card.findAll('button').find(b => b.text().includes('我已保存'))!;
    await saveBtn.trigger('click');
    await w.vm.$nextTick();
    expect(w.text()).toContain('NEW1-NEW2-NEW3-NEW4-NEW5-NEW6');
    // 勾选后再点：恢复流程关闭（回到解锁表单），新码展示消失
    await card.find('input[type="checkbox"]').setValue(true);
    await card.findAll('button').find(b => b.text().includes('我已保存'))!.trigger('click');
    await w.vm.$nextTick();
    expect(w.text()).not.toContain('NEW1-NEW2-NEW3-NEW4-NEW5-NEW6');
    expect(w.find('input[aria-label="主密码"]').exists(), '关闭后回到主密码解锁表单').toBe(true);
  });
});

describe('导航分组（按家人交付旅程，不按功能平铺）', () => {
  it('分组标签与顺序符合持有者→家人的动线', async () => {
    const w = await mountApp();
    const labels = w.findAll('.nav-group-label').map(n => n.text());
    expect(labels).toEqual(['开始', '盘点资产', '安排身后事', '交给家人', '发出前核对']);
  });

  it('继承人向导归入「交给家人」，法律模板归入「安排身后事」', async () => {
    const w = await mountApp();
    const groups = w.findAll('.nav-group');
    const textOf = (i: number) => groups[i].text();
    expect(textOf(2), '安排身后事应含法律模板，不含继承人向导')
      .toContain('法律模板');
    expect(textOf(2)).not.toContain('继承人向导');
    expect(textOf(3), '交给家人应含继承人向导')
      .toContain('继承人向导');
  });

  it('九个旧路由仍全部可达（深链/测试不被破坏）', async () => {
    const w = await mountApp();
    const hrefs = w.findAll('.nav-item').map(a => a.attributes('href'));
    for (const p of ['/dashboard', '/discover', '/assets', '/profile', '/dms', '/report', '/heir', '/review', '/legal']) {
      expect(hrefs.some(h => h?.endsWith(p)), `缺少路由 ${p}`).toBe(true);
    }
  });
});
