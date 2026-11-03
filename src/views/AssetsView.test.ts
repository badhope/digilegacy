import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { createRouter, createMemoryHistory } from 'vue-router';

// AssetsView 是用户查看全部资产的唯一入口：筛选按钮的计数、空态与行渲染
// 直接决定用户以为「自己有没有资产」。这里用真实渲染 + 真实 store 验证。
vi.mock('../api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => false,
}));

import AssetsView from './AssetsView.vue';
import { useAssetsStore } from '../stores/assets';
import { invoke } from '../api/invoke';
import { silenceExpectedErrors } from '../test-utils/silence';
import type { Mock } from 'vitest';

silenceExpectedErrors();

const mockedInvoke = invoke as unknown as Mock;

function makeAsset(over: Record<string, unknown> = {}) {
  return {
    id: 1,
    platform_id: 1,
    platform_name: '支付宝',
    account_identifier: '138****0000',
    account_description: '',
    category: 'property',
    estimated_value: 12000,
    value_currency: 'CNY',
    disposal_method: 'inherit',
    heir_name: null,
    heir_contact: null,
    notes: '',
    has_credential: false,
    is_confirmed: true,
    created_at: '',
    updated_at: '',
    ...over,
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: { template: '<div/>' } },
      { path: '/assets', component: AssetsView },
      { path: '/assets/:id', component: { template: '<div/>' } },
      { path: '/discover', component: { template: '<div/>' } },
    ],
  });
  await router.push('/assets');
  await router.isReady();
  const pinia = createPinia();
  setActivePinia(pinia);
  const w = mount(AssetsView, { global: { plugins: [pinia, router] } });
  return w;
}

beforeEach(() => {
  setActivePinia(createPinia());
  // onMounted 会拉取列表；默认给一个成功的空响应，避免组件走异常分支污染断言
  mockedInvoke.mockReset();
  mockedInvoke.mockResolvedValue({ success: true, message: '', data: [] });
});

describe('AssetsView 渲染', () => {
  it('空态：显示引导卡片而不是空白页', async () => {
    const w = await mountView();
    await flushPromises();
    expect(w.text()).toContain('还没有登记任何数字资产');
    expect(w.text()).toContain('共 0 项');
  });

  it('加载中显示骨架屏而不是空态（避免闪烁）', async () => {
    const store = useAssetsStore();
    const pinia = createPinia();
    setActivePinia(pinia);
    // 手动把 loading 置 true，验证骨架屏渲染
    store.loading = true;
    const w = await mountView();
    expect(w.find('[aria-busy="true"]').exists()).toBe(true);
  });

  it('有数据时渲染资产行、平台名与格式化金额', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [makeAsset()] as never;
    await w.vm.$nextTick();

    expect(w.text()).toContain('支付宝');
    expect(w.text()).toContain('¥1.2万');
    expect(w.text()).toContain('共 1 项');
    expect(w.findAll('.asset-row')).toHaveLength(1);
  });

  it('类别筛选按钮显示各自计数', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [
      makeAsset({ id: 1, category: 'property' }),
      makeAsset({ id: 2, category: 'personality' }),
      makeAsset({ id: 3, category: 'personality' }),
    ] as never;
    await w.vm.$nextTick();

    const text = w.text();
    expect(text).toContain('财产型 (1)');
    expect(text).toContain('人格型 (2)');
    expect(text).toContain('混合型 (0)');
  });

  it('点击筛选后只渲染对应类别', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [
      makeAsset({ id: 1, category: 'property', platform_name: '支付宝' }),
      makeAsset({ id: 2, category: 'personality', platform_name: '微信' }),
    ] as never;
    await w.vm.$nextTick();

    const buttons = w.findAll('button');
    await buttons[2].trigger('click');

    expect(w.findAll('.asset-row')).toHaveLength(1);
    expect(w.text()).toContain('微信');
    expect(w.text()).not.toContain('支付宝');
  });

  it('未设定处置的资产标「待设定」而非其它文案（文案漂移回归）', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [makeAsset({ disposal_method: 'no_action' })] as never;
    await w.vm.$nextTick();

    expect(w.text()).toContain('待设定');
  });

  it('已存凭据的资产显示加密标记', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [makeAsset({ has_credential: true })] as never;
    await w.vm.$nextTick();

    expect(w.text()).toContain('已存凭据');
  });

  it('「全部」必须与顶部计数一致（含未确认资产）', async () => {
    // 变异守卫：把 filter==='all' 改成只返回 is_confirmed 资产时，
    // 顶部显示「共 2 项」但列表只有 1 行，自相矛盾。
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [
      makeAsset({ id: 1, is_confirmed: true, platform_name: '支付宝' }),
      makeAsset({ id: 2, is_confirmed: false, platform_name: '微信' }),
    ] as never;
    await w.vm.$nextTick();

    expect(w.text()).toContain('共 2 项');
    expect(w.findAll('.asset-row')).toHaveLength(2);
  });

  it('无估值显示占位符而不是 ¥0', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [makeAsset({ estimated_value: null })] as never;
    await w.vm.$nextTick();

    expect(w.text()).toContain('-');
  });

  it('加载失败时显示错误与重试，而不是「还没有登记任何数字资产」', async () => {
    // 历史缺陷：store 吞掉异常后 assets 保持 []，视图把失败当空态，
    // 用户会以为资产全部丢失——遗产类产品里这是最危险的误导。
    mockedInvoke.mockRejectedValue(new Error('数据库被占用'));
    const w = await mountView();
    await flushPromises();

    expect(w.text()).toContain('资产列表加载失败');
    expect(w.text()).toContain('重试');
    expect(w.text()).not.toContain('还没有登记任何数字资产');
  });

  it('搜索按平台名/账号过滤，且无结果时提示是筛选结果', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [
      makeAsset({ id: 1, platform_name: '支付宝', account_identifier: 'alipay-user' }),
      makeAsset({ id: 2, platform_name: '微信', account_identifier: 'wechat-user' }),
    ] as never;
    await w.vm.$nextTick();

    const input = w.find('input[type="search"]');
    expect(input.exists(), '搜索框必须存在').toBe(true);
    await input.setValue('wechat');
    expect(w.findAll('.asset-row')).toHaveLength(1);
    expect(w.text()).toContain('微信');

    await input.setValue('不存在的关键词');
    expect(w.findAll('.asset-row')).toHaveLength(0);
    expect(w.text()).toContain('没有匹配的资产');
    expect(w.text()).not.toContain('还没有登记任何数字资产');
  });

  it('排序按价值从高到低', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [
      makeAsset({ id: 1, platform_name: '低价', estimated_value: 10 }),
      makeAsset({ id: 2, platform_name: '高价', estimated_value: 9999 }),
    ] as never;
    await w.vm.$nextTick();

    const select = w.find('select[aria-label="排序方式"]');
    await select.setValue('value');
    const rows = w.findAll('.asset-row');
    expect(rows[0].text()).toContain('高价');
  });
});

