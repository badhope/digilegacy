import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import type { Mock } from 'vitest';

// HeirGuideView 有两个 Tab、难度筛选、类别筛选与分组折叠。
// 这些是真实的数据处理逻辑（groupByCategory / filteredExtendedPlatforms），
// 错一位就会让继承人看到错误的平台清单。
vi.mock('../api/invoke', () => ({ invoke: vi.fn(), isTauriEnv: () => false }));

import HeirGuideView from './HeirGuideView.vue';
import { invoke } from '../api/invoke';
import { usePlatformsStore } from '../stores/platforms';

const mockedInvoke = invoke as unknown as Mock;

function p(id: number, name: string, category: string, over: Record<string, unknown> = {}) {
  return {
    id, name, category, subtype: '', has_property: true, has_personality: false,
    inheritable: true, required_documents: '死亡证明', process_description: '流程',
    estimated_difficulty: '简单', official_policy_url: '', estimated_time_days: '30',
    notes: '', ...over,
  };
}
function ext(id: number, name: string, category: string, over: Record<string, unknown> = {}) {
  return {
    ...p(id, name, category),
    name_en: name, data_source: '', inheritance_form_url: '',
    monetization_path: '', legal_template_ids: '', risk_warnings: '', tags: '',
    last_reviewed: '', ...over,
  };
}

async function mountHeir(platforms: unknown[] = [], extended: unknown[] = []) {
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_all_platforms') return { success: true, message: '', data: platforms };
    if (cmd === 'get_all_platforms_extended') return { success: true, message: '', data: extended };
    return { success: true, message: '', data: null };
  });
  const pinia = createPinia();
  setActivePinia(pinia);
  const w = mount(HeirGuideView, { global: { plugins: [pinia] } });
  for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }
  return w;
}

beforeEach(() => { setActivePinia(createPinia()); mockedInvoke.mockReset(); });

describe('渲染与 Tab', () => {
  it('默认显示继承指引 Tab 且为选中态', async () => {
    const w = await mountHeir([p(1, '支付宝', '支付金融')], [ext(1, '支付宝', '支付金融', { monetization_path: 'x' })]);
    const tabs = w.findAll('[role="tab"]');
    expect(tabs.length).toBe(2);
    expect(tabs[0].attributes('aria-selected')).toBe('true');
    expect(tabs[1].attributes('aria-selected')).toBe('false');
  });

  it('切换到变现 Tab 后选中态互换', async () => {
    const w = await mountHeir([p(1, '支付宝', '支付金融')], [ext(1, '支付宝', '支付金融', { monetization_path: 'x' })]);
    const tabs = w.findAll('[role="tab"]');
    await tabs[1].trigger('click');
    await w.vm.$nextTick();
    expect(tabs[1].attributes('aria-selected')).toBe('true');
    expect(tabs[0].attributes('aria-selected')).toBe('false');
  });

  it('显示重要前提与流程总览', async () => {
    const w = await mountHeir([p(1, '支付宝', '支付金融')], []);
    expect(w.text()).toContain('重要前提');
    expect(w.text()).toContain('继承流程总览');
  });
});

describe('难度筛选', () => {
  it('默认 all 显示全部平台', async () => {
    const w = await mountHeir([
      p(1, '支付宝', '支付金融', { estimated_difficulty: '简单' }),
      p(2, '微信', '社交通讯', { estimated_difficulty: '困难' }),
    ], []);
    const store = usePlatformsStore();
    expect(store.platforms).toHaveLength(2);
    // 分组按类别，两个类别 → 两个 CollapseGroup
    expect(w.findAll('.collapse-group').length).toBe(2);
  });

  it('选「困难」后只保留该难度的平台', async () => {
    const w = await mountHeir([
      p(1, '支付宝', '支付金融', { estimated_difficulty: '简单' }),
      p(2, '微信', '社交通讯', { estimated_difficulty: '困难' }),
    ], []);
    const hard = w.findAll('button').find(b => b.text() === '困难');
    expect(hard, '难度筛选按钮必须存在').toBeTruthy();
    await hard!.trigger('click');
    await flushPromises();
    expect(w.findAll('.collapse-group').length).toBe(1);
    expect(w.text()).toContain('微信');
    expect(w.text()).not.toContain('支付宝');
  });

  it('平台为空时显示空态而不是崩溃', async () => {
    const w = await mountHeir([], []);
    expect(w.findAll('.collapse-group').length).toBe(0);
    expect(w.text()).toContain('重要前提');
  });
});

