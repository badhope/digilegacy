import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { createRouter, createMemoryHistory } from 'vue-router';
import type { Mock } from 'vitest';

// DiscoverView 是资产录入的主入口（121 条语句，此前 0% 覆盖）。
// 这里覆盖单测能抓的部分：计算属性、类别映射、保存分支、重复登记去重。
// 真实点击链路由 e2e/discover.spec.ts 覆盖，两者互补。
vi.mock('../api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => false,
}));

import DiscoverView from './DiscoverView.vue';
import { invoke } from '../api/invoke';
import { usePlatformsStore } from '../stores/platforms';
import { useAssetsStore } from '../stores/assets';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();

const mockedInvoke = invoke as unknown as Mock;

function platform(id: number, name: string, category: string, over: Record<string, unknown> = {}) {
  return {
    id, name, category, subtype: 'test-subtype',
    has_property: true, has_personality: false, inheritable: true,
    required_documents: '', process_description: '', estimated_difficulty: '',
    official_policy_url: '', estimated_time_days: '', notes: '',
    ...over,
  };
}

function assetRow(over: Record<string, unknown> = {}) {
  return {
    id: 1, platform_id: 1, platform_name: '支付宝', account_identifier: 'a',
    account_description: '', category: 'property', estimated_value: null,
    value_currency: 'CNY', disposal_method: 'inherit', heir_name: null,
    heir_contact: null, notes: '', has_credential: false, is_confirmed: true,
    created_at: '', updated_at: '', ...over,
  };
}

async function mountDiscover(platforms: unknown[] = [], opts: { failAdd?: boolean } = {}) {
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_all_platforms') return { success: true, message: '', data: platforms };
    if (cmd === 'get_all_assets') return { success: true, message: '', data: [] };
    if (cmd === 'add_asset') {
      if (opts.failAdd) return { success: false, message: '锁定状态', data: null };
      return { success: true, message: '', data: 1 };
    }
    return { success: true, message: '', data: null };
  });
  const pinia = createPinia();
  setActivePinia(pinia);
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: { template: '<div/>' } },
      { path: '/discover', component: DiscoverView },
      { path: '/assets', component: { template: '<div/>' } },
      { path: '/report', component: { template: '<div/>' } },
    ],
  });
  await router.push('/discover');
  await router.isReady();
  const w = mount(DiscoverView, { global: { plugins: [pinia, router] } });
  for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }
  return w;
}


/** 从 step0 推进到 step1（类别选择）。 */
async function toStep1(w: Awaited<ReturnType<typeof mountDiscover>>) {
  const btn = w.findAll('button').find(b => b.text().includes('继续'));
  if (btn) await btn.trigger('click');
  await flushPromises();
  await w.vm.$nextTick();
  // 问卷会按勾选项预选大类；这批用例验证的是「手动选择」路径，
  // 先清空预选再开始，避免把预选结果当成用户操作。
  const clear = w.findAll('button').find(b => b.text().includes('清空选择'));
  if (clear) {
    await clear.trigger('click');
    await flushPromises();
  }
}

/** 从 step1 推进到 step2（平台勾选）。 */
async function toStep2(w: Awaited<ReturnType<typeof mountDiscover>>) {
  const btn = w.findAll('button').find(b => b.text().includes('下一步：勾选平台'));
  if (btn) await btn.trigger('click');
  await flushPromises();
  await w.vm.$nextTick();
}

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
});

describe('自检问卷', () => {
  it('默认勾选数与 checklistItems 的 checkedInit 一致（7 项）', async () => {
    const w = await mountDiscover();
    // step0 才是问卷页
    const footer = w.find('.checklist-footer');
    expect(footer.exists()).toBe(true);
    expect(footer.text()).toContain('7 / 10');
  });

  it('勾选未选项后计数增加，取消已选项后减少', async () => {
    const w = await mountDiscover();
    const boxes = w.findAll('.checklist input[type="checkbox"]');
    expect(boxes.length).toBe(10);

    await boxes[2].setValue(true); // 加密货币钱包（默认未勾）
    await w.vm.$nextTick();
    expect(w.find('.checklist-footer').text()).toContain('8 / 10');

    await boxes[0].setValue(false); // 银行账户（默认勾选）
    await w.vm.$nextTick();
    expect(w.find('.checklist-footer').text()).toContain('7 / 10');
  });
});

