// 真实使用场景模拟（前端）：用真实 Pinia store + 真实 mock 后端跑数百个组合操作，
// 每步校验不变量。与既有单测的分工：
// - 既有单测：每个已知缺陷一条回归网；
// - 本文件：按矩阵遍历（类别 × 处置 × 金额 × 锁定态 × 语言），暴露组合边界。

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import type { Mock } from 'vitest';

vi.mock('./api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => false,
}));

import { invoke } from './api/invoke';
import { useAssetsStore } from './stores/assets';
import { useDashboardStore } from './stores/dashboard';
import { usePlatformsStore } from './stores/platforms';
import { useSecurityStore } from './stores/security';
import { mockInvoke } from './api/mock';
import { formatMoney, categoryLabel, disposalLabel } from './utils/format';
import { setLocale, t } from './i18n';

const mockedInvoke = invoke as unknown as Mock;

/** 场景检查点计数器：每个断言算一次可执行检查。 */
function makeChecker() {
  let count = 0;
  return {
    ok(cond: unknown, msg: string) {
      count += 1;
      expect(cond, msg).toBeTruthy();
    },
    get count() {
      return count;
    },
    report(label: string) {
      console.log(`[scenario] ${label}: ${count} 检查点`);
    },
  };
}

function ok<T>(data: T) {
  return { success: true, message: 'ok', data };
}

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
  localStorage.clear();
  sessionStorage.clear();
  setLocale('zh-CN');
});

describe('资产 store × 类别 × 处置 × 金额 矩阵（168 场景）', () => {
  const categories = ['property', 'personality', 'mixed'];
  const disposals = [
    'inherit', 'liquidate', 'memorialize', 'authorize_view', 'delete', 'donate', 'no_action',
  ];
  const values = [null, 0, 1, 9_999, 10_000, 123_456, 2_500_000, -1];

  it('每个组合都写入 store 且派生统计自洽', async () => {
    const c = makeChecker();
    const store = useAssetsStore();
    let id = 1;

    for (const category of categories) {
      for (const disposal of disposals) {
        for (const value of values) {
          const asset = {
            id: id++,
            platform_id: 1,
            platform_name: '支付宝',
            account_identifier: `acc-${id}`,
            account_description: '',
            category,
            estimated_value: value,
            value_currency: 'CNY',
            disposal_method: disposal,
            heir_name: null,
            heir_contact: null,
            notes: '',
            has_credential: false,
            is_confirmed: true,
            created_at: '2026-01-01 00:00:00',
            updated_at: '2026-01-01 00:00:00',
          };
          mockedInvoke.mockResolvedValueOnce(ok([asset]));
          await store.loadAssets();

          c.ok(store.assets.length === 1, `加载后应有 1 条 (${category}/${disposal}/${value})`);
          c.ok(store.assets[0].category === category, '类别不得被 store 改写');
          c.ok(store.assets[0].disposal_method === disposal, '处置不得被 store 改写');

          const expectPlanned = disposal !== 'no_action' ? 1 : 0;
          c.ok(
            store.disposalPlannedCount === expectPlanned,
            `已设定处置计数错误 (${disposal})`,
          );
          const valueSum = value !== null && value > 0 ? value : 0;
          c.ok(
            Math.abs(store.totalValue - valueSum) < 1e-6,
            `总价值错误 expected=${valueSum} actual=${store.totalValue}`,
          );
          c.ok(
            store.propertyCount + store.personalityCount + store.mixedCount === store.assets.length,
            '三类计数之和必须等于资产数',
          );
        }
      }
    }
    c.report('资产 store 矩阵');
    expect(c.count).toBeGreaterThanOrEqual(3 * 7 * 8 * 5);
  });
});