describe('变现场景筛选', () => {
  it('只显示有变现/继承信息的平台（过滤空壳）', async () => {
    const w = await mountHeir([], [
      ext(1, '支付宝', '支付金融', { monetization_path: '可提现' }),
      ext(2, '空壳平台', '支付金融'), // 所有扩展字段为空 → 必须被过滤掉
    ]);
    const tabs = w.findAll('[role="tab"]');
    await tabs[1].trigger('click');
    // 切换 Tab 触发 watch → loadPlatformsExtended（异步），需要多轮 flush
    for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }
    // 内容在 CollapseGroup 内，默认折叠 → 必须先展开
    const expand = w.findAll('button').find(b => b.text() === '展开全部');
    expect(expand, '展开全部按钮必须存在').toBeTruthy();
    await expand!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('支付宝');
    expect(w.text).not.toContain('空壳平台');
  });

  it('类别筛选只保留该类别', async () => {
    const w = await mountHeir([], [
      ext(1, '支付宝', '支付金融', { monetization_path: 'x' }),
      ext(2, '微信', '社交通讯', { monetization_path: 'y' }),
    ]);
    const tabs = w.findAll('[role="tab"]');
    await tabs[1].trigger('click');
    for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }

    // 分类筛选按钮显示的是**当前语言**的分类名（中文界面下「支付金融」→「金融资产」），
    // 不再是原始数据值。历史上这里断言原始中文值，等于把「界面直接打印数据串」
    // 这个缺陷固化成了期望行为。
    const catBtn = w.findAll('button').find(b => b.text() === '金融资产');
    expect(catBtn, '类别筛选按钮必须存在且显示译名').toBeTruthy();
    await catBtn!.trigger('click');
    await flushPromises();

    const expand = w.findAll('button').find(b => b.text() === '展开全部');
    if (expand) { await expand.trigger('click'); await flushPromises(); }
    // 只断言分组标题：页面顶部的类别筛选按钮里本来就可能有「社交通讯」等字样，
    // 用整页 text 判断会误判。
    const groupTitles = w.findAll('.collapse-title').map(t => t.text());
    expect(groupTitles).toEqual(['金融资产']);
  });
});

describe('展开/收起全部', () => {
  it('展开全部后所有分组打开，再点收起全部后全部关闭', async () => {
    const w = await mountHeir([
      p(1, '支付宝', '支付金融'),
      p(2, '微信', '社交通讯'),
    ], []);
    const expand = w.findAll('button').find(b => b.text() === '展开全部');
    expect(expand, '展开全部按钮必须存在').toBeTruthy();

    await expand!.trigger('click');
    await flushPromises();
    expect(w.findAll('.collapse-body').length).toBe(2);

    const collapse = w.findAll('button').find(b => b.text() === '收起全部');
    expect(collapse, '收起全部按钮必须存在').toBeTruthy();
    await collapse!.trigger('click');
    await flushPromises();
    expect(w.findAll('.collapse-body').length).toBe(0);
  });
});

describe('政策溯源状态（未核实不得冒充官方口径）', () => {
  const base = (over: Record<string, unknown>) => ext(1, '支付宝', '支付金融', { monetization_path: '余额可继承', ...over });

  it('未核实条目必须显示提示，而不是来源', async () => {
    const w = await mountHeir(
      [p(1, '支付宝', '支付金融')],
      [base({ data_source: '未核实（待人工核对官方公告）', last_reviewed: '' })],
    );
    const tabs = w.findAll('[role="tab"]');
    await tabs[1].trigger('click');
    // 切 Tab 会触发 watch → 异步拉扩展列表，再渲染一轮
    for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }
    // 分组默认折叠，先展开才看得到卡片内容
    const expand = w.findAll('button').find(b => b.text() === '展开全部');
    if (expand) { await expand.trigger('click'); await flushPromises(); }
    expect(w.text(), '未核实必须明说').toContain('尚未人工核对');
  });

  it('有出处且已核对时显示来源与核对月份', async () => {
    const w = await mountHeir(
      [p(1, '支付宝', '支付金融')],
      [base({ data_source: '支付宝官方帮助中心', last_reviewed: '2026-10' })],
    );
    const tabs = w.findAll('[role="tab"]');
    await tabs[1].trigger('click');
    for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }
    const expand = w.findAll('button').find(b => b.text() === '展开全部');
    if (expand) { await expand.trigger('click'); await flushPromises(); }
    expect(w.text()).toContain('支付宝官方帮助中心');
    expect(w.text()).toContain('2026-10');
    expect(w.text()).not.toContain('尚未人工核对');
  });
});
