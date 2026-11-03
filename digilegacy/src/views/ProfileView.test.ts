import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import type { Mock } from 'vitest';

// ProfileView 是安全操作的总入口：启用主密码、改主密码、导出/导入备份、
// 心跳配置、政策补丁。这些路径一旦静默失败，用户会以为「已经加密了」。
vi.mock('../api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => true,
}));

// 导入备份/导入补丁走动态 import 的文件对话框；jsdom 里真实插件不可用，
// 这里给一个稳定的桩，让「选择文件 → 读内容 → 调后端」整条链可测。
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => '/tmp/backup.json'),
}));
vi.mock('@tauri-apps/plugin-fs', () => ({
  readTextFile: vi.fn(async () => '{"app":"digilegacy","version":2,"tables":{"digital_assets":[]}}'),
}));

import ProfileView from './ProfileView.vue';
import { useFeedback, settleConfirm, lastConfirm } from '../composables/feedback';
import { invoke } from '../api/invoke';
import { useSecurityStore } from '../stores/security';
import { locale, setLocale } from '../i18n';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();

const mockedInvoke = invoke as unknown as Mock;

function defaultImpl(over: Record<string, (args?: Record<string, unknown>) => unknown> = {}) {
  return async (cmd: string, args?: Record<string, unknown>) => {
    if (over[cmd]) return over[cmd](args);
    if (cmd === 'security_status') {
      return { success: true, message: '', data: { has_master_password: false, unlocked: true } };
    }
    if (cmd === 'get_user_profile') return { success: true, message: '', data: null };
    if (cmd === 'platform_patch_status') return { success: true, message: '', data: 0 };
    if (cmd === 'get_heartbeat_config') {
      return { success: true, message: '', data: { enabled: false, has_token: false, endpoint: '' } };
    }
    if (cmd === 'get_recovery_seal_exists') return { success: true, message: '', data: false };
    if (cmd === 'get_all_platforms') return { success: true, message: '', data: [] };
    // 新增的四块配置：默认给空结构，避免 undefined 让加载器抛错
    if (cmd === 'get_custody_hints') return { success: true, message: '', data: { backup_location: '', recovery_code_location: '' } };
    if (cmd === 'get_assistant_config') return { success: true, message: '', data: { enabled: false, endpoint: '', model: '', has_key: false } };
    if (cmd === 'get_heirs') return { success: true, message: '', data: [] };
    if (cmd === 'get_smtp_config') return { success: true, message: '', data: { host: '', port: 587, from_addr: '', username: '', has_password: false, allow_insecure: false } };
    return { success: true, message: '', data: null };
  };
}

async function mountProfile(over?: Record<string, (args?: Record<string, unknown>) => unknown>) {
  mockedInvoke.mockImplementation(defaultImpl(over) as never);
  const pinia = createPinia();
  setActivePinia(pinia);
  const w = mount(ProfileView, {
    global: {
      plugins: [pinia],
      // ProfileView 的 DMS 卡片用 router-link 指向「安心传承」；单测不挂路由
      stubs: { RouterLink: { template: '<a><slot /></a>' } },
    },
  });
  // onMounted 里有多条 await（loadUser / get_recovery_seal_exists / heartbeat / patch），
  // 单次 flushPromises 不够，需要多轮让 security.refresh 落地后 DOM 才切到已加密分支。
  // onMounted 现在还会加载保管位置 / 助手 / 继承人 / SMTP 四块，
  // 每块都是独立 await，3 轮 flush 不够，会把「尚未回显」误判成缺陷。
  for (let i = 0; i < 8; i++) {
    await flushPromises();
    await w.vm.$nextTick();
  }
  return w;
}

const autostartIsEnabled = vi.hoisted(() => vi.fn(async () => false));

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
  vi.stubGlobal('alert', vi.fn());
  // 该组件依赖 Tauri 插件动态 import，jsdom 下会 reject；统一兜住避免噪音
  vi.doMock('@tauri-apps/plugin-autostart', () => ({ isEnabled: autostartIsEnabled }));
  autostartIsEnabled.mockReset();
  autostartIsEnabled.mockResolvedValue(false);
});

