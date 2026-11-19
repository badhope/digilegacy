import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { createRouter, createMemoryHistory } from 'vue-router';
import type { Mock } from 'vitest';

// DashboardView 是登录后的首屏：显示名、统计数字、最近资产。
// 逻辑不复杂，但「数据为空」和「有数据」两种渲染都必须正确。
vi.mock('../api/invoke', () => ({ invoke: vi.fn(), isTauriEnv: () => false }));

import DashboardView from './DashboardView.vue';
import { invoke } from '../api/invoke';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();
const mockedInvoke = invoke as unknown as Mock;

async function mountDash(opts: {
  stats?: Record<string, unknown> | null;
  user?: Record<string, unknown> | null;
  assets?: unknown[];
} = {}) {
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_dashboard_stats') {
      return { success: true, message: '', data: opts.stats ?? null };
    }
    if (cmd === 'get_user_profile') {
      return { success: true, message: '', data: opts.user ?? null };
    }
    if (cmd === 'get_all_assets') return { success: true, message: '', data: opts.assets ?? [] };
    if (cmd === 'get_check_in_status') return { success: true, message: '', data: null };
    if (cmd === 'get_dms_status') return { success: true, message: '', data: null };
    return { success: true, message: '', data: null };
  });
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: { template: '<div/>' } },
      { path: '/dashboard', component: DashboardView },
      { path: '/discover', component: { template: '<div/>' } },
      { path: '/report', component: { template: '<div/>' } },
      { path: '/assets', component: { template: '<div/>' } },
      { path: '/assets/:id', component: { template: '<div/>' } },
      { path: '/dms', component: { template: '<div/>' } },
      { path: '/profile', component: { template: '<div/>' } },
    ],
  });
  await router.push('/dashboard');
  await router.isReady();
  const pinia = createPinia();
  setActivePinia(pinia);
  const w = mount(DashboardView, { global: { plugins: [pinia, router] } });
  for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }
  return w;
}

beforeEach(() => { setActivePinia(createPinia()); mockedInvoke.mockReset(); });

describe('DashboardView', () => {
  it('无用户档案时显示「朋友」', async () => {
    const w = await mountDash({ user: null });
    expect(w.text()).toContain('朋友');
  });

  it('有用户档案时显示真实姓名', async () => {
    const w = await mountDash({ user: { full_name: '张三' } });
    expect(w.text()).toContain('张三');
    expect(w.text()).not.toContain('你好，朋友');
  });

  it('姓名为空串时回退到「朋友」（不能渲染空白）', async () => {
    const w = await mountDash({ user: { full_name: '' } });
    expect(w.text()).toContain('朋友');
  });

  it('无统计数据时前三项显示 0，完成度显示占位符而不是 0%', async () => {
    const w = await mountDash({ stats: null });
    const values = w.findAll('.stat-value').map(v => v.text());
    expect(values[0]).toBe('0');
    expect(values[1]).toContain('0');
    expect(values[2]).toBe('0');
    // 0 资产时 0% 会被误读成「做得很差」，实际是「还没开始」
    expect(values[3], '空库完成度用占位符').toBe('—');
  });

  it('有统计数据时按格式化规则展示', async () => {
    const w = await mountDash({
      stats: {
        total_assets: 4, total_estimated_value: 16000,
        disposal_planned_count: 4, completion_percentage: 100, issues: [],
      },
    });
    const values = w.findAll('.stat-value').map(v => v.text());
    expect(values[0]).toBe('4');
    expect(values[1]).toContain('1.6万');
    expect(values[2]).toBe('4');
    expect(values[3]).toBe('100%');
  });

  it('最近资产最多显示 5 条', async () => {
    const many = Array.from({ length: 8 }, (_, i) => ({
      id: i + 1, platform_id: 1, platform_name: `平台${i}`,
      account_identifier: 'a', account_description: '', category: 'property',
      estimated_value: 10, value_currency: 'CNY', disposal_method: 'inherit',
      heir_name: null, heir_contact: null, notes: '', has_credential: false,
      is_confirmed: true, created_at: '', updated_at: '',
    }));
    const w = await mountDash({ assets: many });
    // 只验证渲染成功且不抛错；切片逻辑由 store 测试覆盖
    expect(w.text()).toContain('平台0');
  });
});

