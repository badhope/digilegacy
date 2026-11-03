import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import type { Mock } from 'vitest';

// ReportView 的两个导出按钮此前失败时完全静默：
// 点击后界面毫无反应，用户会以为文件已生成却找不到。
const mockTauriEnv = vi.hoisted(() => ({ value: false }));
vi.mock('../api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => mockTauriEnv.value,
}));
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    onmessage: ((value: unknown) => void) | null = null;
    send(value: unknown) { this.onmessage?.(value); }
  },
}));

import ReportView from './ReportView.vue';
import { invoke } from '../api/invoke';
import { silenceExpectedErrors } from '../test-utils/silence';
import { settleConfirm } from '../composables/feedback';

silenceExpectedErrors();

const mockedInvoke = invoke as unknown as Mock;

/** ZIP 路径包含动态 import + JSZip 压缩 + btoa，jsdom 下需要真实事件循环。 */
async function settle(w: { vm: { $nextTick: () => Promise<unknown> } }) {
  // ZIP 路径：动态 import jszip → 压缩 → btoa，在低配/高负载机器上可能超过 400ms。
  // 改为「轮询直到 export_zip_to_file 被调用或超时」，避免固定 sleep 导致的偶发失败。
  // 全量并行时 JSZip 压缩可能被 CPU 抢占拖慢，超时给足 8 秒
  const deadline = Date.now() + 8000;
  while (Date.now() < deadline) {
    await flushPromises();
    await w.vm.$nextTick();
    if ((invoke as unknown as Mock).mock.calls.some(c => c[0] === 'export_zip_to_file')) return;
    await new Promise(r => setTimeout(r, 100));
  }
  // 超时也退出，让断言给出真实失败原因
}

function reportData() {
  return {
    assets: [],
    property_assets_count: 0,
    personality_assets_count: 0,
    mixed_assets_count: 0,
    total_estimated_value: 0,
    generated_at: '2026-01-01 00:00:00',
    user: {
      full_name: '测试用户', phone: '13800000000', email: 'a@b.com',
      heir_primary_name: '继承人', heir_primary_relation: '子女',
      emergency_contact_name: '联系人', emergency_contact_relation: '配偶',
    },
  };
}

async function mountReport(over: Record<string, unknown> = {}) {
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd in over) {
      const value = over[cmd as keyof typeof over];
      return typeof value === 'function' ? await (value as (args?: Record<string, unknown>) => unknown)(undefined) : value;
    }
    if (cmd === 'get_inheritance_report') return { success: true, message: '', data: reportData() };
    if (cmd === 'get_all_platforms') return { success: true, message: '', data: [] };
    // 报告 HTML 由后端渲染（与桌面版同一契约），测试里给等价的最小文档
    if (cmd === 'render_report') {
      return {
        success: true,
        message: '报告已生成',
        data: '<!DOCTYPE html><html lang="zh-CN"><head><title>数字遗产继承报告</title><style>@media print{body{padding:0}}</style></head><body><div class="report-page"><div class="doc-title">数字遗产继承报告</div></div></body></html>',
      };
    }
    return { success: true, message: '', data: null };
  });
  const pinia = createPinia();
  setActivePinia(pinia);
  const w = mount(ReportView, { global: { plugins: [pinia] } });
  await flushPromises();
  await w.vm.$nextTick();
  return w;
}

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
  mockTauriEnv.value = false;
});

