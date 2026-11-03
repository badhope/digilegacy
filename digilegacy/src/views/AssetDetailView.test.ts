import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { createRouter, createMemoryHistory } from 'vue-router';
import type { Mock } from 'vitest';

// AssetDetailView 负责凭据的保存/复制/清除（109 条语句，此前 0% 覆盖）。
// 凭据是本项目最敏感的本地数据，这条路径的错误处理必须验证。
vi.mock('../api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => false,
}));

import AssetDetailView from './AssetDetailView.vue';
import { settleConfirm } from '../composables/feedback';
import { invoke } from '../api/invoke';
import { useAssetsStore } from '../stores/assets';
import { usePlatformsStore } from '../stores/platforms';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();

const mockedInvoke = invoke as unknown as Mock;

function asset(over: Record<string, unknown> = {}) {
  return {
    id: 1, platform_id: 1, platform_name: '支付宝',
    account_identifier: '138****0000', account_description: '主账号',
    category: 'property', estimated_value: 1000, value_currency: 'CNY',
    disposal_method: 'inherit', heir_name: '张三', heir_contact: '139****0000',
    notes: '备注', has_credential: false, is_confirmed: true,
    created_at: '', updated_at: '', ...over,
  };
}

function platform(over: Record<string, unknown> = {}) {
  return {
    id: 1, name: '支付宝', category: '支付金融', subtype: '移动支付',
    has_property: true, has_personality: false, inheritable: true,
    required_documents: '', process_description: '',
    estimated_difficulty: '简单', official_policy_url: 'https://example.com/policy',
    estimated_time_days: '30', notes: '', ...over,
  };
}

async function mountDetail(opts: {
  a?: ReturnType<typeof asset>;
  invokeOverrides?: Record<string, unknown>;
} = {}) {
  const a = opts.a ?? asset();
  mockedInvoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
    const ov = opts.invokeOverrides ?? {};
    if (cmd in ov) {
      const v = ov[cmd];
      return typeof v === 'function' ? (v as (a?: unknown) => unknown)(args) : v;
    }
    if (cmd === 'get_all_assets') return { success: true, message: '', data: [a] };
    if (cmd === 'get_all_platforms') return { success: true, message: '', data: [platform()] };
    // 凭据区只在已启用主密码时渲染（AssetDetailView.vue:20），
    // 必须默认 true，否则所有凭据用例都在空转。
    if (cmd === 'security_status') return { success: true, message: '', data: { has_master_password: true, unlocked: true } };
    if (cmd === 'get_asset_by_id') return { success: true, message: '', data: a };
    return { success: true, message: '', data: null };
  });

  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', component: { template: '<div/>' } },
      { path: '/assets', component: { template: '<div/>' } },
      { path: '/profile', component: { template: '<div/>' } },
      { path: '/assets/:id', component: AssetDetailView },
    ],
  });
  await router.push(`/assets/${a.id}`);
  await router.isReady();

  const pinia = createPinia();
  setActivePinia(pinia);
  const w = mount(AssetDetailView, { global: { plugins: [pinia, router] } });
  for (let i = 0; i < 3; i++) { await flushPromises(); await w.vm.$nextTick(); }
  return w;
}

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
  vi.stubGlobal('confirm', vi.fn(() => true));
  vi.stubGlobal('alert', vi.fn());
});

describe('渲染', () => {
  it('展示资产标识与平台信息', async () => {
    const w = await mountDetail();
    expect(w.text()).toContain('支付宝');
    expect(w.text()).toContain('138****0000');
  });

  it('金融类平台显示相应提示', async () => {
    const w = await mountDetail();
    // 平台 category=支付金融 → isFinancialPlatform 为 true
    expect(w.text()).toContain('支付宝');
  });

  it('资产不存在时不崩溃', async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_all_assets') return { success: true, message: '', data: [] };
      if (cmd === 'get_all_platforms') return { success: true, message: '', data: [] };
      if (cmd === 'security_status') return { success: true, message: '', data: { has_master_password: false, unlocked: true } };
      return { success: true, message: '', data: null };
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/', component: { template: '<div/>' } },
        { path: '/assets', component: { template: '<div/>' } },
        { path: '/profile', component: { template: '<div/>' } },
        { path: '/assets/:id', component: AssetDetailView },
      ],
    });
    await router.push('/assets/999');
    await router.isReady();
    const pinia = createPinia();
    setActivePinia(pinia);
    const w = mount(AssetDetailView, { global: { plugins: [pinia, router] } });
    await flushPromises();
    expect(w.text().length).toBeGreaterThan(0);
  });
});