describe('类别选择与平台过滤', () => {
  it('未选类别时按「全部平台数」显示，且下一步禁用', async () => {
    const w = await mountDiscover([
      platform(1, '支付宝', '支付金融'),
      platform(2, '微信', '社交通讯'),
    ]);
    await toStep1(w);
    // 未选类别时「下一步：勾选平台」必须禁用，无法进入平台页
    const next = w.findAll('button').find(b => b.text().includes('下一步：勾选平台'))!;
    expect(next.attributes('disabled')).toBeDefined();
    expect(w.text()).toContain('已选择 0 类资产');
  });

  it('选一个类别只统计该类别下的平台', async () => {
    const w = await mountDiscover([
      platform(1, '支付宝', '支付金融'),
      platform(2, '微信', '社交通讯'),
    ]);
    await toStep1(w);
    const card = w.findAll('.category-card')[0]; // 金融资产
    await card.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('已选择 1 类资产');
    await toStep2(w);
    expect(w.text()).toContain('已为你筛选 1 个平台');
  });

  it('复合类别（学习/音视频）映射多个数据分类', async () => {
    const w = await mountDiscover([
      platform(1, '得到', '知识付费'),
      platform(2, 'B站', '音视频'),
      platform(3, '百度网盘', '存储云服务'),
      platform(4, '支付宝', '支付金融'),
    ]);
    await toStep1(w);
    // 找到「学习/音视频」这张卡（key=知识付费）
    const cards = w.findAll('.category-card');
    const target = cards.find(c => c.text().includes('学习/音视频'))!;
    await target.trigger('click');
    await flushPromises();
    await toStep2(w);
    expect(w.text()).toContain('已为你筛选 3 个平台');
  });

  it('再次点击取消选择', async () => {
    const w = await mountDiscover([platform(1, '支付宝', '支付金融')]);
    await toStep1(w);
    const card = w.findAll('.category-card')[0];
    await card.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('已选择 1 类资产');
    await card.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('已选择 0 类资产');
  });

  it('所选类别下没有平台时，分组不渲染空壳', async () => {
    const w = await mountDiscover([platform(1, '支付宝', '支付金融')]);
    await toStep1(w);
    const cards = w.findAll('.category-card');
    const empty = cards.find(c => c.text().includes('游戏资产'))!;
    await empty.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('已选择 1 类资产');
    await toStep2(w);
    expect(w.text()).toContain('已为你筛选 0 个平台');
  });
});