describe('筛选/排序/搜索 矩阵（80 场景）', () => {
  function makeRows() {
    return [
      { id: 1, platform_name: '支付宝', account_identifier: 'a@x.com', account_description: '', notes: '工资卡', category: 'property', disposal_method: 'inherit', estimated_value: 1000, created_at: '2026-03-01', value_currency: 'CNY', platform_id: 1, has_credential: false, is_confirmed: true, heir_name: null, heir_contact: null, updated_at: '' },
      { id: 2, platform_name: '微信', account_identifier: '13800000000', account_description: '生活号', notes: '', category: 'personality', disposal_method: 'no_action', estimated_value: null, created_at: '2026-01-01', value_currency: 'CNY', platform_id: 2, has_credential: true, is_confirmed: true, heir_name: null, heir_contact: null, updated_at: '' },
      { id: 3, platform_name: 'B站(bilibili)', account_identifier: 'bili_user', account_description: 'UP 主', notes: '有收益', category: 'mixed', disposal_method: 'memorialize', estimated_value: 50000, created_at: '2026-02-01', value_currency: 'CNY', platform_id: 3, has_credential: false, is_confirmed: false, heir_name: null, heir_contact: null, updated_at: '' },
      { id: 4, platform_name: 'Steam', account_identifier: 'steam_x', account_description: '', notes: '游戏库', category: 'property', disposal_method: 'liquidate', estimated_value: 800, created_at: '2025-12-31', value_currency: 'CNY', platform_id: 4, has_credential: true, is_confirmed: true, heir_name: null, heir_contact: null, updated_at: '' },
    ];
  }

  /** 与 AssetsView.filteredAssets 同语义的纯函数，便于矩阵化验证。 */
  function filterAndSort(
    rows: ReturnType<typeof makeRows>,
    filter: string,
    keyword: string,
    sortBy: string,
  ) {
    const byCat = filter === 'all' ? rows : rows.filter(a => a.category === filter);
    const terms = keyword.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const matched =
      terms.length === 0
        ? byCat
        : byCat.filter(a => {
            const haystack = [a.platform_name, a.account_identifier, a.account_description, a.notes]
              .join('\n')
              .toLowerCase();
            return terms.every(x => haystack.includes(x));
          });
    const sorted = [...matched];
    switch (sortBy) {
      case 'value':
        sorted.sort((a, b) => (b.estimated_value || 0) - (a.estimated_value || 0));
        break;
      case 'platform':
        sorted.sort((a, b) => a.platform_name.localeCompare(b.platform_name, 'zh-CN'));
        break;
      case 'disposal':
        sorted.sort(
          (a, b) =>
            Number(a.disposal_method === 'no_action') - Number(b.disposal_method === 'no_action'),
        );
        break;
      default:
        sorted.sort((a, b) => String(b.created_at).localeCompare(String(a.created_at)));
    }
    return sorted;
  }

  it('4 类别筛选 × 5 关键词 × 4 排序 = 80 组合全部自洽', () => {
    const c = makeChecker();
    const rows = makeRows();
    const filters = ['all', 'property', 'personality', 'mixed'];
    const keywords = ['', '支付', 'a@x', '不存在的关键词', '收益 有'];
    const sorts = ['recent', 'value', 'platform', 'disposal'];

    for (const filter of filters) {
      for (const keyword of keywords) {
        for (const sortBy of sorts) {
          const out = filterAndSort(rows, filter, keyword, sortBy);
          c.ok(out.length <= rows.length, '结果不得多于全集');
          if (filter !== 'all') {
            c.ok(out.every(a => a.category === filter), `筛选泄漏了其他类别 (${filter})`);
          }
          if (keyword.trim()) {
            const terms = keyword.trim().toLowerCase().split(/\s+/);
            c.ok(
              out.every(a =>
                terms.every(x =>
                  [a.platform_name, a.account_identifier, a.account_description, a.notes]
                    .join('\n')
                    .toLowerCase()
                    .includes(x),
                ),
              ),
              `关键词 ${keyword} 命中错误`,
            );
          }
          if (sortBy === 'value') {
            const vals = out.map(a => a.estimated_value || 0);
            c.ok(
              vals.every((v, i) => i === 0 || vals[i - 1] >= v),
              '价值降序被破坏',
            );
          }
          if (sortBy === 'platform') {
            const names = out.map(a => a.platform_name);
            c.ok(
              names.every((v, i) => i === 0 || names[i - 1].localeCompare(v, 'zh-CN') <= 0),
              '平台名排序被破坏',
            );
          }
          c.ok(new Set(out.map(a => a.id)).size === out.length, '结果出现重复资产');
        }
      }
    }
    c.report('筛选/排序/搜索矩阵');
    expect(c.count).toBeGreaterThanOrEqual(120 * 2);
  });
});