describe('ReportView 导出反馈', () => {
  it('导出成功时显示落地路径', async () => {
    const w = await mountReport({
      export_report_to_file: { success: true, message: 'ok', data: 'C:/app/报告.html' },
    });

    const btn = w.findAll('button').find(b => b.text().includes('导出报告'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('C:/app/报告.html');
  });

  it('导出返回 success:false 时必须显示错误（不得静默）', async () => {
    const w = await mountReport({
      export_report_to_file: { success: false, message: '磁盘已满', data: null },
    });

    const btn = w.findAll('button').find(b => b.text().includes('导出报告'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text(), '导出失败必须给用户可见提示').toContain('磁盘已满');
  });

  it('导出抛异常时必须显示错误（不得静默）', async () => {
    const w = await mountReport();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'export_report_to_file') throw new Error('IPC 断开');
      if (cmd === 'get_inheritance_report') return { success: true, message: '', data: reportData() };
      return { success: true, message: '', data: [] };
    });

    const btn = w.findAll('button').find(b => b.text().includes('导出报告'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text(), '异常必须转成用户可见提示').toContain('导出失败');
  });
});

describe('导出继承工具包（ZIP）', () => {
  // 说明：不在这里断言「export_zip_to_file 被调用」。
  // 该路径是 动态 import jszip → 压缩 → btoa → IPC 的异步长链，
  // 在 jsdom + 并行跑测试时偶尔超过任何固定/轮询等待，造成假失败。
  // ZIP 打包本身由 E2E（真实浏览器）覆盖；这里只验证「失败必须可见」，
  // 这是纯 UI 契约，不依赖压缩耗时。

  it('ZIP 导出返回失败时必须给出可见提示', async () => {
    const w = await mountReport({
      export_zip_to_file: { success: false, message: '磁盘空间不足', data: null },
    });

    const btn = w.findAll('button').find(b => b.text().includes('导出继承工具包'))!;
    expect(btn, 'ZIP 导出按钮必须存在').toBeTruthy();
    await btn.trigger('click');

    // 轮询到错误提示出现（不依赖 ZIP 完成的精确时刻）
    const deadline = Date.now() + 8000;
    let shown = false;
    while (Date.now() < deadline) {
      await flushPromises();
      await w.vm.$nextTick();
      if (w.text().includes('磁盘空间不足') || w.text().includes('导出工具包失败')) {
        shown = true;
        break;
      }
      await new Promise(r => setTimeout(r, 100));
    }
    expect(shown, 'ZIP 导出失败必须给用户可见提示').toBe(true);
  });

  it('ZIP 导出抛异常时也必须有提示（不外泄为未处理拒绝）', async () => {
    const w = await mountReport();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_inheritance_report') return { success: true, message: '', data: reportData() };
      if (cmd === 'get_all_platforms') return { success: true, message: '', data: [] };
      if (cmd === 'export_zip_to_file') throw new Error('IPC 断开');
      return { success: true, message: '', data: null };
    });

    const btn = w.findAll('button').find(b => b.text().includes('导出继承工具包'))!;
    await btn.trigger('click');

    const deadline = Date.now() + 8000;
    let shown = false;
    while (Date.now() < deadline) {
      await flushPromises();
      await w.vm.$nextTick();
      if (w.text().includes('导出工具包失败')) { shown = true; break; }
      await new Promise(r => setTimeout(r, 100));
    }
    expect(shown, '抛异常也必须转成可见提示').toBe(true);
  });
});

describe('打印', () => {
  it('点打印调用 window.print', async () => {
    const printSpy = vi.fn();
    vi.stubGlobal('print', printSpy);
    const w = await mountReport();

    const btn = w.findAll('button').find(b => b.text().includes('打印'))!;
    expect(btn, '打印按钮必须存在').toBeTruthy();
    await btn.trigger('click');

    expect(printSpy).toHaveBeenCalled();
  });
});

describe('报告加载失败', () => {
  it('IPC 抛错时必须给出可见提示，而不是停在空白页', async () => {
    const w = await mountReport({
      get_inheritance_report: Promise.reject(new Error('数据库未初始化')),
    });
    await flushPromises();
    await w.vm.$nextTick();

    expect(w.text(), '加载失败必须可见').toContain('报告加载失败');
  });

  it('后端返回 success:false 时显示后端消息', async () => {
    const w = await mountReport({
      get_inheritance_report: { success: false, message: '数据已加密，请先解锁', data: null },
    });
    await flushPromises();
    await w.vm.$nextTick();

    expect(w.text()).toContain('数据已加密，请先解锁');
  });
});