describe('凭据保存', () => {
  it('空输入时本地拦截，不调用后端', async () => {
    const w = await mountDetail();
    const saveBtn = w.findAll('button').find(b => b.text().includes('加密保存'));
    expect(saveBtn, '凭据保存按钮必须存在（需 hasMasterPassword=true）').toBeTruthy();

    await saveBtn!.trigger('click');
    await flushPromises();

    expect(mockedInvoke).not.toHaveBeenCalledWith('set_asset_credential', expect.anything());
    expect(w.text()).toContain('请输入要保存的内容');
  });

  it('保存成功后清空输入并刷新 has_credential', async () => {
    const w = await mountDetail({
      invokeOverrides: {
        set_asset_credential: { success: true, message: '凭据已保存', data: null },
        get_asset_by_id: { success: true, message: '', data: asset({ has_credential: true }) },
      },
    });

    const input = w.find('input[aria-label*="凭据"]');
    expect(input.exists(), '凭据输入框必须存在').toBe(true);
    await input.setValue('user:pass');

    const btn = w.findAll('button').find(b => b.text().includes('加密保存'));
    expect(btn, '加密保存按钮必须存在').toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('凭据已保存');
  });

  it('保存失败时显示后端消息（不静默）', async () => {
    const w = await mountDetail({
      invokeOverrides: {
        set_asset_credential: { success: false, message: '请先解锁后再保存凭据', data: null },
      },
    });

    const input = w.find('input[aria-label*="凭据"]');
    expect(input.exists()).toBe(true);
    await input.setValue('secret');

    const btn = w.findAll('button').find(b => b.text().includes('加密保存'));
    expect(btn).toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('请先解锁');
  });
});

describe('凭据复制', () => {
  it('未保存凭据时给出明确提示，不写入剪贴板', async () => {
    const writeText = vi.fn();
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText, readText: vi.fn() } });

    const w = await mountDetail({
      invokeOverrides: {
        get_asset_credential: { success: true, message: '', data: '' },
      },
    });

    const btn = w.findAll('button').find(b => b.text().includes('复制凭据'));
    expect(btn, '复制凭据按钮必须存在').toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('尚未保存凭据');
    expect(writeText).not.toHaveBeenCalled();
  });

  it('有凭据时写入剪贴板并提示', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText, readText: vi.fn() } });

    const w = await mountDetail({
      invokeOverrides: {
        get_asset_credential: { success: true, message: '', data: 'user:pass' },
      },
    });

    const btn = w.findAll('button').find(b => b.text().includes('复制凭据'));
    expect(btn).toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();
    expect(writeText).toHaveBeenCalledWith('user:pass');
    expect(w.text()).toContain('已复制');
  });

  it('读取凭据失败时提示错误', async () => {
    const w = await mountDetail({
      invokeOverrides: {
        get_asset_credential: () => { throw new Error('数据已加密，请先解锁'); },
      },
    });

    const btn = w.findAll('button').find(b => b.text().includes('复制凭据'));
    expect(btn).toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('复制失败');
  });
});

describe('凭据清除', () => {
  it('用户取消时不调用后端', async () => {
    const w = await mountDetail({ a: asset({ has_credential: true }) });
    const btn = w.findAll('button').find(b => b.text().trim() === '清除');
    expect(btn, '有凭据时必须显示清除按钮').toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();
    // 自定义确认框：取消
    settleConfirm(false);
    await flushPromises();
    expect(mockedInvoke).not.toHaveBeenCalledWith('set_asset_credential', expect.anything());
  });

  it('确认后调用后端清除', async () => {
    const w = await mountDetail({
      a: asset({ has_credential: true }),
      invokeOverrides: { set_asset_credential: { success: true, message: '凭据已清除', data: null } },
    });
    const btn = w.findAll('button').find(b => b.text().trim() === '清除');
    expect(btn).toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();
    settleConfirm(true);
    await flushPromises();
    expect(mockedInvoke).toHaveBeenCalledWith('set_asset_credential', expect.objectContaining({ credential: '' }));
  });
});