describe('完成度四维分解（旧口径会虚高）', () => {
  it('处置已定但继承人未填时，不得显示为完成', async () => {
    const w = await mountDash({
      stats: {
        total_assets: 4,
        disposal_planned_count: 4,
        heir_assigned_count: 0,
        identifier_ready_count: 4,
        trigger_ready: false,
        trigger_channel: 'none',
        dms_enabled: false,
        completion_percentage: 25,
        total_estimated_value: 0, issues: [],
      },
    });
    // 四个检查项必须都渲染出来，用户能看到「卡在哪一步」
    const items = w.findAll('.check-item');
    expect(items.length).toBe(4);
    const done = w.findAll('.check-item.done');
    expect(done.length, '继承人与触发链都未完成，最多只有两项达标').toBeLessThanOrEqual(2);
    expect(w.text()).toContain('继承人有名有姓');
  });

  it('触发链开启但没有可用通道时，明说家人收不到通知', async () => {
    const w = await mountDash({
      stats: {
        total_assets: 1,
        disposal_planned_count: 1,
        heir_assigned_count: 1,
        identifier_ready_count: 1,
        trigger_ready: false,
        trigger_channel: 'none',
        dms_enabled: true,
        completion_percentage: 75,
        total_estimated_value: 0, issues: [],
      },
    });
    expect(w.text(), '必须明确说明没有通道，而不是显示已开启就算完成')
      .toContain('没有可用通道');
  });

  it('邮件通道可用时展示邮件', async () => {
    const w = await mountDash({
      stats: {
        total_assets: 1,
        disposal_planned_count: 1,
        heir_assigned_count: 1,
        identifier_ready_count: 1,
        trigger_ready: true,
        trigger_channel: 'smtp',
        dms_enabled: true,
        completion_percentage: 100,
        total_estimated_value: 0, issues: [],
      },
    });
    expect(w.text()).toContain('邮件通道可用');
  });
});

describe('触发链达标口径（仅心跳不算家人可达）', () => {
  it('仅心跳可用时，触发链项为警告态而非完成态', async () => {
    const w = await mountDash({
      stats: {
        total_assets: 1,
        disposal_planned_count: 1,
        heir_assigned_count: 1,
        identifier_ready_count: 1,
        trigger_ready: false,
        trigger_channel: 'heartbeat',
        dms_enabled: true,
        completion_percentage: 75,
        total_estimated_value: 0, issues: [],
      },
    });
    const items = w.findAll('.check-item');
    const trigger = items[items.length - 1];
    expect(trigger.classes(), '仅心跳不得显示为已完成').not.toContain('done');
    expect(trigger.classes(), '应显示为警告（已开启但家人收不到）').toContain('warn');
    expect(trigger.text()).toContain('!');
  });

  it('邮件通道可用时才是完成态', async () => {
    const w = await mountDash({
      stats: {
        total_assets: 1,
        disposal_planned_count: 1,
        heir_assigned_count: 1,
        identifier_ready_count: 1,
        trigger_ready: true,
        trigger_channel: 'smtp',
        dms_enabled: true,
        completion_percentage: 100,
        total_estimated_value: 0,
      },
    });
    const items = w.findAll('.check-item');
    const trigger = items[items.length - 1];
    expect(trigger.classes()).toContain('done');
    expect(trigger.text()).toContain('邮件通道可用');
  });
});