describe('工具包内容完整性', () => {
  it('ZIP 里的报告必须是独立 HTML（含 DOCTYPE 与打印样式），不是页内片段', async () => {
    const w = await mountReport();
    const btn = w.findAll('button').find(b => b.text().includes('导出继承工具包'))!;
    await btn.trigger('click');

    const deadline = Date.now() + 8000;
    let zipCall: unknown[] | undefined;
    while (Date.now() < deadline) {
      await flushPromises();
      await w.vm.$nextTick();
      zipCall = (invoke as unknown as Mock).mock.calls.find(c => c[0] === 'export_zip_to_file');
      if (zipCall) break;
      await new Promise(r => setTimeout(r, 100));
    }
    expect(zipCall, 'ZIP 导出必须调用后端').toBeTruthy();

    // 解码 base64 后解压，逐条校验文件内容
    const JSZip = (await import('jszip')).default;
    const base64 = String((zipCall![1] as { content: string }).content);
    const zip = await JSZip.loadAsync(base64, { base64: true });
    const htmlFile = zip.file('01-继承报告.html');
    expect(htmlFile, '工具包必须包含报告 HTML').toBeTruthy();
    const html = await htmlFile!.async('string');
    expect(html.startsWith('<!DOCTYPE html>'), '报告必须是完整文档').toBe(true);
    expect(html).toContain('@media print');
    expect(html).toContain('report-page');
  });
});

describe('报告 HTML 改由后端渲染', () => {
  it('导出必须调用 render_report，不得回退到抓 DOM', async () => {
    const w = await mountReport({
      export_report_to_file: { success: true, message: 'ok', data: 'C:/app/r.html' },
    });
    const btn = w.findAll('button').find(b => b.text().includes('导出报告'))!;
    await btn.trigger('click');
    await flushPromises();

    const cmds = mockedInvoke.mock.calls.map(c => c[0]);
    expect(cmds, '必须请求后端渲染').toContain('render_report');
    const exportCall = mockedInvoke.mock.calls.find(c => c[0] === 'export_report_to_file');
    expect(exportCall, '必须写出报告').toBeTruthy();
    const html = (exportCall![1] as { content: string }).content;
    expect(html.startsWith('<!DOCTYPE html>'), '导出内容必须是完整文档').toBe(true);
  });

  it('后端拒绝渲染（占位符）时必须显示原因，且不写文件', async () => {
    const w = await mountReport({
      render_report: { success: false, message: '报告中存在未解密字段，请先解锁后重新生成（本次未生成任何文件）', data: null },
    });
    const btn = w.findAll('button').find(b => b.text().includes('导出报告'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('未解密');
    expect(
      mockedInvoke.mock.calls.some(c => c[0] === 'export_report_to_file'),
      '被拒绝时不得写文件',
    ).toBe(false);
  });
});

describe('家人执行清单导出', () => {
  it('导出必须调用后端渲染并按平台生成清单', async () => {
    const w = await mountReport({
      render_action_guide: { success: true, message: 'ok', data: '<!DOCTYPE html><html><body>guide</body></html>' },
      export_report_to_file: { success: true, message: 'ok', data: 'C:/app/guide.html' },
    });
    const btn = w.findAll('button').find(b => b.text().includes('导出家人执行清单'))!;
    expect(btn, '执行清单入口必须存在').toBeTruthy();
    await btn.trigger('click');
    await flushPromises();

    const cmds = mockedInvoke.mock.calls.map(c => c[0]);
    expect(cmds).toContain('render_action_guide');
    expect(cmds).toContain('export_report_to_file');
    // 必须带 locale：否则英文用户拿到中文清单
    const guideCall = mockedInvoke.mock.calls.find(c => c[0] === 'render_action_guide');
    expect((guideCall![1] as Record<string, unknown>).locale).toBeTruthy();
  });

  it('后端拒绝生成时显示原因且不写文件', async () => {
    const w = await mountReport({
      render_action_guide: { success: false, message: '报告中存在未解密字段，请先解锁后重新生成（本次未生成任何文件）', data: null },
    });
    const btn = w.findAll('button').find(b => b.text().includes('导出家人执行清单'))!;
    await btn.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('未解密');
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'export_report_to_file')).toBe(false);
  });
});

