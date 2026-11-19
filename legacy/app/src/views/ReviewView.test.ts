import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import type { Mock } from 'vitest';

// ReviewView 的关键契约：AI 只能把条目推进到「待人工」，
// 界面上的「通过」按钮背后必须是人工动作，且状态如实展示。
vi.mock('../api/invoke', () => ({ invoke: vi.fn(), isTauriEnv: () => false }));

import ReviewView from './ReviewView.vue';
import { invoke } from '../api/invoke';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();
const mockedInvoke = invoke as unknown as Mock;

const PROFILE = {
  id: 'platform-policy',
  title_key: 'review.profile.policy',
  criteria: '检查来源与材料一致性',
  use_ai: true,
  strictness: 'standard',
  strict_checks: [],
};

function state(over: Partial<Record<string, unknown>> = {}) {
  return {
    key: '1',
    title: '支付宝',
    preview: '平台：支付宝 · 所需材料：死亡证明',
    status: 'pending',
    ai_verdict: null,
    ai_note: '',
    human_note: '',
    reviewed_at: null,
    stale: false,
    ...over,
  };
}

async function mountReview(over: Record<string, unknown> = {}) {
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd in over) return over[cmd as keyof typeof over];
    if (cmd === 'list_review_profiles') return { success: true, message: '', data: [PROFILE] };
    if (cmd === 'review_payload_fields') return { success: true, message: '', data: ['平台名称', '官方链接'] };
    if (cmd === 'review_progress') return { success: true, message: '', data: { total: 1, approved: 0, awaiting_human: 0, pending: 1 } };
    if (cmd === 'list_review_items') return { success: true, message: '', data: [state()] };
    return { success: true, message: '', data: null };
  });
  const pinia = createPinia();
  setActivePinia(pinia);
  const w = mount(ReviewView, { global: { plugins: [pinia] } });
  for (let i = 0; i < 3; i++) {
    await flushPromises();
    await w.vm.$nextTick();
  }
  return w;
}

/** 条目默认折叠：先展开第一条才能看到备注、按钮与 AI 说明。 */
async function expandFirst(w: Awaited<ReturnType<typeof mountReview>>) {
  const head = w.find('.review-head-btn');
  if (head.exists()) {
    await head.trigger('click');
    await flushPromises();
  }
}

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
});

describe('审核场景与送审字段', () => {
  it('按后端注册表渲染场景，不写死业务名', async () => {
    const w = await mountReview();
    const tabs = w.findAll('[role="tab"]');
    expect(tabs.length).toBe(1);
    expect(tabs[0].text()).toContain('平台政策');
  });

  it('多场景时全部渲染，且切换场景会重新拉条目', async () => {
    const w = await mountReview({
      list_review_profiles: {
        success: true, message: '',
        data: [
          { ...PROFILE },
          { id: 'legal-template', title_key: 'review.profile.legal', criteria: '逐项检查', use_ai: true, strictness: 'strict', strict_checks: [] },
          { id: 'family-note', title_key: 'review.profile.familyNote', criteria: '检查可执行性', use_ai: true },
          { id: 'asset-classification', title_key: 'review.profile.asset', criteria: '检查类别', use_ai: true },
        ],
      },
    });
    const tabs = w.findAll('[role="tab"]');
    expect(tabs.length).toBe(4);
    expect(tabs.map(t => t.text())).toEqual(['平台政策', '法律文书', '家人说明', '资产分类']);

    await tabs[1].trigger('click');
    await flushPromises();
    const calls = mockedInvoke.mock.calls.filter(c => c[0] === 'list_review_items');
    expect(calls.length, '每次切场景都要重新拉条目').toBeGreaterThanOrEqual(2);
    expect((calls[calls.length - 1][1] as Record<string, unknown>).profileId).toBe('legal-template');
  });

  it('严格场景显示严格模式说明与必检项', async () => {
    const w = await mountReview({
      list_review_profiles: {
        success: true, message: '',
        data: [
          { ...PROFILE },
          {
            id: 'legal-template', title_key: 'review.profile.legal', criteria: '逐项检查',
            use_ai: true, strictness: 'strict',
            strict_checks: [
              { id: 'signer', label: '签署主体已填写' },
              { id: 'blanks', label: '应填空白项已填完' },
            ],
          },
        ],
      },
    });
    await w.findAll('[role="tab"]')[1].trigger('click');
    await flushPromises();
    expect(w.text()).toContain('严格模式');
    expect(w.text()).toContain('签署主体已填写');
    expect(w.text()).toContain('缺项本次初审作废');
  });

  it('展示审核进度，避免用户一条条数', async () => {
    const w = await mountReview();
    expect(w.text()).toContain('共 1 条');
    expect(w.text()).toContain('待审 1');
  });

  it('明确展示 AI 能看到哪些字段', async () => {
    const w = await mountReview();
    expect(w.text()).toContain('AI 能看到的内容');
    expect(w.text()).toContain('平台名称');
    expect(w.text()).toContain('官方链接');
  });

  it('纯人工场景要说明 AI 不参与', async () => {
    const w = await mountReview({
      list_review_profiles: {
        success: true, message: '',
        data: [{ ...PROFILE, id: 'legal', title_key: 'review.profile.policy', use_ai: false }],
      },
    });
    expect(w.text()).toContain('纯人工审核');
  });
});