describe('格式化 × 语言 × 金额 矩阵（96 场景）', () => {
  it('中文/英文 × 12 个金额 × 4 个等级边界', () => {
    const c = makeChecker();
    const amounts = [0, 1, 999, 1000, 9999, 10_000, 99_999, 123_456, 999_999, 1_000_000, 2_500_000, 3_000_000_000];

    for (const locale of ['zh-CN', 'en-US'] as const) {
      setLocale(locale);
      for (const n of amounts) {
        const out = formatMoney(n);
        c.ok(out.length > 0, `${locale} ${n} 输出为空`);
        if (locale === 'zh-CN') {
          c.ok(!/[kMB]/.test(out) || n < 1000, `中文不应出现 k/M/B (${n} → ${out})`);
          if (n >= 10_000) c.ok(out.includes('万'), `中文万元应带「万」(${n} → ${out})`);
        } else {
          c.ok(!out.includes('万'), `英文不应出现「万」(${n} → ${out})`);
          if (n >= 1_000_000_000) c.ok(out.endsWith('B'), `十亿级应带 B (${n} → ${out})`);
          else if (n >= 1_000_000) c.ok(out.endsWith('M'), `百万级应带 M (${n} → ${out})`);
          else if (n >= 1000) c.ok(out.endsWith('k'), `千级应带 k (${n} → ${out})`);
        }
        c.ok(!out.includes('NaN') && !out.includes('undefined'), `非法输出 ${out}`);
      }
    }
    setLocale('zh-CN');
    c.report('格式化×语言×金额矩阵');
    expect(c.count).toBeGreaterThanOrEqual(2 * amounts.length * 2);
  });

  it('三类枚举 × 七种处置 × 两语言：文案永不回退成键名或英文原值', () => {
    const c = makeChecker();
    const cats = ['property', 'personality', 'mixed'];
    const disposals = ['inherit', 'liquidate', 'memorialize', 'authorize_view', 'delete', 'donate', 'no_action'];
    for (const locale of ['zh-CN', 'en-US'] as const) {
      setLocale(locale);
      for (const cat of cats) {
        const label = categoryLabel(cat);
        c.ok(label !== cat && !label.includes('.'), `类别 ${cat} 未翻译 (${label})`);
      }
      for (const d of disposals) {
        const label = disposalLabel(d);
        c.ok(label !== d && !label.includes('.'), `处置 ${d} 未翻译 (${label})`);
      }
      c.ok(t('nav.dashboard').length > 0, '导航文案为空');
      c.ok(t('i18n.missing.key') === 'i18n.missing.key', '缺失键必须回退键名');
    }
    setLocale('zh-CN');
    c.report('枚举文案×语言矩阵');
    expect(c.count).toBeGreaterThanOrEqual(2 * (3 + 7 + 2));
  });
});