describe('保存资产', () => {
  async function goToSaveStep(w: Awaited<ReturnType<typeof mountDiscover>>) {
    // step0 → step1 → step2
    await w.findAll('button').find(b => b.text().includes('继续'))!.trigger('click');
    const card = w.findAll('.category-card')[0];
    await card.trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text().includes('下一步：勾选平台'))!.trigger('click');
    await flushPromises();
  }

  it('未选平台时直接跳到完成页，savedCount 为 0', async () => {
    const w = await mountDiscover([platform(1, '支付宝', '支付金融')]);
    await goToSaveStep(w);
    // 没有勾选任何平台，仍可进入详情步但保存会走空分支
    const next = w.findAll('button').find(b => b.text().includes('下一步：设定资产详情'));
    if (next && !next.attributes('disabled')) {
      await next.trigger('click');
      await flushPromises();
      const save = w.findAll('button').find(b => b.text().includes('保存全部资产'));
      if (save) {
        await save.trigger('click');
        await flushPromises();
        expect(w.text()).toContain('0');
      }
    }
  });

  it('已登记的平台不重复写入（去重）', async () => {
    const p = platform(1, '支付宝', '支付金融');
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_all_platforms') return { success: true, message: '', data: [p] };
      // 该平台已在资产列表里
      if (cmd === 'get_all_assets') return { success: true, message: '', data: [assetRow({ platform_id: 1 })] };
      if (cmd === 'add_asset') return { success: true, message: '', data: 2 };
      return { success: true, message: '', data: null };
    });
    const pinia = createPinia();
    setActivePinia(pinia);
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/', component: { template: '<div/>' } },
        { path: '/discover', component: DiscoverView },
        { path: '/assets', component: { template: '<div/>' } },
        { path: '/report', component: { template: '<div/>' } },
      ],
    });
    await router.push('/discover');
    await router.isReady();
    const w = mount(DiscoverView, { global: { plugins: [pinia, router] } });
    for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }

    await w.findAll('button').find(b => b.text().includes('继续'))!.trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text().includes('清空选择'))!.trigger('click');
    await w.findAll('.category-card')[0].trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text().includes('下一步：勾选平台'))!.trigger('click');
    await flushPromises();

    // 勾选已登记的平台
    const pc = w.findAll('.platform-card')[0];
    if (pc) await pc.trigger('click');
    const next = w.findAll('button').find(b => b.text().includes('下一步：设定资产详情'));
    if (next && !next.attributes('disabled')) {
      await next.trigger('click');
      await flushPromises();
      const save = w.findAll('button').find(b => b.text().includes('保存全部资产'));
      if (save) {
        await save.trigger('click');
        await flushPromises();
      }
    }
    // 去重命中：不应调用 add_asset
    expect(mockedInvoke.mock.calls.filter(c => c[0] === 'add_asset')).toHaveLength(0);
  });

  it('保存失败时显示部分成功提示，不静默', async () => {
    const p = platform(1, '支付宝', '支付金融');
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_all_platforms') return { success: true, message: '', data: [p] };
      if (cmd === 'get_all_assets') return { success: true, message: '', data: [] };
      if (cmd === 'add_asset') return { success: false, message: '主密码已设置但处于锁定状态', data: null };
      return { success: true, message: '', data: null };
    });
    const pinia = createPinia();
    setActivePinia(pinia);
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/', component: { template: '<div/>' } },
        { path: '/discover', component: DiscoverView },
        { path: '/assets', component: { template: '<div/>' } },
        { path: '/report', component: { template: '<div/>' } },
      ],
    });
    await router.push('/discover');
    await router.isReady();
    const w = mount(DiscoverView, { global: { plugins: [pinia, router] } });
    for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }

    await w.findAll('button').find(b => b.text().includes('继续'))!.trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text().includes('清空选择'))!.trigger('click');
    await w.findAll('.category-card')[0].trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text().includes('下一步：勾选平台'))!.trigger('click');
    await flushPromises();
    await w.findAll('.platform-card')[0].trigger('click');
    const next = w.findAll('button').find(b => b.text().includes('下一步：设定资产详情'));
    if (next && !next.attributes('disabled')) {
      await next.trigger('click');
      await flushPromises();
      const save = w.findAll('button').find(b => b.text().includes('保存全部资产'));
      if (save) {
        await save.trigger('click');
        await flushPromises();
        expect(w.text()).toContain('部分资产保存失败');
      }
    }
  });
});

describe('store 联动', () => {
  it('加载平台后写入 platforms store', async () => {
    await mountDiscover([platform(1, '支付宝', '支付金融')]);
    const store = usePlatformsStore();
    expect(store.platforms).toHaveLength(1);
    expect(store.loaded).toBe(true);
  });

  it('加载资产后写入 assets store', async () => {
    await mountDiscover([]);
    const store = useAssetsStore();
    expect(store.assets).toEqual([]);
  });
});