describe('AI 初审与人工终审的分工', () => {
  it('AI 初审后条目显示「未发现问题」但仍为「待人工确认」', async () => {
    const w = await mountReview({
      run_ai_review: {
        success: true, message: 'AI 初审完成，1 条待人工确认',
        data: [state({ status: 'awaiting_human', ai_verdict: 'pass', ai_note: '未发现矛盾' })],
      },
    });
    await w.findAll('button').find(b => b.text().includes('运行 AI 初审'))!.trigger('click');
    await flushPromises();
    await expandFirst(w);

    expect(w.text()).toContain('未发现矛盾');
    // 状态标签必须仍是「待人工确认」：AI 不能把条目置为已通过。
    // （不能用整页文本断言「不含已通过」——筛选按钮本身就有这个词。）
    const statusTag = w.findAll('.review-head .tag').map(t => t.text());
    expect(statusTag).toContain('待人工确认');
    expect(statusTag, 'AI 初审后不得出现已通过状态').not.toContain('已通过');
  });

  it('未配置 AI 时如实降级为纯人工，不伪造结论', async () => {
    const w = await mountReview({
      run_ai_review: {
        success: true, message: '未配置 AI 端点，已转为纯人工审核',
        data: [state({ ai_verdict: 'skipped', ai_note: '未配置 AI 端点，已转为纯人工审核' })],
      },
    });
    await w.findAll('button').find(b => b.text().includes('运行 AI 初审'))!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('未配置 AI 端点');
    expect(w.text()).toContain('未运行');
  });

  it('人工点「通过」才提交人工结论，并带备注', async () => {
    const w = await mountReview({
      submit_human_review: { success: true, message: '审核结论已记录', data: null },
    });
    await expandFirst(w);
    await w.find('input.form-input').setValue('已核对官方公告');
    await w.findAll('button').find(b => b.text() === '通过')!.trigger('click');
    await flushPromises();

    const call = mockedInvoke.mock.calls.find(c => c[0] === 'submit_human_review');
    expect(call, '必须调用人工终审').toBeTruthy();
    const args = call![1] as Record<string, unknown>;
    expect(args.verdict).toBe('approved');
    expect(args.note).toBe('已核对官方公告');
  });

  it('拒绝与退回修改分别提交对应结论', async () => {
    const w = await mountReview({
      submit_human_review: { success: true, message: 'ok', data: null },
    });
    await expandFirst(w);
    await w.findAll('button').find(b => b.text() === '拒绝')!.trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text() === '退回修改')!.trigger('click');
    await flushPromises();

    const verdicts = mockedInvoke.mock.calls
      .filter(c => c[0] === 'submit_human_review')
      .map(c => (c[1] as Record<string, unknown>).verdict);
    expect(verdicts).toEqual(['rejected', 'needs_edit']);
  });
});

describe('留痕与失效', () => {
  it('内容变化时显示「需重审」', async () => {
    const w = await mountReview({
      list_review_items: {
        success: true, message: '',
        data: [state({ status: 'pending', stale: true, ai_verdict: 'pass' })],
      },
    });
    expect(w.text()).toContain('内容已变');
  });

  it('审核历史展示操作方与结论', async () => {
    const w = await mountReview({
      review_history: {
        success: true, message: '',
        data: [{
          profile_id: 'platform-policy', item_key: '1', fingerprint: 'abc',
          ai_verdict: 'flag', ai_note: '缺来源', human_verdict: 'approved',
          human_note: '已补链接', decided_by: 'human', created_at: '2026-10-26 10:00:00',
        }],
      },
    });
    await expandFirst(w);
    await w.findAll('button').find(b => b.text().includes('审核历史'))!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('2026-10-26 10:00:00');
    expect(w.text()).toContain('已补链接');
    expect(w.text()).toContain('human');
  });

  it('后端报错时可见，不静默', async () => {
    const w = await mountReview({
      run_ai_review: { success: false, message: '未知的审核场景', data: null },
    });
    await w.findAll('button').find(b => b.text().includes('运行 AI 初审'))!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('未知的审核场景');
  });
});