describe('外部链接', () => {
  it('无官方链接时提示，不调用后端', async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_all_assets') return { success: true, message: '', data: [asset()] };
      if (cmd === 'get_all_platforms') return { success: true, message: '', data: [platform({ official_policy_url: '' })] };
      if (cmd === 'security_status') return { success: true, message: '', data: { has_master_password: false, unlocked: true } };
      if (cmd === 'get_asset_by_id') return { success: true, message: '', data: asset() };
      return { success: true, message: '', data: null };
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/', component: { template: '<div/>' } },
        { path: '/profile', component: { template: '<div/>' } },
        { path: '/assets/:id', component: AssetDetailView },
      ],
    });
    await router.push('/assets/1');
    await router.isReady();
    const pinia = createPinia();
    setActivePinia(pinia);
    const w = mount(AssetDetailView, { global: { plugins: [pinia, router] } });
    await flushPromises();

    const btn = w.findAll('button').find(b => b.text().includes('打开平台官网'));
    expect(btn, '官方链接按钮必须存在').toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();
    expect(mockedInvoke).not.toHaveBeenCalledWith('open_external', expect.anything());
    expect(w.text()).toContain('暂无该平台官方链接');
  });
});

describe('保存资产', () => {
  it('点击保存调用 updateAsset 并返回上一页', async () => {
    const w = await mountDetail();
    const saveBtn = w.findAll('button').find(b => b.text() === '保存修改');
    expect(saveBtn, '保存按钮必须存在').toBeTruthy();

    await saveBtn!.trigger('click');
    await flushPromises();

    expect(mockedInvoke.mock.calls.some(c => c[0] === 'update_asset')).toBe(true);
  });

  it('保存失败时留在页面并显示错误（不得静默、不得假装成功）', async () => {
    const w = await mountDetail({
      invokeOverrides: { update_asset: { success: false, message: '锁定状态', data: null } },
    });
    const saveBtn = w.findAll('button').find(b => b.text() === '保存修改')!;

    await saveBtn.trigger('click');
    await flushPromises();

    expect(mockedInvoke.mock.calls.some(c => c[0] === 'update_asset')).toBe(true);
    // 页面必须给出可见错误，且不能离开当前路由（用户还要重试）
    const alert = w.find('[role="alert"]');
    expect(alert.exists(), '失败必须渲染 role=alert 的错误提示').toBe(true);
    expect(alert.text()).toContain('保存失败');
    expect(w.find('#detail-identifier').exists()).toBe(true);
  });
});

describe('删除资产', () => {
  it('取消确认时不调用 deleteAsset', async () => {
    const w = await mountDetail();
    const btn = w.findAll('button').find(b => b.text() === '删除此资产');
    expect(btn, '删除按钮必须存在').toBeTruthy();
    await btn!.trigger('click');
    await flushPromises();
    settleConfirm(false);
    await flushPromises();
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'delete_asset')).toBe(false);
  });

  it('确认后调用 deleteAsset', async () => {
    const w = await mountDetail();
    const btn = w.findAll('button').find(b => b.text() === '删除此资产')!;
    await btn.trigger('click');
    await flushPromises();
    settleConfirm(true);
    await flushPromises();
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'delete_asset')).toBe(true);
  });
});

describe('占位符防护（数据覆盖风险）', () => {
  it('未解密字段显示告警，并阻止保存（不得提交占位符覆盖真实数据）', async () => {
    const w = await mountDetail({
      a: asset({
        account_identifier: '【加密数据 · 需解锁】',
        notes: '【加密数据 · 需解锁】',
      }),
    });

    expect(w.text(), '必须显示未解密告警').toContain('部分字段未能解密');
    expect(w.text()).toContain('账号标识');
    expect(w.text()).toContain('备注');

    const saveBtn = w.findAll('button').find(b => b.text() === '保存修改')!;
    await saveBtn.trigger('click');
    await flushPromises();

    expect(mockedInvoke, '不得调用 update_asset').not.toHaveBeenCalledWith(
      'update_asset',
      expect.anything(),
    );
    expect(w.text()).toContain('无法保存');
  });

  it('没有占位符时正常保存（防护不能误伤普通路径）', async () => {
    const w = await mountDetail({ a: asset({ account_identifier: 'real@example.com' }) });
    expect(w.text()).not.toContain('部分字段未能解密');

    const saveBtn = w.findAll('button').find(b => b.text() === '保存修改')!;
    await saveBtn.trigger('click');
    await flushPromises();

    expect(mockedInvoke.mock.calls.some(c => c[0] === 'update_asset')).toBe(true);
  });
});