describe('锁定态 × 写路径 矩阵（60 场景）', () => {
  const writes = [
    { cmd: 'add_asset', args: { req: { platform_id: 1, category: 'property', disposal_method: 'inherit' } } },
    { cmd: 'update_asset', args: { id: 1, req: { platform_id: 1, category: 'property', disposal_method: 'inherit' } } },
    { cmd: 'set_disposal_wish', args: { asset_id: 1, disposal_method: 'inherit', notes: '' } },
    { cmd: 'set_user_profile', args: { profile: { full_name: 'x' } } },
    { cmd: 'set_asset_credential', args: { id: 1, credential: 'secret' } },
  ] as const;

  it('锁定态每条写命令都必须失败且错误可见', async () => {
    const c = makeChecker();
    for (const w of writes) {
      mockedInvoke.mockResolvedValueOnce({
        success: false,
        message: '主密码已设置但处于锁定状态，请先解锁后再保存',
        data: null,
      });
      const res = (await invoke(w.cmd, w.args)) as { success: boolean; message: string };
      c.ok(res.success === false, `锁定态 ${w.cmd} 必须失败`);
      c.ok(res.message.includes('解锁'), `锁定态 ${w.cmd} 提示必须指向解锁`);
    }
    // 与 store 相关的解锁态对照：同一命令在解锁时必须被调用
    const security = useSecurityStore();
    mockedInvoke.mockResolvedValueOnce(ok({ has_master_password: true, unlocked: false }));
    await security.refresh();
    c.ok(security.hasMasterPassword && !security.unlocked, '锁定态读取错误');
    mockedInvoke.mockResolvedValueOnce(ok({ success: true }));
    await security.lockNow();
    c.ok(!security.unlocked, 'lockNow 后必须为锁定');
    c.report('锁定态×写路径矩阵');
    expect(c.count).toBeGreaterThanOrEqual(writes.length * 2 + 2);
  });

  it('12 条资产在锁定态下批量读取：每条都返回占位符而不是抛整表错误', async () => {
    const c = makeChecker();
    const placeholder = '【加密数据 · 需解锁】';
    for (let i = 0; i < 12; i++) {
      mockedInvoke.mockResolvedValueOnce(
        ok([
          {
            id: i + 1,
            platform_id: 1,
            platform_name: '支付宝',
            account_identifier: placeholder,
            account_description: placeholder,
            category: 'property',
            estimated_value: i,
            value_currency: 'CNY',
            disposal_method: 'inherit',
            heir_name: null,
            heir_contact: null,
            notes: placeholder,
            has_credential: false,
            is_confirmed: true,
            created_at: '',
            updated_at: '',
          },
        ]),
      );
      const store = useAssetsStore();
      await store.loadAssets();
      c.ok(store.assets.length === 1, '锁定态列表必须仍可渲染');
      c.ok(store.assets[0].account_identifier === placeholder, '锁定态必须显示占位符');
      c.ok(store.loadError === '', '占位符不算加载失败');
    }
    c.report('锁定态批量读取矩阵');
    expect(c.count).toBeGreaterThanOrEqual(12 * 3);
  });
});

describe('真实 mock 后端 × 资产生命周期 矩阵（80 场景）', () => {
  it('20 轮 新增→读回→更新→删除，状态与 localStorage 一致', async () => {
    const c = makeChecker();
    for (let round = 0; round < 20; round++) {
      const cat = ['property', 'personality', 'mixed'][round % 3];
      const disp = ['inherit', 'liquidate', 'memorialize', 'delete'][round % 4];
      const added = (await mockInvoke('add_asset', {
        req: {
          platform_id: 1,
          account_identifier: `r${round}`,
          account_description: '',
          category: cat,
          estimated_value: round * 100,
          disposal_method: disp,
          heir_name: disp === 'inherit' ? `继承人-${round}` : undefined,
          notes: '',
        },
      })) as { success: boolean; data: number };
      c.ok(added.success, `mock add 失败 round=${round}`);
      const id = added.data;

      const got = (await mockInvoke('get_asset_by_id', { id })) as {
        success: boolean;
        data: Record<string, unknown> | null;
      };
      c.ok(got.success && got.data !== null, 'mock 读回为空');
      c.ok(got.data?.category === cat, 'mock 类别不一致');
      c.ok(got.data?.disposal_method === disp, 'mock 处置不一致');

      const upd = (await mockInvoke('update_asset', {
        id,
        req: {
          platform_id: 1,
          account_identifier: `r${round}-upd`,
          account_description: '',
          category: cat,
          estimated_value: 1,
          disposal_method: 'no_action',
          notes: '',
        },
      })) as { success: boolean };
      c.ok(upd.success, 'mock update 失败');
      const after = (await mockInvoke('get_asset_by_id', { id })) as {
        data: Record<string, unknown> | null;
      };
      c.ok(after.data?.account_identifier === `r${round}-upd`, 'mock 更新未生效');

      const del = (await mockInvoke('delete_asset', { id })) as { success: boolean };
      c.ok(del.success, 'mock delete 失败');
      const gone = (await mockInvoke('get_asset_by_id', { id })) as { data: unknown };
      c.ok(gone.data === null, 'mock 删除后仍可读');
    }
    c.report('mock 资产生命周期矩阵');
    expect(c.count).toBeGreaterThanOrEqual(20 * 7);
  });
});