describe('待办卡片（差在哪比差多少更有用）', () => {
  const withIssues = (issues: Array<Record<string, unknown>>) => ({
    stats: {
      total_assets: 4,
      disposal_planned_count: 4,
      heir_assigned_count: 4,
      identifier_ready_count: 4,
      trigger_ready: true,
      trigger_channel: 'smtp',
      dms_enabled: true,
      completion_percentage: 100,
      total_estimated_value: 0,
      issues,
    },
  });

  it('没有待办时不渲染卡片', async () => {
    const w = await mountDash(withIssues([]));
    expect(w.text()).not.toContain('还差这几步');
  });

  it('有待办时逐条渲染，并显示条数', async () => {
    const w = await mountDash(withIssues([
      { key: 'missing_heir', severity: 'blocker', count: 3, target: '/assets' },
      { key: 'custody_missing', severity: 'blocker', count: 0, target: '/profile' },
    ]));
    expect(w.text()).toContain('还差这几步');
    expect(w.text()).toContain('3 项资产选了「继承」却没写继承人');
    expect(w.text()).toContain('还没填写备份与恢复码的保管位置');
    expect(w.findAll('.issue-card').length).toBe(2);
  });

  it('严重度对应不同样式类，拦路项用 blocker', async () => {
    const w = await mountDash(withIssues([
      { key: 'missing_heir', severity: 'blocker', count: 1, target: '/assets' },
      { key: 'no_credential', severity: 'info', count: 2, target: '/assets' },
    ]));
    const cards = w.findAll('.issue-card');
    expect(cards[0].classes()).toContain('issue-blocker');
    expect(cards[1].classes()).toContain('issue-info');
  });

  it('点击卡片跳到修复位置', async () => {
    const push = vi.fn();
    const w = await mountDash(withIssues([
      { key: 'custody_missing', severity: 'blocker', count: 0, target: '/profile' },
    ]));
    // 用 router 桩记录跳转
    const vm = w.vm as unknown as { $router: { push: (p: string) => void } };
    vm.$router.push = push;
    await w.find('.issue-card').trigger('click');
    expect(push).toHaveBeenCalledWith('/profile');
  });

  it('未知 key 不崩溃，原样显示键名以便发现漏配', async () => {
    const w = await mountDash(withIssues([
      { key: 'brand_new_issue', severity: 'warn', count: 1, target: '/assets' },
    ]));
    expect(w.text()).toContain('dashboard.issue.brand_new_issue');
  });
});

describe('空库状态（0 资产时不该显示 0%）', () => {
  it('0 资产显示「尚未开始」而不是 0%', async () => {
    const w = await mountDash({
      stats: {
        total_assets: 0,
        disposal_planned_count: 0,
        heir_assigned_count: 0,
        identifier_ready_count: 0,
        trigger_ready: false,
        trigger_channel: 'none',
        dms_enabled: false,
        completion_percentage: 0,
        total_estimated_value: 0,
        issues: [],
      },
    });
    expect(w.text()).toContain('尚未开始');
    // 百分比标签不应出现在空库状态
    const tags = w.findAll('.tag').map(t => t.text());
    expect(tags).not.toContain('待完善');
  });

  it('有资产时才显示百分比与口径说明', async () => {
    const w = await mountDash({
      stats: {
        total_assets: 2,
        disposal_planned_count: 1,
        heir_assigned_count: 2,
        identifier_ready_count: 2,
        trigger_ready: false,
        trigger_channel: 'none',
        dms_enabled: false,
        completion_percentage: 50,
        total_estimated_value: 100,
        issues: [],
      },
    });
    expect(w.text()).toContain('四项检查各占 25%');
    expect(w.text()).not.toContain('尚未开始');
  });
});

describe('家人交付视角', () => {
  it('空库时首屏说明从第一项资产开始，而不是显示完成度', async () => {
    const w = await mountDash({ stats: { total_assets: 0 } });
    expect(w.text()).toContain('从盘点第一项资产开始');
    expect(w.text()).not.toContain('工具包已可交付');
  });

  it('有待办时说明还差几步，并保留可点击卡片', async () => {
    const w = await mountDash({
      stats: {
        total_assets: 3,
        issues: [{ key: 'missing_heir', severity: 'blocker', count: 2, target: '/assets' }],
      },
    });
    expect(w.text()).toContain('还差 1 步就能交给家人');
    expect(w.findAll('.issue-card').length).toBe(1);
  });

  it('无待办且已有资产时说明工具包可交付，并指向报告与向导', async () => {
    const w = await mountDash({ stats: { total_assets: 2, issues: [], trigger_ready: true } });
    expect(w.text()).toContain('工具包已可交付给家人');
    const hrefs = w.findAll('a').map(a => a.attributes('href'));
    expect(hrefs).toContain('/report');
    expect(hrefs).toContain('/heir');
  });

  it('家人会看到什么卡片三项都在，触发链未就绪时如实显示还不能', async () => {
    const w = await mountDash({ stats: { total_assets: 2, heir_assigned_count: 1, trigger_ready: false } });
    expect(w.text()).toContain('家人会看到什么');
    expect(w.text()).toContain('还不能');
  });
});