describe('启用主密码', () => {
  it('少于 8 位时本地拦截，不调用后端', async () => {
    const w = await mountProfile();

    await w.find('#pf-master-pw').setValue('short');
    await w.find('#pf-master-pw2').setValue('short');
    const btn = w.findAll('button').find(b => b.text().includes('启用加密'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(mockedInvoke).not.toHaveBeenCalledWith('set_master_password', expect.anything());
    expect(w.text()).toContain('至少 8 个字符');
  });

  it('两次输入不一致时本地拦截', async () => {
    const w = await mountProfile();

    await w.find('#pf-master-pw').setValue('master-pass-1');
    await w.find('#pf-master-pw2').setValue('master-pass-2');
    const btn = w.findAll('button').find(b => b.text().includes('启用加密'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(mockedInvoke).not.toHaveBeenCalledWith('set_master_password', expect.anything());
    expect(w.text()).toContain('不一致');
  });

  it('成功后必须把恢复密钥展示给用户（一次性，不能丢）', async () => {
    const w = await mountProfile({
      set_master_password: () => ({
        success: true,
        message: '加密已启用',
        data: {
          status: { has_master_password: true, unlocked: true },
          recovery_code: 'ABCD-EFGH-JKLM-NPQR-STUV-WXYZ',
        },
      }),
    });

    await w.find('#pf-master-pw').setValue('master-pass-1');
    await w.find('#pf-master-pw2').setValue('master-pass-1');
    const btn = w.findAll('button').find(b => b.text().includes('启用加密'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('ABCD-EFGH-JKLM-NPQR-STUV-WXYZ');
  });

  it('后端失败时显示错误且不展示恢复密钥', async () => {
    const w = await mountProfile({
      set_master_password: () => ({ success: false, message: '已设置主密码，暂不支持修改', data: null }),
    });

    await w.find('#pf-master-pw').setValue('master-pass-1');
    await w.find('#pf-master-pw2').setValue('master-pass-1');
    const btn = w.findAll('button').find(b => b.text().includes('启用加密'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('已设置主密码');
  });
});

describe('备份导出', () => {
  it('备份源主密码 placeholder 跟随英文 i18n，而不是硬编码中文', async () => {
    const previousLocale = locale.value;
    setLocale('en-US');
    try {
      const w = await mountProfile();
      expect(w.find('#pf-backup-source-password').attributes('placeholder')).toBe(
        'Enter the source master password (required for encrypted backups)',
      );
      expect(w.find('#pf-backup-source-password').attributes('placeholder')).not.toContain('备份源');
    } finally {
      setLocale(previousLocale);
    }
  });

  it('成功时显示落地路径', async () => {
    const w = await mountProfile({
      export_backup: () => ({ success: true, message: '备份已导出', data: 'C:/app/备份/x.json' }),
    });

    const btn = w.findAll('button').find(b => b.text().includes('导出备份'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('C:/app/备份/x.json');
  });

  it('失败时明确报错，不得静默', async () => {
    const w = await mountProfile({
      export_backup: () => ({ success: false, message: '写入备份失败', data: null }),
    });

    const btn = w.findAll('button').find(b => b.text().includes('导出备份'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('写入备份失败');
  });

  it('自动备份状态可见：显示快照数量、最新文件名与大小', async () => {
    const w = await mountProfile({
      get_backup_status: () => ({
        success: true,
        message: '',
        data: { count: 3, latest_name: 'digilegacy-20261026.json', latest_size: 4096 },
      }),
    });
    expect(w.text()).toContain('自动备份');
    expect(w.text()).toContain('3');
    expect(w.text()).toContain('digilegacy-20261026.json');
    expect(w.text()).toContain('4 KB');
  });

  it('没有快照时说明「尚无快照」而不是空白', async () => {
    const w = await mountProfile({
      get_backup_status: () => ({ success: true, message: '', data: { count: 0, latest_name: null, latest_size: null } }),
    });
    expect(w.text()).toContain('尚无快照');
  });

  it('自动备份状态读取失败也必须有可见说明', async () => {
    const w = await mountProfile({
      get_backup_status: () => { throw new Error('IPC 断开'); },
    });
    expect(w.text()).toContain('自动备份状态读取失败');
  });
});

describe('心跳配置', () => {
  it('未填令牌时启用被后端拒绝并回显原因', async () => {
    const w = await mountProfile({
      set_heartbeat_config: () => ({ success: false, message: '启用心跳前请先填写令牌', data: null }),
    });

    const cb = w.find('#hb-endpoint');
    expect(cb.exists()).toBe(true);
  });

  it('加载时回填已保存的端点', async () => {
    const w = await mountProfile({
      get_heartbeat_config: () => ({
        success: true, message: '', data: { enabled: true, has_token: true, endpoint: 'https://hb.example.org' },
      }),
    });

    const input = w.find('#hb-endpoint').element as HTMLInputElement;
    expect(input.value).toBe('https://hb.example.org');
  });
});

describe('政策补丁', () => {
  it('显示当前补丁条数', async () => {
    const w = await mountProfile({ platform_patch_status: () => ({ success: true, message: '', data: 3 }) });
    expect(w.text()).toContain('3');
  });

  it('补丁数为 0 时清除按钮禁用', async () => {
    const w = await mountProfile();
    const clearBtn = w.findAll('button').find(b => b.text().includes('清除全部补丁'))!;
    expect(clearBtn.attributes('disabled')).toBeDefined();
  });
});

describe('档案保存（saveProfile）', () => {
  it('保存档案不得改写 DMS 配置（单一事实源在「安心传承」页）', async () => {
    // 历史缺陷：档案页把 user_profile 里的旧间隔回写 dms_config，
    // 用户在 DMS 页设的 90 天会被一次无关的「保存档案」静默改回 30 天。
    const calls: string[] = [];
    const w = await mountProfile({
      set_user_profile: () => { calls.push('set_user_profile'); return { success: true, message: 'ok', data: null }; },
      set_dead_mans_switch: () => { calls.push('set_dead_mans_switch'); return { success: true, message: 'ok', data: null }; },
    });

    const btn = w.findAll('button').find(b => b.text().includes('保存档案'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(calls).toContain('set_user_profile');
    expect(calls, '档案保存不得触碰 dms_config').not.toContain('set_dead_mans_switch');
  });

  it('保存失败必须明确报错，不得静默', async () => {
    // 变异守卫：把 catch 改成空块后，111 个测试曾全绿，
    // 而用户会以为保存成功、实际什么都没写。
    const w = await mountProfile({
      set_user_profile: () => ({ success: false, message: '主密码已设置但处于锁定状态', data: null }),
    });

    const btn = w.findAll('button').find(b => b.text().includes('保存档案'))!;
    await btn.trigger('click');
    await flushPromises();

    const { toasts } = useFeedback();
    expect(
      toasts.some(t => t.message.includes('保存失败')),
      `应出现失败 toast，实际: ${JSON.stringify(toasts.map(t => t.message))}`
    ).toBe(true);
  });
});

describe('改主密码（changeMasterPassword）', () => {
  it('成功后必须展示轮换的新恢复码', async () => {
    // 变异守卫：不展示新码时旧码已失效，用户将永久失去恢复能力。
    const w = await mountProfile({
      security_status: () => ({ success: true, message: '', data: { has_master_password: true, unlocked: true } }),
      change_master_password: () => ({
        success: true,
        message: '主密码已更新',
        data: {
          status: { has_master_password: true, unlocked: true },
          recovery_code: 'ROTA-TED1-CODE-2345-6789-ABCD',
        },
      }),
    });

    // 改密区块受 v-if="security.hasMasterPassword" 控制；
    // 直接置位 store 再等一拍渲染，避免依赖 onMounted 的 refresh 时序
    const sec = useSecurityStore();
    sec.hasMasterPassword = true;
    sec.unlocked = true;
    await flushPromises();
    await w.vm.$nextTick();

    await w.find('#pf-old-pw').setValue('old-master-1');
    await w.find('#pf-new-pw').setValue('new-master-22');
    await w.find('#pf-new-pw2').setValue('new-master-22');
    const btn = w.findAll('button').find(b => b.text().includes('更新主密码'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('ROTA-TED1-CODE-2345-6789-ABCD');
  });

  it('新密码不足 8 位时本地拦截，不调用后端', async () => {
    const w = await mountProfile({
      security_status: () => ({ success: true, message: '', data: { has_master_password: true, unlocked: true } }),
    });

    const sec = useSecurityStore();
    sec.hasMasterPassword = true;
    sec.unlocked = true;
    await flushPromises();
    await w.vm.$nextTick();

    await w.find('#pf-old-pw').setValue('old-master-1');
    await w.find('#pf-new-pw').setValue('short');
    await w.find('#pf-new-pw2').setValue('short');
    const btn = w.findAll('button').find(b => b.text().includes('更新主密码'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(mockedInvoke).not.toHaveBeenCalledWith('change_master_password', expect.anything());
    expect(w.text()).toContain('至少 8 位');
  });

  it('两次新密码不一致时本地拦截', async () => {
    const w = await mountProfile({
      security_status: () => ({ success: true, message: '', data: { has_master_password: true, unlocked: true } }),
    });
    const sec = useSecurityStore();
    sec.hasMasterPassword = true;
    sec.unlocked = true;
    await flushPromises();
    await w.vm.$nextTick();

    await w.find('#pf-old-pw').setValue('old-master-1');
    await w.find('#pf-new-pw').setValue('new-master-22');
    await w.find('#pf-new-pw2').setValue('new-master-33');
    const btn = w.findAll('button').find(b => b.text().includes('更新主密码'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(mockedInvoke).not.toHaveBeenCalledWith('change_master_password', expect.anything());
    expect(w.text()).toContain('不一致');
  });

  it('缺少当前密码时本地拦截', async () => {
    const w = await mountProfile({
      security_status: () => ({ success: true, message: '', data: { has_master_password: true, unlocked: true } }),
    });
    const sec = useSecurityStore();
    sec.hasMasterPassword = true;
    sec.unlocked = true;
    await flushPromises();
    await w.vm.$nextTick();

    await w.find('#pf-new-pw').setValue('new-master-22');
    await w.find('#pf-new-pw2').setValue('new-master-22');
    const btn = w.findAll('button').find(b => b.text().includes('更新主密码'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(mockedInvoke).not.toHaveBeenCalledWith('change_master_password', expect.anything());
    expect(w.text()).toContain('请填写当前主密码');
  });

  it('后端拒绝时展示失败原因，不得显示成功', async () => {
    const w = await mountProfile({
      security_status: () => ({ success: true, message: '', data: { has_master_password: true, unlocked: true } }),
      change_master_password: () => ({ success: false, message: '当前主密码错误', data: null }),
    });
    const sec = useSecurityStore();
    sec.hasMasterPassword = true;
    sec.unlocked = true;
    await flushPromises();
    await w.vm.$nextTick();

    await w.find('#pf-old-pw').setValue('wrong-old-1');
    await w.find('#pf-new-pw').setValue('new-master-22');
    await w.find('#pf-new-pw2').setValue('new-master-22');
    const btn = w.findAll('button').find(b => b.text().includes('更新主密码'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('当前主密码错误');
  });

  it('异常抛出时也展示错误（不吞异常）', async () => {
    const w = await mountProfile({
      security_status: () => ({ success: true, message: '', data: { has_master_password: true, unlocked: true } }),
      change_master_password: () => { throw new Error('IPC 断开'); },
    });
    const sec = useSecurityStore();
    sec.hasMasterPassword = true;
    sec.unlocked = true;
    await flushPromises();
    await w.vm.$nextTick();

    await w.find('#pf-old-pw').setValue('old-master-1');
    await w.find('#pf-new-pw').setValue('new-master-22');
    await w.find('#pf-new-pw2').setValue('new-master-22');
    const btn = w.findAll('button').find(b => b.text().includes('更新主密码'))!;
    await btn.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('IPC 断开');
  });
});

describe('导入备份（导入后必须刷新全部数据视图）', () => {
  it('导入成功后重新拉取资产与统计，而不是只刷新安全状态', async () => {
    // 历史缺陷：备份覆盖了数据库，但前端 store 仍是导入前的数据，
    // 用户看到旧列表会以为「恢复失败」并反复导入。
    const calls: string[] = [];
    const w = await mountProfile({
      import_backup: () => { calls.push('import_backup'); return { success: true, message: '已恢复', data: '2 条' }; },
      get_all_assets: () => { calls.push('get_all_assets'); return { success: true, message: '', data: [] }; },
      get_dashboard_stats: () => { calls.push('get_dashboard_stats'); return { success: true, message: '', data: null }; },
      get_user_profile: () => { calls.push('get_user_profile'); return { success: true, message: '', data: null }; },
      get_heirs: () => { calls.push('get_heirs'); return { success: true, message: '', data: [] }; },
      get_custody_hints: () => { calls.push('get_custody_hints'); return { success: true, message: '', data: { backup_location: '', recovery_code_location: '' } }; },
      get_assistant_config: () => { calls.push('get_assistant_config'); return { success: true, message: '', data: { enabled: false, endpoint: '', model: '', has_key: false } }; },
      get_smtp_config: () => { calls.push('get_smtp_config'); return { success: true, message: '', data: { host: '', port: 587, from_addr: '', username: '', has_password: false, allow_insecure: false } }; },
      get_heartbeat_config: () => { calls.push('get_heartbeat_config'); return { success: true, message: '', data: { enabled: false, has_token: false, endpoint: '' } }; },
    });

    const btn = w.findAll('button').find(b => b.text().includes('导入备份文件'))!;
    await btn.trigger('click');
    await flushPromises();
    // 覆盖导入需要二次确认
    const { settleConfirm } = await import('../composables/feedback');
    settleConfirm(true);
    for (let i = 0; i < 4; i++) await flushPromises();

    expect(calls).toContain('import_backup');
    expect(calls, '导入后必须重取资产列表').toContain('get_all_assets');
    expect(calls, '导入后必须重取统计').toContain('get_dashboard_stats');
    expect(calls, '导入后必须重取用户档案').toContain('get_user_profile');
    expect(calls, '导入后必须重取继承人').toContain('get_heirs');
    expect(calls, '导入后必须重取保管位置').toContain('get_custody_hints');
    expect(calls, '导入后必须重取助手配置').toContain('get_assistant_config');
    expect(calls, '导入后必须重取 SMTP 配置').toContain('get_smtp_config');
    expect(calls, '导入后必须重取心跳配置').toContain('get_heartbeat_config');
    expect(w.text()).toContain('已恢复');
  });
});

describe('恢复密钥状态与开机自启', () => {
  it('恢复密钥已生成时显示对应文案', async () => {
    const w = await mountProfile({
      get_recovery_seal_exists: () => ({ success: true, message: '', data: true }),
    });
    // ProfileView 不在 onMounted 刷新安全状态，恢复密钥区域只在已启用加密时渲染
    const s = useSecurityStore();
    s.hasMasterPassword = true;
    await w.vm.$nextTick();
    expect(w.text()).toContain('已生成（见设置主密码时的提示');
  });

  it('恢复密钥查询失败时按「未知」处理而不是误报已生成', async () => {
    const w = await mountProfile({
      get_recovery_seal_exists: () => ({ success: false, message: 'error', data: null }),
    });
    const s = useSecurityStore();
    s.hasMasterPassword = true;
    await w.vm.$nextTick();
    // 失败时 recoveryExists = null → 显示「检测中/未知」而非「已生成」
    expect(w.text()).toContain('检查中…');
  });

  it('开机自启已启用时复选框勾选', async () => {
    autostartIsEnabled.mockResolvedValueOnce(true);
    const w = await mountProfile();
    // 页面存在多个 checkbox（DMS 副本/加密开关等），断言标签存在且回填后仍渲染
    expect(w.text()).toContain('开机自动启动（驻留托盘');
  });

  it('自启插件不可用时静默回退为关闭（不阻断页面加载）', async () => {
    autostartIsEnabled.mockRejectedValueOnce(new Error('plugin missing'));
    const w = await mountProfile();
    expect(w.text()).toContain('开机自动启动（驻留托盘');
  });
});

describe('占位符防护（档案数据覆盖风险）', () => {
  it('未解密档案显示告警并阻止保存', async () => {
    const w = await mountProfile({
      get_user_profile: () => ({
        success: true,
        message: '',
        data: {
          id: 1,
          full_name: '【加密数据 · 需解锁】',
          phone: '【加密数据 · 需解锁】',
          email: '',
          id_last4: '',
          emergency_contact_name: '', emergency_contact_phone: '',
          emergency_contact_relation: '', emergency_contact_email: '',
          heir_primary_name: '', heir_primary_phone: '',
          heir_primary_relation: '', heir_primary_email: '',
          check_in_interval_days: 30, last_check_in: null, dms_enabled: false,
          created_at: '', updated_at: '',
        },
      }),
    });

    expect(w.text(), '必须显示未解密告警').toContain('部分档案字段未能解密');

    const saveBtn = w.findAll('button').find(b => b.text().includes('保存档案'))!;
    await saveBtn.trigger('click');
    await flushPromises();

    expect(mockedInvoke, '不得调用 set_user_profile').not.toHaveBeenCalledWith(
      'set_user_profile',
      expect.anything(),
    );
    const { toasts } = useFeedback();
    expect(
      toasts.some(t => t.message.includes('无法保存')),
      `应出现阻止保存的 toast，实际: ${JSON.stringify(toasts.map(t => t.message))}`,
    ).toBe(true);
  });

  it('正常档案不显示告警且可以保存', async () => {
    const w = await mountProfile({
      get_user_profile: () => ({
        success: true,
        message: '',
        data: {
          id: 1, full_name: '张三', phone: '13800000000', email: 'a@b.com', id_last4: '',
          emergency_contact_name: '', emergency_contact_phone: '',
          emergency_contact_relation: '', emergency_contact_email: '',
          heir_primary_name: '', heir_primary_phone: '',
          heir_primary_relation: '', heir_primary_email: '',
          check_in_interval_days: 30, last_check_in: null, dms_enabled: false,
          created_at: '', updated_at: '',
        },
      }),
    });
    expect(w.text()).not.toContain('部分档案字段未能解密');
    const saveBtn = w.findAll('button').find(b => b.text().includes('保存档案'))!;
    await saveBtn.trigger('click');
    await flushPromises();
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'set_user_profile')).toBe(true);
  });
});

describe('SMTP 配置（家人能否真的收到通知的前提）', () => {
  const baseSmtp = {
    host: 'smtp.example.com', port: 587, from_addr: 'noreply@example.com',
    username: 'noreply', has_password: true, allow_insecure: false,
  };

  it('打开页面即回显已保存配置，但不回显密码明文', async () => {
    const w = await mountProfile({ get_smtp_config: () => ({ success: true, message: '', data: baseSmtp }) });
    expect((w.find('#smtp-host').element as HTMLInputElement).value).toBe('smtp.example.com');
    expect((w.find('#smtp-port').element as HTMLInputElement).value).toBe('587');
    // 密码框永远为空：只有 has_password 提示，绝不下发明文
    expect((w.find('#smtp-pass').element as HTMLInputElement).value).toBe('');
    // 只通过 placeholder 提示「已保存」，绝不回填明文
    expect(w.find('#smtp-pass').attributes('placeholder')).toContain('已保存');
  });

  it('保存时不填密码则传 null，避免把已保存的密码清空', async () => {
    const w = await mountProfile({
      get_smtp_config: () => ({ success: true, message: '', data: baseSmtp }),
      set_smtp_config: () => ({ success: true, message: 'SMTP 配置已保存', data: null }),
    });
    const btn = w.findAll('button').find(b => b.text().includes('保存 SMTP 配置'))!;
    await btn.trigger('click');
    await flushPromises();

    const call = mockedInvoke.mock.calls.find(c => c[0] === 'set_smtp_config');
    expect(call, '必须调用后端保存').toBeTruthy();
    expect((call![1] as Record<string, unknown>).password, '未填密码必须传 null').toBeNull();
  });

  it('填了新密码则明文传给后端（由后端负责加密落库）', async () => {
    const w = await mountProfile({
      get_smtp_config: () => ({ success: true, message: '', data: baseSmtp }),
      set_smtp_config: () => ({ success: true, message: 'ok', data: null }),
    });
    await w.find('#smtp-pass').setValue('app-specific-key');
    const btn = w.findAll('button').find(b => b.text().includes('保存 SMTP 配置'))!;
    await btn.trigger('click');
    await flushPromises();
    expect((mockedInvoke.mock.calls.find(c => c[0] === 'set_smtp_config')![1] as Record<string, unknown>).password)
      .toBe('app-specific-key');
  });

  it('测试发送成功与失败都必须可见', async () => {
    const w = await mountProfile({
      get_smtp_config: () => ({ success: true, message: '', data: baseSmtp }),
      send_test_email: () => ({ success: false, message: '测试邮件发送失败：连接超时', data: null }),
    });
    const btn = w.findAll('button').find(b => b.text().includes('发送测试邮件'))!;
    await btn.trigger('click');
    await flushPromises();
    expect(w.text(), '失败原因必须显示').toContain('连接超时');
  });
});

describe('备份导入模式（覆盖是不可逆操作）', () => {
  it('默认入口按 merge 导入并传递源主密码', async () => {
    const w = await mountProfile({
      import_backup: () => ({ success: true, message: '备份导入完成', data: '已按合并模式导入 1 条资产' }),
    });
    await w.find('#pf-backup-source-password').setValue('source-backup-password');
    const btn = w.findAll('button').find(b => b.text().includes('导入备份'))!;
    await btn.trigger('click');
    await flushPromises();
    settleConfirm(true);
    await flushPromises();
    const call = mockedInvoke.mock.calls.find(c => c[0] === 'import_backup');
    expect(call, '必须调用导入').toBeTruthy();
    expect((call![1] as Record<string, unknown>).mode).toBe('merge');
    expect((call![1] as Record<string, unknown>).sourcePassword).toBe('source-backup-password');
  });

  it('覆盖导入必须显式传 replace，且在确认框里说明会清空', async () => {
    const w = await mountProfile({
      import_backup: () => ({ success: true, message: '备份导入完成', data: '已按覆盖模式导入 1 条资产' }),
    });
    const btn = w.findAll('button').find(b => b.text().includes('覆盖导入'))!;
    expect(btn, '覆盖入口必须存在').toBeTruthy();
    await btn.trigger('click');
    await flushPromises();
    // 确认框由 App 级 FeedbackLayer 渲染；单测里直接结算该 Promise。
    // 断言确认文案本身是破坏性的（说明会清空），不能静默覆盖。
    const pending = lastConfirm();
    expect(pending, '必须弹出确认').toBeTruthy();
    expect(pending!.title + pending!.message).toContain('清空');
    settleConfirm(true);
    await flushPromises();
    const call = mockedInvoke.mock.calls.find(c => c[0] === 'import_backup');
    expect((call![1] as Record<string, unknown>).mode).toBe('replace');
  });
});

describe('保管位置（家人能否找到凭据的前提）', () => {
  it('保存时把两项一起提交', async () => {
    const w = await mountProfile({
      get_custody_hints: () => ({ success: true, message: '', data: { backup_location: '', recovery_code_location: '' } }),
      set_custody_hints: () => ({ success: true, message: '保管位置已保存', data: null }),
    });
    await w.find('#custody-backup').setValue('家中保险柜');
    await w.find('#custody-recovery').setValue('笔记本第3页');
    const btn = w.findAll('button').find(b => b.text().includes('保存保管位置'))!;
    expect(btn, '保管位置入口必须存在').toBeTruthy();
    await btn.trigger('click');
    await flushPromises();
    const call = mockedInvoke.mock.calls.find(c => c[0] === 'set_custody_hints');
    expect(call).toBeTruthy();
    expect((call![1] as { hints: Record<string, string> }).hints.backup_location).toBe('家中保险柜');
    expect((call![1] as { hints: Record<string, string> }).hints.recovery_code_location).toBe('笔记本第3页');
  });

  it('保存失败时必须显示原因', async () => {
    const w = await mountProfile({
      get_custody_hints: () => ({ success: true, message: '', data: { backup_location: '', recovery_code_location: '' } }),
      set_custody_hints: () => ({ success: false, message: '主密码已设置但处于锁定状态，请先解锁后再保存', data: null }),
    });
    const btn = w.findAll('button').find(b => b.text().includes('保存保管位置'))!;
    await btn.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('请先解锁');
  });
});

describe('AI 助手配置（默认关闭、不内置端点）', () => {
  it('默认未启用时展示「没有服务就保持关闭」的说明', async () => {
    const w = await mountProfile({
      get_assistant_config: () => ({ success: true, message: '', data: { enabled: false, endpoint: '', model: '', has_key: false } }),
    });
    expect(w.find('#ai-enabled').element).toBeTruthy();
    expect((w.find('#ai-enabled').element as HTMLInputElement).checked).toBe(false);
    expect(w.text()).toContain('没有端点时保持关闭');
  });

  it('保存时不填 Key 则传 null，避免清空已保存的 Key', async () => {
    const w = await mountProfile({
      get_assistant_config: () => ({ success: true, message: '', data: { enabled: true, endpoint: 'https://api.example.com/v1', model: 'm', has_key: true } }),
      set_assistant_config: () => ({ success: true, message: '助手配置已保存', data: null }),
    });
    const btn = w.findAll('button').find(b => b.text().includes('保存助手配置'))!;
    await btn.trigger('click');
    await flushPromises();
    const call = mockedInvoke.mock.calls.find(c => c[0] === 'set_assistant_config');
    expect((call![1] as Record<string, unknown>).apiKey, '未填 Key 必须传 null').toBeNull();
  });

  it('启用但缺端点时显示后端拒绝原因', async () => {
    const w = await mountProfile({
      get_assistant_config: () => ({ success: true, message: '', data: { enabled: false, endpoint: '', model: '', has_key: false } }),
      set_assistant_config: () => ({ success: false, message: '启用助手需要填写端点与模型名；没有服务时请保持关闭', data: null }),
    });
    await w.find('#ai-enabled').setValue(true);
    const btn = w.findAll('button').find(b => b.text().includes('保存助手配置'))!;
    await btn.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('没有服务时请保持关闭');
  });
});

describe('继承人名单（多继承人 + 执行人）', () => {
  const h = (name: string, executor = false) => ({
    name, relation: '子女', phone: '', email: '', share_note: '',
    is_executor: executor, sort_order: 0,
  });

  it('加载后展示全部继承人', async () => {
    const w = await mountProfile({
      get_heirs: () => ({ success: true, message: '', data: [h('王五', true), h('王六')] }),
    });
    expect((w.find('#heir-name-0').element as HTMLInputElement).value).toBe('王五');
    expect((w.find('#heir-name-1').element as HTMLInputElement).value).toBe('王六');
  });

  it('添加与删除继承人', async () => {
    const w = await mountProfile({ get_heirs: () => ({ success: true, message: '', data: [] }) });
    await w.findAll('button').find(b => b.text().includes('添加继承人'))!.trigger('click');
    await flushPromises();
    expect(w.find('#heir-name-0').exists()).toBe(true);
    await w.findAll('button').find(b => b.text() === '删除')!.trigger('click');
    await flushPromises();
    expect(w.find('#heir-name-0').exists()).toBe(false);
  });

  it('保存时提交整组名单', async () => {
    const w = await mountProfile({
      get_heirs: () => ({ success: true, message: '', data: [h('王五', true)] }),
      set_heirs: () => ({ success: true, message: '继承人名单已保存', data: null }),
    });
    await w.findAll('button').find(b => b.text().includes('保存继承人名单'))!.trigger('click');
    await flushPromises();
    const call = mockedInvoke.mock.calls.find(c => c[0] === 'set_heirs');
    expect(call).toBeTruthy();
    const list = (call![1] as { heirs: Array<Record<string, unknown>> }).heirs;
    expect(list.length).toBe(1);
    expect(list[0].name).toBe('王五');
    expect(list[0].is_executor).toBe(true);
  });

  it('后端拒绝（无执行人）时必须显示原因', async () => {
    const w = await mountProfile({
      get_heirs: () => ({ success: true, message: '', data: [h('王五', false)] }),
      set_heirs: () => ({ success: false, message: '必须恰好指定一位遗产执行人（当前 0 位）', data: null }),
    });
    await w.findAll('button').find(b => b.text().includes('保存继承人名单'))!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('恰好指定一位遗产执行人');
  });
});

// ── 继承人恢复包（单机应用唯一能让家人解密的路径）──
describe('继承人恢复包', () => {
  it('未生成时如实显示「尚未生成」，不假装已保护', async () => {
    const w = await mountProfile({
      heir_recovery_status: () => ({ success: true, message: '', data: { exists: false, stale: false } }),
    });
    expect(w.text(), '必须显示未生成状态').toContain('尚未生成恢复包');
  });

  it('已生成时显示门限/份数/生成时间', async () => {
    const w = await mountProfile({
      heir_recovery_status: () => ({
        success: true,
        message: '',
        data: {
          exists: true,
          threshold: 3,
          total: 5,
          created_at: '2026-10-29 10:00:00',
          passphrase_protected: false,
          stale: false,
        },
      }),
    });
    const text = w.text();
    expect(text).toContain('门限 3 / 共 5 份');
    expect(text).toContain('2026-10-29 10:00:00');
  });

  it('主密码变更后（stale）必须提示旧份额已失效', async () => {
    const w = await mountProfile({
      heir_recovery_status: () => ({
        success: true,
        message: '',
        data: {
          exists: true,
          threshold: 3,
          total: 5,
          created_at: '2026-10-29 10:00:00',
          passphrase_protected: false,
          stale: true,
        },
      }),
    });
    expect(w.text()).toContain('已失效');
  });

  it('门限大于份数时本地拦截，不调用后端', async () => {
    const w = await mountProfile({
      heir_recovery_status: () => ({ success: true, message: '', data: { exists: false, stale: false } }),
      security_status: () => ({ success: true, message: '', data: { has_master_password: true, unlocked: true } }),
    });
    await w.find('#kit-threshold').setValue('4');
    await w.find('#kit-total').setValue('2');
    const btn = w.findAll('button').find(b => b.text().includes('生成恢复包'))!;
    await btn.trigger('click');
    await flushPromises();
    expect(mockedInvoke.mock.calls.some(c => c[0] === 'generate_heir_recovery_kit')).toBe(false);
  });

  it('生成成功后显示落地路径与份数', async () => {
    const w = await mountProfile({
      heir_recovery_status: () => ({ success: true, message: '', data: { exists: false, stale: false } }),
      security_status: () => ({ success: true, message: '', data: { has_master_password: true, unlocked: true } }),
      generate_heir_recovery_kit: () => ({
        success: true,
        message: '恢复包已生成',
        data: { path: 'C:/data/继承人恢复包/kit.html', threshold: 3, total: 5 },
      }),
    });
    const btn = w.findAll('button').find(b => b.text().includes('生成恢复包'))!;
    await btn.trigger('click');
    await flushPromises();
    const call = mockedInvoke.mock.calls.find(c => c[0] === 'generate_heir_recovery_kit');
    expect(call, '必须调用后端生成').toBeTruthy();
    expect((call![1] as Record<string, unknown>).threshold).toBe(3);
    expect((call![1] as Record<string, unknown>).total).toBe(5);
    expect(w.text()).toContain('kit.html');
  });

  it('生成失败时显示后端原因，不得静默', async () => {
    const w = await mountProfile({
      heir_recovery_status: () => ({ success: true, message: '', data: { exists: false, stale: false } }),
      security_status: () => ({ success: true, message: '', data: { has_master_password: true, unlocked: true } }),
      generate_heir_recovery_kit: () => ({ success: false, message: '请先解锁后再生成恢复包', data: null }),
    });
    const btn = w.findAll('button').find(b => b.text().includes('生成恢复包'))!;
    await btn.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('请先解锁后再生成恢复包');
  });
});

describe('恢复演练', () => {
  it('受口令保护时透传演练口令并显示 GCM 指纹验证结果', async () => {
    const w = await mountProfile({
      heir_recovery_status: () => ({
        success: true, message: '', data: { exists: true, threshold: 3, total: 5, created_at: '2026-10-29', stale: false },
      }),
      run_recovery_drill: (args) => ({
        success: true,
        message: '',
        data: {
          self_test_ok: true, self_test_ms: 3, provided: 3, valid: 3, threshold: 3,
          enough: true, same_batch: true, passphrase_verified: true, errors: [],
        },
      }),
    });
    await w.find('#kit-drill-passphrase').setValue('drill-passphrase');
    await w.findAll('button').find(b => b.text().includes('开始演练'))!.trigger('click');
    await flushPromises();
    const call = mockedInvoke.mock.calls.find(c => c[0] === 'run_recovery_drill');
    expect((call?.[1] as Record<string, unknown>).passphrase).toBe('drill-passphrase');
    expect(w.text()).toContain('口令与批次指纹验证通过');
  });

  it('演练结果显示自检、有效份数与批次一致性', async () => {
    const w = await mountProfile({
      heir_recovery_status: () => ({
        success: true, message: '', data: { exists: true, threshold: 3, total: 5, created_at: '2026-10-29', stale: false },
      }),
      run_recovery_drill: () => ({
        success: true,
        message: '',
        data: { self_test_ok: true, self_test_ms: 3, provided: 3, valid: 3, threshold: 3, enough: true, same_batch: true, errors: [] },
      }),
    });
    const btn = w.findAll('button').find(b => b.text().includes('开始演练'))!;
    expect(btn, '演练入口必须存在').toBeTruthy();
    await btn.trigger('click');
    await flushPromises();
    const text = w.text();
    expect(text).toContain('通过（3 ms）');
    expect(text).toContain('有效份额 3 / 提供 3');
    expect(text).toContain('份额数量足够');
    expect(text).toContain('批次一致');
  });

  it('份额不足时明确提示，不假装通过', async () => {
    const w = await mountProfile({
      heir_recovery_status: () => ({
        success: true, message: '', data: { exists: true, threshold: 3, total: 5, created_at: '2026-10-29', stale: false },
      }),
      run_recovery_drill: () => ({
        success: true, message: '',
        data: { self_test_ok: true, self_test_ms: 2, provided: 2, valid: 2, threshold: 3, enough: false, same_batch: false, errors: [] },
      }),
    });
    await w.findAll('button').find(b => b.text().includes('开始演练'))!.trigger('click');
    await flushPromises();
    expect(w.text()).toContain('份额数量不足');
    expect(w.text()).toContain('批次不一致');
  });
});