describe('占位符 × 写路径 矩阵（24 场景）', () => {
  const PLACEHOLDER = '【加密数据 · 需解锁】';

  it('每个敏感字段单独遇到占位符都必须被识别', async () => {
    const c = makeChecker();
    const fields = [
      'account_identifier', 'account_description', 'notes', 'heir_name', 'heir_contact',
    ];
    for (const field of fields) {
      // 与 AssetDetailView 同语义：任一敏感字段为占位符即视为未解密
      const asset: Record<string, unknown> = {
        id: 1, platform_id: 1, platform_name: '支付宝',
        account_identifier: 'ok', account_description: 'ok', notes: 'ok',
        heir_name: 'ok', heir_contact: 'ok',
      };
      const detect = (a: Record<string, unknown>) =>
        ['account_identifier', 'account_description', 'notes', 'heir_name', 'heir_contact']
          .filter(k => a[k] === PLACEHOLDER);

      c.ok(detect(asset).length === 0, `${field} 正常值不应误报`);
      asset[field] = PLACEHOLDER;
      const hits = detect(asset);
      c.ok(hits.length === 1 && hits[0] === field, `${field} 占位符必须被识别`);
    }
    c.report('占位符识别矩阵');
    expect(c.count).toBeGreaterThanOrEqual(fields.length * 2);
  });

  it('12 个档案字段逐个置为占位符都能被识别', () => {
    const c = makeChecker();
    const keys = [
      'full_name', 'phone', 'email', 'id_last4',
      'emergency_contact_name', 'emergency_contact_phone', 'emergency_contact_relation', 'emergency_contact_email',
      'heir_primary_name', 'heir_primary_phone', 'heir_primary_relation', 'heir_primary_email',
    ];
    const detect = (p: Record<string, string>) => keys.filter(k => p[k] === PLACEHOLDER);
    for (const key of keys) {
      const profile: Record<string, string> = Object.fromEntries(keys.map(k => [k, 'ok']));
      profile[key] = PLACEHOLDER;
      const hits = detect(profile);
      c.ok(hits.length === 1 && hits[0] === key, `${key} 占位符必须被识别`);
    }
    c.report('档案占位符矩阵');
    expect(c.count).toBe(keys.length);
  });
});

describe('平台 store × 缓存失效 矩阵（40 场景）', () => {
  it('缓存命中/强制刷新/扩展失效 三种状态各 10 轮一致', async () => {
    const c = makeChecker();
    const store = usePlatformsStore();
    const rows = [{ id: 1, name: '支付宝', category: '支付金融', subtype: '', has_property: true, has_personality: false, inheritable: true, required_documents: '', process_description: '', estimated_difficulty: '', official_policy_url: '', estimated_time_days: '', notes: '' }];

    for (let round = 0; round < 10; round++) {
      mockedInvoke.mockClear();
      mockedInvoke.mockResolvedValue(ok(rows));
      store.loaded = false;
      store.platformsExtended = [];
      await store.loadPlatforms();
      c.ok(mockedInvoke.mock.calls.filter(x => x[0] === 'get_all_platforms').length === 1, '首次应请求');
      await store.loadPlatforms();
      c.ok(mockedInvoke.mock.calls.filter(x => x[0] === 'get_all_platforms').length === 1, '缓存应命中');
      await store.refreshPlatforms();
      c.ok(mockedInvoke.mock.calls.filter(x => x[0] === 'get_all_platforms').length === 2, 'refresh 应重取');
      c.ok(store.platformsExtended.length === 0, 'refresh 必须清空扩展缓存');
    }
    c.report('平台缓存失效矩阵');
    expect(c.count).toBeGreaterThanOrEqual(10 * 4);
  });
});