describe('状态与结论标签的健壮性', () => {
  it('ai_verdict 为空时显示占位符，不泄露 i18n 键名', async () => {
    const w = await mountReview({
      list_review_items: { success: true, message: '', data: [state({ ai_verdict: null })] },
    });
    expect(w.text()).not.toContain('review.ai.null');
    expect(w.text()).not.toContain('review.ai.');
  });

  it('未知状态值同样不泄露键名', async () => {
    const w = await mountReview({
      list_review_items: { success: true, message: '', data: [state({ status: 'weird_status' })] },
    });
    expect(w.text()).not.toContain('review.status.weird');
  });

  it('纯人工场景不显示「运行 AI 初审」按钮', async () => {
    const w = await mountReview({
      list_review_profiles: {
        success: true, message: '',
        data: [{ ...PROFILE, use_ai: false }],
      },
    });
    const btn = w.findAll('button').find(b => b.text().includes('运行 AI 初审'));
    expect(btn, '纯人工场景不该有 AI 按钮').toBeFalsy();
    expect(w.text()).toContain('审核范围');
  });
});

describe('待审内容可见性', () => {
  it('展开后显示内容摘要，审核员能看到审的是什么', async () => {
    const w = await mountReview();
    await expandFirst(w);
    expect(w.text()).toContain('所需材料：死亡证明');
  });
});

describe('条目折叠', () => {
  it('默认收起：只显示摘要行，不显示操作按钮', async () => {
    const w = await mountReview({
      list_review_items: { success: true, message: '', data: [state({ title: '支付宝' }), state({ key: '2', title: '微信支付' })] },
    });
    // 两条摘要行都在
    expect(w.findAll('.review-head-btn').length).toBe(2);
    // 默认不展开 → 没有「通过」按钮
    expect(w.findAll('button').some(b => b.text() === '通过')).toBe(false);
  });

  it('点击摘要行展开，再次点击收起', async () => {
    const w = await mountReview();
    const head = w.find('.review-head-btn');
    await head.trigger('click');
    await flushPromises();
    expect(w.findAll('button').some(b => b.text() === '通过')).toBe(true);
    expect(head.attributes('aria-expanded')).toBe('true');

    await head.trigger('click');
    await flushPromises();
    expect(w.findAll('button').some(b => b.text() === '通过')).toBe(false);
  });
});

describe('严格模式：疑点必须写理由才能放行', () => {
  const strictProfile = {
    id: 'legal-template',
    title_key: 'review.profile.legal',
    criteria: '逐项检查',
    use_ai: true,
    strictness: 'strict',
    strict_checks: [{ id: 'blanks', label: '应填空白项已填完' }],
  };

  const flaggedItem = state({
    key: '1',
    title: '数字资产遗嘱',
    ai_verdict: 'flag',
    ai_note: '第 3 段仍有未填空白',
    status: 'awaiting_human',
    preview: '正文：立遗嘱人：____',
  });

  it('AI 报过疑点时，空备注点「通过」被前端拦下并给出原因', async () => {
    const w = await mountReview({
      list_review_profiles: { success: true, message: '', data: [strictProfile] },
      list_review_items: { success: true, message: '', data: [flaggedItem] },
    });
    await expandFirst(w);
    await w.findAll('button').find(b => b.text() === '通过')!.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('请先填写通过理由');
    expect(
      mockedInvoke.mock.calls.some(c => c[0] === 'submit_human_review'),
      '被拦下时不得调用后端',
    ).toBe(false);
  });

  it('写明理由后可以提交', async () => {
    const w = await mountReview({
      list_review_profiles: { success: true, message: '', data: [strictProfile] },
      list_review_items: { success: true, message: '', data: [flaggedItem] },
      submit_human_review: { success: true, message: '审核结论已记录', data: null },
    });
    await expandFirst(w);
    await w.find('input.form-input').setValue('空白项为无需填写，已核对');
    await w.findAll('button').find(b => b.text() === '通过')!.trigger('click');
    await flushPromises();

    const call = mockedInvoke.mock.calls.find(c => c[0] === 'submit_human_review');
    expect(call).toBeTruthy();
    expect((call![1] as Record<string, unknown>).note).toBe('空白项为无需填写，已核对');
  });

  it('标准场景下同样情形不强制写理由', async () => {
    const w = await mountReview({
      list_review_profiles: { success: true, message: '', data: [{ ...PROFILE, strictness: 'standard', strict_checks: [] }] },
      list_review_items: { success: true, message: '', data: [{ ...flaggedItem, ai_verdict: 'flag' }] },
      submit_human_review: { success: true, message: 'ok', data: null },
    });
    await expandFirst(w);
    await w.findAll('button').find(b => b.text() === '通过')!.trigger('click');
    await flushPromises();
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'submit_human_review')).toBe(true);
  });
});