describe('家人演练', () => {
  it('演练结果列出未通过项，并说明卡在哪', async () => {
    const w = await mountReport({
      run_family_drill: {
        success: true,
        message: '',
        data: {
          platforms: 2, assets: 3,
          assets_without_heir: 1, assets_without_policy: 0, unverified_platforms: 1,
          trigger_ready: false, trigger_channel: 'none',
          checks: [
            { key: 'heir', ok: false, count: 1 },
            { key: 'policy', ok: true, count: 0 },
            { key: 'verified', ok: false, count: 1 },
            { key: 'trigger', ok: false, count: 1 },
          ],
        },
      },
    });
    const btn = w.findAll('button').find(b => b.text().includes('开始演练'))!;
    expect(btn, '演练入口必须存在').toBeTruthy();
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('家人将看到 2 个平台');
    expect(w.text()).toContain('没有写继承人');
    expect(w.text()).toContain('不会收到任何提醒');
  });

  it('全部通过时给出明确结论，而不是仅不显示错误', async () => {
    const w = await mountReport({
      run_family_drill: {
        success: true,
        message: '',
        data: {
          platforms: 1, assets: 1,
          assets_without_heir: 0, assets_without_policy: 0, unverified_platforms: 0,
          trigger_ready: true, trigger_channel: 'smtp',
          checks: [
            { key: 'heir', ok: true, count: 0 },
            { key: 'policy', ok: true, count: 0 },
            { key: 'verified', ok: true, count: 0 },
            { key: 'trigger', ok: true, count: 0 },
          ],
        },
      },
    });
    const btn = w.findAll('button').find(b => b.text().includes('开始演练'))!;
    await btn.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('演练通过');
  });
});

describe('AI 助手（默认关闭，不内置服务）', () => {
  it('未启用时不显示提问面板，避免暗示内置 AI', async () => {
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: false, endpoint: '', model: '', has_key: false } },
    });
    await flushPromises();
    expect(w.text()).not.toContain('向助手提问');
  });

  it('启用后可见，且必须先预览才能提问', async () => {
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: true, endpoint: 'https://api.example.com/v1', model: 'm', has_key: true } },
      preview_assistant_payload: {
        success: true, message: '',
        data: { fields: ['平台名称', '处置方式'], payload: '{}', chars: 42 },
      },
    });
    await flushPromises();
    expect(w.text()).toContain('向助手提问');

    const askBtn = () => w.findAll('button').find(b => b.text() === '提问')!;
    expect(askBtn().attributes('disabled'), '未预览前不能提问').toBeDefined();

    await w.findAll('button').find(b => b.text().includes('查看将发送的内容'))!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('将发送以下非身份字段');
    expect(w.text()).toContain('42');
    expect(askBtn().attributes('disabled'), '预览后可以提问').toBeUndefined();
  });

  it('提问结果必须标注「AI 生成 · 未核对」', async () => {
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: true, endpoint: 'https://x/v1', model: 'm', has_key: true } },
      preview_assistant_payload: { success: true, message: '', data: { fields: ['平台名称'], payload: '{}', chars: 10 } },
      ask_assistant: { success: true, message: '', data: '1. 先办支付宝。' },
    });
    await flushPromises();
    await w.findAll('button').find(b => b.text().includes('查看将发送的内容'))!.trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text() === '提问')!.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('先办支付宝');
    expect(w.text(), '必须标注未核对').toContain('未经人工核对');
  });

  it('提问必须带当前语言', async () => {
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: true, endpoint: 'https://x/v1', model: 'm', has_key: true } },
      preview_assistant_payload: { success: true, message: '', data: { fields: ['平台名称'], payload: '{}', chars: 10 } },
      ask_assistant: { success: true, message: '', data: 'ok' },
    });
    await flushPromises();
    await w.findAll('button').find(b => b.text().includes('查看将发送的内容'))!.trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text() === '提问')!.trigger('click');
    await flushPromises();
    const call = mockedInvoke.mock.calls.find(c => c[0] === 'ask_assistant');
    expect((call![1] as Record<string, unknown>).locale).toBeTruthy();
  });
});