describe('待完善筛选（把「家人拿到也办不了」的资产集中起来）', () => {
  async function mountWith(rows: Record<string, unknown>[]) {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = rows.map((r, i) => makeAsset({ id: i + 1, ...r })) as never;
    await w.vm.$nextTick();
    return w;
  }

  it('三条缺陷都算待完善：无继承人 / 无标识 / 需交接但无凭据', async () => {
    const w = await mountWith([
      { heir_name: '王五', account_identifier: 'acc', has_credential: true }, // 完整
      { heir_name: null },                                                     // 无继承人
      { account_identifier: '' },                                              // 无标识
      { has_credential: false },                                               // 无凭据
    ]);
    const btn = w.findAll('button').find(b => b.text().includes('待完善'))!;
    expect(btn, '待完善筛选按钮必须存在').toBeTruthy();
    expect(btn.text(), '应识别出 3 条').toContain('(3)');
  });

  it('点选后只显示待完善资产，再点一次恢复全部', async () => {
    const w = await mountWith([
      { heir_name: '王五', account_identifier: 'acc', has_credential: true },
      { heir_name: null },
    ]);
    const btn = () => w.findAll('button').find(b => b.text().includes('待完善'))!;
    await btn().trigger('click');
    await flushPromises();
    expect(w.findAll('.asset-row').length).toBe(1);
    await btn().trigger('click');
    await flushPromises();
    expect(w.findAll('.asset-row').length).toBe(2);
  });

  it('全部完整时按钮禁用，避免点了没反应', async () => {
    const w = await mountWith([
      { heir_name: '王五', account_identifier: 'acc', has_credential: true },
    ]);
    const btn = w.findAll('button').find(b => b.text().includes('待完善'))!;
    expect(btn.attributes('disabled')).toBeDefined();
  });

  it('「变现分配」不要求继承人，不应算作待完善', async () => {
    const w = await mountWith([
      { disposal_method: 'liquidate', heir_name: null, has_credential: true },
    ]);
    const btn = w.findAll('button').find(b => b.text().includes('待完善'))!;
    expect(btn.text()).toContain('(0)');
  });
});

describe('行内缺口标签（不点进详情也要看得出缺什么）', () => {
  it('无继承人的资产行显示「缺继承人」', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [makeAsset({ id: 1, heir_name: null })] as never;
    await w.vm.$nextTick();
    expect(w.text()).toContain('缺继承人');
  });

  it('无账号标识显示「缺账号标识」', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [makeAsset({ id: 1, account_identifier: '' })] as never;
    await w.vm.$nextTick();
    expect(w.text()).toContain('缺账号标识');
  });

  it('需交接但无凭据显示「缺凭据」', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [makeAsset({ id: 1, has_credential: false })] as never;
    await w.vm.$nextTick();
    expect(w.text()).toContain('缺凭据');
  });

  it('完整资产不显示任何缺口标签', async () => {
    const w = await mountView();
    const store = useAssetsStore();
    store.assets = [makeAsset({ id: 1, heir_name: '王五', account_identifier: 'a', has_credential: true })] as never;
    await w.vm.$nextTick();
    expect(w.text()).not.toContain('缺继承人');
    expect(w.text()).not.toContain('缺账号标识');
    expect(w.text()).not.toContain('缺凭据');
  });
});