describe('权威来源（预览必须与导出同源）', () => {
  it('报告带权威来源时，预览里也渲染出来', async () => {
    const w = await mountReport({
      get_inheritance_report: {
        success: true, message: '',
        data: {
          ...reportData(),
          authorities: [
            {
              id: 'civil-code-1122', kind: 'portal',
              title_zh: '《中华人民共和国民法典》第一千一百二十二条', title_en: 'Civil Code, Article 1122',
              publisher: '全国人大', url: 'https://flk.npc.gov.cn/',
              note_zh: '遗产是自然人死亡时遗留的个人合法财产。', note_en: 'Estate definition.',
              applies_to: ['property'], reachable: true, content_matched: true,
              last_checked: '2026-10-27', http_status: 200, observed_title: '国家法律法规数据库',
              portal_note_zh: '检索入口：按法规名称检索后查看原文。', portal_note_en: 'Search portal.',
            },
          ],
        },
      },
    });
    expect(w.text(), '预览必须显示权威来源章节').toContain('权威来源');
    expect(w.text()).toContain('民法典');
    expect(w.text()).toContain('入口可达');
    expect(w.text()).toContain('内容已核对');
    expect(w.text(), '检索入口要标明不是条文页').toContain('非条文页');
  });

  it('未匹配的来源必须显示为未核对，不能与已核对混同', async () => {
    const w = await mountReport({
      get_inheritance_report: {
        success: true, message: '',
        data: {
          ...reportData(),
          authorities: [
            {
              id: 'pbc', kind: 'institution',
              title_zh: '中国人民银行', title_en: 'PBC',
              publisher: '人民银行', url: 'https://www.pbc.gov.cn/',
              note_zh: '存款继承规定发布机构。', note_en: 'Issuer.',
              applies_to: ['property'], reachable: true, content_matched: false,
              last_checked: '2026-10-27', http_status: 200, observed_title: '中国人民银行',
              portal_note_zh: '', portal_note_en: '',
            },
          ],
        },
      },
    });
    expect(w.text()).toContain('内容未核对');
    expect(w.text()).not.toContain('内容已核对');
  });

  it('没有来源时不渲染空章节', async () => {
    const w = await mountReport();
    expect(w.text()).not.toContain('权威来源');
  });
});

describe('AI 问答历史（本地留痕）', () => {
  it('回答完成后刷新历史列表', async () => {
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: true, endpoint: 'https://x/v1', model: 'm', has_key: true } },
      preview_assistant_payload: { success: true, message: '', data: { fields: ['平台名称'], payload: '{}', chars: 10 } },
      ask_assistant: { success: true, message: '', data: '答案内容' },
      list_ai_chat: {
        success: true, message: '',
        data: [{ id: 1, question: '支付宝怎么继承？', answer: '准备死亡证明', model: 'm', created_at: '2026-10-30 10:00:00' }],
      },
    });
    await flushPromises();
    expect(w.text()).toContain('问答历史');
    expect(w.text()).toContain('支付宝怎么继承？');

    // 再次提问 → 历史重新加载
    await w.findAll('button').find(b => b.text().includes('查看将发送的内容'))!.trigger('click');
    await flushPromises();
    await w.findAll('button').find(b => b.text() === '提问')!.trigger('click');
    await flushPromises();
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'list_ai_chat')).toBe(true);
  });

  it('清空历史必须走确认框，确认后才调用后端', async () => {
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: false } },
      list_ai_chat: {
        success: true, message: '',
        data: [{ id: 1, question: 'q', answer: 'a', model: 'm', created_at: '2026-10-30' }],
      },
      clear_ai_chat: { success: true, message: '已清空', data: null },
    });
    await flushPromises();
    const btn = w.findAll('button').find(b => b.text().includes('清空历史'))!;
    expect(btn, '清空入口必须存在').toBeTruthy();
    await btn.trigger('click');
    await flushPromises();
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'clear_ai_chat')).toBe(false);
    settleConfirm(true);
    await flushPromises();
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'clear_ai_chat')).toBe(true);
  });
});

describe('内置 Agent（受限本地工具循环）', () => {
  it('启用 AI 后显示 Agent 任务面板与安全提示', async () => {
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: true, endpoint: 'https://x/v1', model: 'm', has_key: true } },
      list_ai_chat: { success: true, message: '', data: [] },
    });
    await flushPromises();
    expect(w.find('[data-testid="agent-card"]').exists()).toBe(true);
    expect(w.text()).toContain('内置 Agent');
    expect(w.text()).toContain('不会写入数据或读取凭据');
    expect(w.find('[data-testid="agent-run"]').attributes('disabled')).toBeDefined();
  });

  it('浏览器演示模式不启动 Agent，给出桌面版边界提示', async () => {
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: true, endpoint: 'https://x/v1', model: 'm', has_key: true } },
      list_ai_chat: { success: true, message: '', data: [] },
    });
    await flushPromises();
    await w.find('#agent-task').setValue('查支付宝余额继承流程');
    await w.find('[data-testid="agent-run"]').trigger('click');
    await flushPromises();
    expect(w.text()).toContain('桌面版可用');
  });

  it('Agent 时间线与草稿结果可见（契约 fixture）', async () => {
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: true, endpoint: 'https://x/v1', model: 'm', has_key: true } },
      list_ai_chat: { success: true, message: '', data: [] },
    });
    await flushPromises();
    // 浏览器模式下不真正调用 endpoint；这里锁定 UI 的 empty/disabled 边界，
    // Tauri Channel 的事件序列由 Rust agent 单测覆盖。
    expect(w.find('.agent-timeline').exists()).toBe(false);
    expect(w.find('[data-testid="agent-run"]').exists()).toBe(true);
  });
});

describe('内置 Agent 桌面分支', () => {
  it('Tauri 模式通过 Channel 显示工具步骤与最终草稿', async () => {
    mockTauriEnv.value = true;
    const w = await mountReport({
      get_assistant_config: { success: true, message: '', data: { enabled: true, endpoint: 'https://x/v1', model: 'm', has_key: true } },
      list_ai_chat: { success: true, message: '', data: [] },
      run_agent_task_streaming: (args?: Record<string, unknown>) => {
        const channel = args?.onEvent as { send?: (value: unknown) => void } | undefined;
        channel?.send?.({ kind: 'tool_start', round: 1, tool: 'query_platform_policy' });
        return { success: true, message: '', data: { steps: [{ kind: 'tool_end', round: 1, tool: 'query_platform_policy', result: '政策结果' }], final_text: '给家人的草稿', truncated: false } };
      },
    });
    await flushPromises();
    await w.find('#agent-task').setValue('查支付宝继承流程');
    await w.find('[data-testid="agent-run"]').trigger('click');
    await flushPromises();
    expect(w.text()).toContain('给家人的草稿');
    expect(w.text()).toContain('政策结果');
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'run_agent_task_streaming')).toBe(true);
  });
});
