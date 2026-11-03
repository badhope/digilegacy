// Web 开发预览专用 mock：仅在 npm run dev 的浏览器环境生效
// 数据存 localStorage（明文），只用于查看前端界面外观，不得用于真实数据

import type {
  ApiResponse, CheckInStatus, DashboardStats, DigitalAsset, Platform, PlatformExtended, SecurityStatus, UserProfile,
} from '../types';
import platformsSeed from './mock-platforms.json';

const LS_KEY = 'digilegacy-web-preview-v1';
// 演示数据的结构版本。改了种子数据（新增字段/修正示例）就 +1：
// 否则老访客的 localStorage 会一直盖住新种子，界面上永远看不到修复。
const DEMO_SCHEMA = 2;
const SS_UNLOCK = 'digilegacy-web-preview-unlocked';

interface MockDms {
  enabled: boolean;
  interval: number;
  lastCheckIn: string | null;
}

interface MockState {
  assets: DigitalAsset[];
  profile: UserProfile | null;
  checkIns: string[];
  dms: MockDms;
  masterPassword: string | null;
  credentials: Record<string, string>;
  nextAssetId: number;
  __schema?: number;
  custody?: { backup_location: string; recovery_code_location: string };
  /** 演示态的 SMTP 设置：真实收发不发生，只用于让「触发链是否就绪」这一维
   *  与后端同一口径计算（后端 = DMS 启用 && SMTP 已配置）。 */
  smtp?: { host: string; port: number; from_addr: string; username: string; has_password: boolean };
  reviews?: Record<string, Array<{
    key: string; title: string; preview?: string; status: string;
    ai_verdict: string | null; ai_note: string; human_note: string;
    reviewed_at: string | null; stale: boolean;
  }>>;
  heirs?: Array<{
    id?: number; name: string; relation: string; phone: string; email: string;
    share_note: string; is_executor: boolean; sort_order: number;
  }>;
  aiChat?: Array<{ id: number; question: string; answer: string; model: string; created_at: string }>;
}

const seedPlatforms = platformsSeed as unknown as Array<Record<string, unknown>>;

function platformById(id: number): Record<string, unknown> | undefined {
  return seedPlatforms.find(p => p.id === id);
}

function platformIdByName(keyword: string): number {
  const hit = seedPlatforms.find(p => String(p.name).includes(keyword));
  return hit ? Number(hit.id) : Number(seedPlatforms[0].id);
}

function now(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

function defaultState(): MockState {
  const demo = (
    pid: number, ident: string, cat: string, value: number, method: string, heir?: string, notes = ''
  ): DigitalAsset => ({
    id: undefined as unknown as number,
    platform_id: pid,
    platform_name: String(platformById(pid)?.name || ''),
    account_identifier: ident,
    account_description: '',
    category: cat as DigitalAsset['category'],
    estimated_value: value,
    value_currency: 'CNY',
    disposal_method: method,
    heir_name: heir || null,
    heir_contact: null,
    notes,
    has_credential: false,
    is_confirmed: true,
    created_at: now(),
    updated_at: now(),
  });
  const assets = [
    demo(platformIdByName('支付宝'), '138****0000（演示）', 'property', 12000, 'inherit', '演示继承人'),
    demo(platformIdByName('B站') , 'demo@example.com', 'mixed', 3000, 'memorialize'),
    demo(platformIdByName('微信支付'), '138****0000', 'property', 500, 'inherit', '演示继承人', '仅处理零钱余额；微信社交账号本身不可继承'),
    demo(platformIdByName('Steam'), 'demo_user', 'property', 800, 'liquidate'),
  ];
  let id = 1;
  for (const a of assets) {
    a.id = id++;
  }
  return {
    assets,
    profile: {
      id: 1,
      full_name: '演示用户',
      phone: '138****0000',
      email: 'demo@example.com',
      id_last4: '',
      emergency_contact_name: '演示联系人',
      emergency_contact_phone: '139****0000',
      emergency_contact_relation: '子女',
      emergency_contact_email: 'contact@example.com',
      heir_primary_name: '演示继承人',
      heir_primary_phone: '137****0000',
      heir_primary_relation: '子女',
      heir_primary_email: 'heir@example.com',
      check_in_interval_days: 30,
      last_check_in: null,
      dms_enabled: true,
      created_at: now(),
      updated_at: now(),
    },
    checkIns: [],
    heirs: [
      {
        id: 1, name: '演示继承人', relation: '子女',
        phone: '137****0000', email: 'heir@example.com',
        share_note: '负责金融账户与主要资产', is_executor: true, sort_order: 0,
      },
    ],
    dms: { enabled: true, interval: 30, lastCheckIn: null },
    masterPassword: null,
    credentials: {},
    nextAssetId: id,
    custody: { backup_location: '', recovery_code_location: '' },
  };
}

function loadState(): MockState {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as MockState & { __schema?: number };
      // 结构版本不同 → 丢弃旧缓存并重建，避免演示页显示过时/矛盾的数据
      if ((parsed.__schema ?? 1) === DEMO_SCHEMA) return parsed;
    }
  } catch {
    // 忽略损坏的预览数据
  }
  const fresh = defaultState();
  saveState(fresh);
  return fresh;
}

function saveState(state: MockState) {
  localStorage.setItem(LS_KEY, JSON.stringify({ ...state, __schema: DEMO_SCHEMA }));
}

/** 按场景生成演示审核条目（结构与桌面版一致）。 */
function seedReviewItems(
  profileId: string,
  state: MockState,
): Array<{
  key: string; title: string; status: string;
  ai_verdict: string | null; ai_note: string; human_note: string;
  reviewed_at: string | null; stale: boolean;
}> {
  const rows: Array<[string, string]> =
    profileId === 'platform-policy'
      ? [
          ['1', '支付宝'], ['2', '微信支付'], ['3', '云闪付'], ['22', 'Steam'], ['16', 'B站(bilibili)'],
        ]
      : profileId === 'legal-template'
        ? [
            ['will-digital', '数字资产遗嘱（标准版）'],
            ['inheritance-declaration', '数字遗产继承声明书'],
            ['power-of-attorney-digital', '数字资产授权委托书'],
            ['affidavit-digital-asset', '数字资产继承宣誓书'],
            ['platform-application', '平台继承申请函'],
            ['inheritance-agreement', '继承人协议书'],
          ]
        : profileId === 'family-note'
          ? [
              ['backup-location', '加密备份存放位置'],
              ['recovery-location', '恢复码保管位置'],
            ]
          : profileId === 'asset-classification'
            ? state.assets.map((x, i) => [String(i + 1), String(x.platform_name)] as [string, string])
            : [];
  // 摘要必须与场景语义一致：家人说明场景下写「平台/所需材料」是错的，
  // 会让审核员以为自己在看政策条目。
  const previewFor = (title: string): string => {
    switch (profileId) {
      case 'platform-policy':
        return `平台：${title} · 所需材料：死亡证明、继承公证书 · 办理流程：联系平台官方客服提交材料 · 官方链接：待核实`;
      case 'legal-template':
        return `文书：${title} · 正文含签署主体、空白填写项、日期栏与见证人签字位`;
      case 'family-note':
        return `字段：${title} · 内容：家中书房保险柜第二层（示例）`;
      case 'asset-classification':
        return `平台：${title} · 资产类别：property · 处置方式：inherit`;
      default:
        return title;
    }
  };
  return rows.map(([key, title]) => ({
    key,
    title,
    preview: previewFor(title),
    status: 'pending',
    ai_verdict: null,
    ai_note: '',
    human_note: '',
    reviewed_at: null,
    stale: false,
  }));
}

function ok<T>(data: T, message = '操作成功'): ApiResponse<T> {
  return { success: true, message, data };
}

function fail<T>(message: string): ApiResponse<T> {
  return { success: false, message, data: null };
}

function isUnlocked(): boolean {
  return sessionStorage.getItem(SS_UNLOCK) === '1';
}

const VALID_CATEGORIES = ['property', 'personality', 'mixed'];
const VALID_DISPOSALS = ['inherit', 'liquidate', 'memorialize', 'authorize_view', 'delete', 'donate', 'no_action'];

function validateAssetRequest(req: Record<string, unknown>): string | null {
  if (Number(req.platform_id) <= 0) return '平台 id 必须为正数';
  const category = String(req.category ?? '');
  if (!VALID_CATEGORIES.includes(category)) return `无效的资产类别: ${category}`;
  const disposal = String(req.disposal_method ?? 'no_action');
  if (!VALID_DISPOSALS.includes(disposal)) return `无效的处置方式: ${disposal}`;
  const value = req.estimated_value as number | null | undefined;
  if (value != null && (!Number.isFinite(value) || value < 0 || value > 1e12)) {
    return '预估价值必须是 0 到 1 万亿之间的有限数值';
  }
  for (const [label, raw] of [
    ['账号标识', req.account_identifier],
    ['账号描述', req.account_description],
    ['备注', req.notes],
    ['继承人姓名', req.heir_name],
    ['继承人联系方式', req.heir_contact],
  ] as const) {
    if (raw != null && String(raw).length > 8_000) return `${label}过长（上限 8000 字符）`;
  }
  if (disposal === 'inherit' && !String(req.heir_name ?? '').trim()) {
    return '处置方式为「指定继承」时必须填写继承人姓名';
  }
  return null;
}

function toPlatform(entry: Record<string, unknown>): Platform {
  return {
    id: Number(entry.id),
    name: String(entry.name),
    category: String(entry.category),
    subtype: String(entry.subtype ?? ''),
    has_property: Boolean(entry.has_property),
    has_personality: Boolean(entry.has_personality),
    inheritable: Boolean(entry.inheritable),
    required_documents: String(entry.required_documents ?? ''),
    process_description: String(entry.process_description ?? ''),
    estimated_difficulty: String(entry.estimated_difficulty ?? ''),
    official_policy_url: String(entry.official_policy_url ?? ''),
    estimated_time_days: String(entry.estimated_time_days ?? ''),
    notes: String(entry.notes ?? ''),
  };
}

function withPlatformName(a: DigitalAsset): DigitalAsset {
  return { ...a, platform_name: String(platformById(a.platform_id)?.name || a.platform_name || '') };
}

function download(filename: string, blob: Blob) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const state = loadState();
  const a = (args || {}) as Record<string, never>;
  const req = (args?.req || {}) as Record<string, unknown>;

  switch (cmd) {
    case 'get_all_platforms':
      return ok(seedPlatforms.map(toPlatform)) as T;

    case 'get_all_platforms_extended':
      return ok(seedPlatforms.map(p => ({ ...toPlatform(p), name_en: String(p.name_en ?? ''), data_source: String(p.data_source ?? ''), inheritance_form_url: String(p.inheritance_form_url ?? ''), monetization_path: String(p.monetization_path ?? ''), legal_template_ids: String(p.legal_template_ids ?? ''), risk_warnings: String(p.risk_warnings ?? ''), tags: String(p.tags ?? '') })) as unknown as PlatformExtended[]) as T;

    case 'get_all_assets':
      return ok(state.assets.map(withPlatformName)) as T;

    case 'get_asset_by_id': {
      const hit = state.assets.find(x => x.id === Number(a['id']));
      return ok(hit ? withPlatformName(hit) : null) as T;
    }

    case 'get_assets_by_category':
      return ok(state.assets.filter(x => x.category === a['category']).map(withPlatformName)) as T;

    case 'add_asset': {
      const validationError = validateAssetRequest(req);
      if (validationError) return fail<null>(validationError) as T;
      const asset: DigitalAsset = {
        id: state.nextAssetId++,
        platform_id: Number(req.platform_id),
        platform_name: String(platformById(Number(req.platform_id))?.name || ''),
        account_identifier: String(req.account_identifier ?? ''),
        account_description: String(req.account_description ?? ''),
        category: String(req.category) as DigitalAsset['category'],
        estimated_value: (req.estimated_value as number | null) ?? null,
        value_currency: 'CNY',
        disposal_method: String(req.disposal_method ?? 'no_action'),
        heir_name: (req.heir_name as string) || null,
        heir_contact: (req.heir_contact as string) || null,
        notes: String(req.notes ?? ''),
        has_credential: false,
        is_confirmed: true,
        created_at: now(),
        updated_at: now(),
      };
      state.assets.unshift(asset);
      saveState(state);
      return ok(asset.id, '数字资产添加成功') as T;
    }

    case 'update_asset': {
      const target = state.assets.find(x => x.id === Number(a['id']));
      if (!target) return fail<null>('资产不存在或已被删除') as T;
      const validationError = validateAssetRequest(req);
      if (validationError) return fail<null>(validationError) as T;
      Object.assign(target, {
        platform_id: Number(req.platform_id),
        account_identifier: String(req.account_identifier ?? ''),
        account_description: String(req.account_description ?? ''),
        category: String(req.category),
        estimated_value: (req.estimated_value as number | null) ?? null,
        disposal_method: String(req.disposal_method ?? 'no_action'),
        heir_name: (req.heir_name as string) || null,
        heir_contact: (req.heir_contact as string) || null,
        notes: String(req.notes ?? ''),
        updated_at: now(),
      });
      saveState(state);
      return ok(null, '资产更新成功') as T;
    }

    case 'delete_asset': {
      state.assets = state.assets.filter(x => x.id !== Number(a['id']));
      delete state.credentials[String(a['id'])];
      saveState(state);
      return ok(null, '资产已删除') as T;
    }

    case 'set_disposal_wish': {
      const target = state.assets.find(x => x.id === Number(a['asset_id']));
      if (!target) return fail<null>('资产不存在') as T;
      const disposal = String(a['disposal_method'] ?? '');
      if (!VALID_DISPOSALS.includes(disposal)) return fail<null>(`无效的处置方式: ${disposal}`) as T;
      const heirName = (a['heir_name'] as string) || '';
      const heirContact = (a['heir_contact'] as string) || '';
      const notes = String(a['notes'] ?? '');
      if (disposal === 'inherit' && !heirName.trim()) return fail<null>('处置方式为「指定继承」时必须填写继承人姓名') as T;
      if (heirName.length > 8_000 || heirContact.length > 8_000 || notes.length > 8_000) {
        return fail<null>('处置意愿字段过长（上限 8000 字符）') as T;
      }
      target.disposal_method = disposal;
      target.heir_name = (a['heir_name'] as string) || null;
      target.heir_contact = (a['heir_contact'] as string) || null;
      target.notes = String(a['notes'] ?? '');
      saveState(state);
      return ok(null, '处置意愿已设定') as T;
    }

    case 'get_user_profile':
      return ok(state.profile) as T;

    case 'set_user_profile': {
      if (!args?.profile || typeof args.profile !== 'object') {
        return fail<null>('用户档案参数缺失') as T;
      }
      const profile = args.profile as UserProfile;
      state.profile = { ...profile, updated_at: now() };
      saveState(state);
      return ok(null, '用户档案已保存') as T;
    }

    case 'get_inheritance_report': {
      const assets = state.assets.map(withPlatformName);
      const total = assets.reduce((sum, x) => sum + (x.estimated_value || 0), 0);
      // 与桌面版同契约：政策随报告一起下发，预览与导出读同一份。
      const policyIds = Array.from(new Set(assets.map(x => x.platform_id)));
      const policies = policyIds.map(pid => {
        const p = platformById(pid);
        return {
          platform_id: pid,
          name: String(p?.name ?? ''),
          required_documents: String(p?.required_documents ?? ''),
          process_description: String(p?.process_description ?? ''),
          estimated_difficulty: String(p?.estimated_difficulty ?? ''),
          estimated_time_days: String(p?.estimated_time_days ?? ''),
          official_policy_url: String(p?.official_policy_url ?? ''),
          monetization_path: String(p?.monetization_path ?? ''),
          risk_warnings: String(p?.risk_warnings ?? ''),
          tags: String(p?.tags ?? ''),
          has_source: false,
        };
      });
      // 与桌面版同契约：报告带权威来源，预览与导出读同一份
      const authority = (id: string, kind: string, zh: string, en: string, note: string, noteEn: string, matched: boolean, portalZh = '', portalEn = '') => ({
        id, kind, title_zh: zh, title_en: en, publisher: '演示', url: 'https://flk.npc.gov.cn/',
        note_zh: note, note_en: noteEn, applies_to: ['property'],
        reachable: true, content_matched: matched, last_checked: '2026-10-27',
        http_status: 200, observed_title: '国家法律法规数据库',
        portal_note_zh: portalZh, portal_note_en: portalEn,
      });
      return ok({
        user: state.profile || { full_name: '未设置' },
        assets,
        policies,
        authorities: [
          authority('civil-code-1122', 'portal', '《中华人民共和国民法典》第一千一百二十二条', 'Civil Code, Article 1122', '遗产是自然人死亡时遗留的个人合法财产。', 'An estate is the lawful personal property left at death.', true, '检索入口：按法规名称检索后查看原文。', 'Search portal: look up the statute by name.'),
          authority('pbc', 'institution', '中国人民银行', "People's Bank of China", '存款继承相关规定的发布机构。', 'Issuer of rules on deposit inheritance.', false),
        ],
        total_estimated_value: total,
        property_assets_count: assets.filter(x => x.category === 'property').length,
        personality_assets_count: assets.filter(x => x.category === 'personality').length,
        mixed_assets_count: assets.filter(x => x.category === 'mixed').length,
        generated_at: now(),
      }) as T;
    }

    case 'get_dashboard_stats': {
      const assets = state.assets;
      const planned = assets.filter(x => x.disposal_method !== 'no_action').length;
      // 与桌面版同一口径：继承必须有继承人，占位符不算已填。
      const heirAssigned = assets.filter(
        x => x.disposal_method !== 'inherit' || Boolean(x.heir_name && x.heir_name.trim()),
      ).length;
      const identifierReady = assets.filter(
        x => x.account_identifier && x.account_identifier.trim() && !x.account_identifier.includes('需解锁'),
      ).length;
      // 与桌面版同口径：只有邮件通道可用才算触发链达标；
      // 仅心跳时家人收不到邮件，不能算完成。
      //
      // 演示环境不连接真实 SMTP（get_smtp_config 恒返回未配置），
      // 因此这里必须走与后端相同的判定而不是写死 false——写死会让
      // 「触发链」这一维在任何演示状态下都恒为 0，四维完成度永远不到 100%，
      // 与「配置了 SMTP 就应达标」的后端契约不一致。
      const smtpConfigured = Boolean(state.smtp?.host && state.smtp?.from_addr);
      const triggerReady = state.dms.enabled && smtpConfigured;
      const ratio = (n: number) => (assets.length > 0 ? Math.round((n * 100) / assets.length) : 0);
      const completion = assets.length > 0
        ? Math.round((ratio(planned) + ratio(heirAssigned) + ratio(identifierReady) + (triggerReady ? 100 : 0)) / 4)
        : 0;
      const stats: DashboardStats = {
        total_assets: assets.length,
        property_count: assets.filter(x => x.category === 'property').length,
        personality_count: assets.filter(x => x.category === 'personality').length,
        mixed_count: assets.filter(x => x.category === 'mixed').length,
        disposal_planned_count: planned,
        heir_assigned_count: heirAssigned,
        identifier_ready_count: identifierReady,
        trigger_ready: triggerReady,
        trigger_channel: state.dms.enabled ? 'heartbeat' : 'none',
        total_estimated_value: assets.reduce((sum, x) => sum + (x.estimated_value || 0), 0),
        last_check_in: state.dms.lastCheckIn,
        dms_enabled: state.dms.enabled,
        completion_percentage: completion,
        // 与桌面版同口径：把「还差什么」算成可点击的待办
        issues: (() => {
          const list: Array<{ key: string; severity: 'blocker' | 'warn' | 'info'; count: number; target: string }> = [];
          const noDisposal = assets.filter(x => x.disposal_method === 'no_action').length;
          if (noDisposal > 0) list.push({ key: 'missing_disposal', severity: 'warn', count: noDisposal, target: '/assets' });
          const noHeir = assets.filter(x => x.disposal_method === 'inherit' && !(x.heir_name || '').trim()).length;
          if (noHeir > 0) list.push({ key: 'missing_heir', severity: 'blocker', count: noHeir, target: '/assets' });
          const noIdent = assets.filter(x => !(x.account_identifier || '').trim()).length;
          if (noIdent > 0) list.push({ key: 'missing_identifier', severity: 'warn', count: noIdent, target: '/assets' });
          const noCred = assets.filter(x => !x.has_credential && ['inherit', 'liquidate', 'authorize_view'].includes(x.disposal_method)).length;
          if (noCred > 0) list.push({ key: 'no_credential', severity: 'info', count: noCred, target: '/assets' });
          if (!state.custody?.backup_location || !state.custody?.recovery_code_location) {
            list.push({ key: 'custody_missing', severity: 'blocker', count: 0, target: '/profile' });
          }
          // 演示数据里所有平台政策都未核实，按资产去重后统计
          const verifiedPlatforms = new Set<number>();
          if (verifiedPlatforms.size < new Set(assets.map(x => x.platform_id)).size) {
            list.push({
              key: 'policy_unverified',
              severity: 'info',
              count: new Set(assets.map(x => x.platform_id)).size,
              target: '/review',
            });
          }
          if (!triggerReady) {
            list.push({
              key: state.dms.enabled ? 'trigger_no_channel' : 'trigger_disabled',
              severity: 'blocker', count: 0, target: '/profile',
            });
          }
          return list;
        })(),
      };
      return ok(stats) as T;
    }

    case 'update_check_in': {
      const ts = now();
      state.checkIns.push(ts);
      state.dms.lastCheckIn = ts;
      if (state.profile) state.profile.last_check_in = ts;
      saveState(state);
      return ok(ts, '检查完成，状态良好') as T;
    }

    case 'get_check_in_status': {
      const interval = state.dms.interval;
      let nextDue: string | null = null;
      let daysUntil: number | null = null;
      if (state.dms.lastCheckIn) {
        const last = new Date(state.dms.lastCheckIn.replace(' ', 'T'));
        const due = new Date(last.getTime() + interval * 86400000);
        nextDue = due.toISOString().slice(0, 10);
        daysUntil = Math.ceil((due.getTime() - Date.now()) / 86400000);
      }
      const status: CheckInStatus = {
        last_check_in: state.dms.lastCheckIn,
        interval_days: interval,
        dms_enabled: state.dms.enabled,
        next_check_in_due: nextDue,
        days_until_due: daysUntil,
        recent_check_ins: [...state.checkIns].reverse().slice(0, 5),
      };
      return ok(status) as T;
    }

    case 'set_dead_mans_switch': {
      const interval = Number(a['intervalDays'] ?? a['interval_days'] ?? 30);
      if (!Number.isInteger(interval) || interval < 1 || interval > 365) {
        return fail<null>('检查间隔必须为 1 到 365 天') as T;
      }
      state.dms.enabled = Boolean(a['enabled']);
      state.dms.interval = interval;
      saveState(state);
      return ok(null, 'Dead Man\'s Switch 设置已保存') as T;
    }

    case 'evaluate_dms':
      return ok({ enabled: state.dms.enabled, overdue: false, current_stage: 0, days_overdue: 0, notifications: [] }) as T;

    case 'get_dms_status':
      return ok({ enabled: state.dms.enabled, overdue: false, current_stage: 0, days_overdue: 0 }) as T;

    case 'get_dms_history':
      return ok([]) as T;

    case 'render_report': {
      // 演示环境用同一份结构的最小 HTML，保持与桌面版同一命令契约
      const r = (await mockInvoke('get_inheritance_report')) as { data?: { assets?: unknown[] } };
      const count = r.data?.assets?.length ?? 0;
      return ok(
        `<!DOCTYPE html><html lang="zh-CN"><head><meta charset="UTF-8"><title>数字遗产继承报告</title>` +
        `<style>@media print{body{padding:0}}</style></head><body>` +
        `<div class="doc-title">数字遗产继承报告</div><p>演示报告：共 ${count} 条资产。</p></body></html>`,
        '报告已生成',
      ) as T;
    }

    case 'export_report_to_file': {
      download('继承报告.html', new Blob([String(a['content'] ?? '')], { type: 'text/html;charset=utf-8' }));
      return ok('（浏览器预览）已下载 继承报告.html', '报告已导出') as T;
    }

    case 'export_zip_to_file': {
      const b64 = String(a['content'] ?? '');
      const bin = atob(b64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      download('继承工具包.zip', new Blob([bytes], { type: 'application/zip' }));
      return ok('（浏览器预览）已下载 继承工具包.zip', '继承工具包已导出') as T;
    }

    case 'security_status': {
      const status: SecurityStatus = { has_master_password: state.masterPassword !== null, unlocked: state.masterPassword === null ? true : isUnlocked() };
      return ok(status) as T;
    }

    case 'set_master_password': {
      const password = String(a['password'] ?? '');
      if (password.length < 8) return fail<null>('主密码至少需要 8 个字符') as T;
      if (state.masterPassword !== null) return fail<null>('已设置主密码，暂不支持修改') as T;
      state.masterPassword = password;
      saveState(state);
      sessionStorage.setItem(SS_UNLOCK, '1');
      // 与桌面端一致：恢复密钥放在 data 字段返回（预览模式为演示数据）
      return ok({ status: { has_master_password: true, unlocked: true }, recovery_code: 'ABCD-EFGH-JKLM-NPQR-STUV-WXYZ' }, '演示模式未加密/仅用于展示') as T;
    }

    case 'unlock': {
      if (state.masterPassword === null) return fail<null>('尚未设置主密码') as T;
      if (String(a['password'] ?? '') !== state.masterPassword) return fail<null>('主密码错误') as T;
      sessionStorage.setItem(SS_UNLOCK, '1');
      return ok({ has_master_password: true, unlocked: true }) as T;
    }

    case 'lock':
      sessionStorage.removeItem(SS_UNLOCK);
      return ok({ has_master_password: state.masterPassword !== null, unlocked: false }, '已锁定') as T;

    case 'change_master_password': {
      const oldPw = String(a['oldPassword'] ?? a['old_password'] ?? '');
      const newPw = String(a['newPassword'] ?? a['new_password'] ?? '');
      const confirmPw = String(a['confirmPassword'] ?? a['confirm_password'] ?? '');
      if (newPw.length < 8) return fail<null>('新主密码至少需要 8 个字符') as T;
      if (newPw !== confirmPw) return fail<null>('两次输入的新主密码不一致') as T;
      if (state.masterPassword === null) return fail<null>('尚未设置主密码') as T;
      if (oldPw !== state.masterPassword) return fail<null>('当前主密码错误') as T;
      state.masterPassword = newPw;
      saveState(state);
      return ok({ has_master_password: true, unlocked: true }, '主密码已更新（预览模式）') as T;
    }

    case 'set_asset_credential': {
      const id = String(a['id']);
      const credential = String(a['credential'] ?? '');
      if (credential && state.masterPassword !== null && !isUnlocked()) return fail<null>('请先解锁后再保存凭据') as T;
      if (credential) {
        state.credentials[id] = credential;
      } else {
        delete state.credentials[id];
      }
      const target = state.assets.find(x => x.id === Number(a['id']));
      if (target) target.has_credential = Boolean(credential);
      saveState(state);
      return ok(null, credential ? '演示模式未加密/仅用于展示' : '凭据已清除') as T;
    }

    case 'get_asset_credential': {
      if (state.masterPassword !== null && !isUnlocked()) return fail<null>('数据已加密，请先解锁') as T;
      return ok(state.credentials[String(a['id'])] ?? '') as T;
    }

    case 'export_backup': {
      const backup = {
        app: 'digilegacy',
        version: 3,
        created_at: now(),
        tables: {
          digital_assets: state.assets,
          user_profile: state.profile ? [state.profile] : [],
          check_in_log: state.checkIns.map((t, i) => ({ id: i + 1, checked_at: t, status: 'ok', note: '' })),
          dms_config: state.dms,
          security_config: state.masterPassword ? { kdf_salt: 'preview', verifier: 'preview' } : null,
          heartbeat_config: null,
          platform_overrides: [],
        },
      };
      download(`归零加密备份_预览_${Date.now()}.json`, new Blob([JSON.stringify(backup, null, 2)], { type: 'application/json' }));
      return ok('（浏览器预览）已下载备份 JSON', '备份已导出') as T;
    }

    case 'import_backup': {
      try {
        const parsed = JSON.parse(String(a['content'] ?? ''));
        if (parsed.app !== 'digilegacy' || ![1, 2, 3].includes(parsed.version)) return fail<null>('不是有效的归零备份文件') as T;
        const tables = parsed.tables || {};
        const incoming = (tables.digital_assets || []).map((x: DigitalAsset) => withPlatformName(x));
        // 与桌面版同一语义：merge 追加、replace 清空。演示环境也必须一致，
        // 否则用户会在演示里以为「导入=覆盖」而不敢用，或反之误覆盖真实数据。
        const mode = String(a['mode'] ?? 'merge');
        if (mode === 'replace') {
          state.assets = incoming;
          state.profile = (tables.user_profile || [])[0] || null;
          state.checkIns = (tables.check_in_log || []).map((x: { checked_at: string }) => x.checked_at);
        } else {
          let nextId = state.assets.reduce((m, x) => Math.max(m, x.id || 0), 0) + 1;
          for (const item of incoming) {
            state.assets.push({ ...item, id: nextId++ });
          }
        }
        state.dms = { enabled: Boolean(tables.dms_config?.enabled), interval: Number(tables.dms_config?.check_in_interval_days ?? 30), lastCheckIn: tables.dms_config?.last_check_in ?? null };
        state.nextAssetId = state.assets.reduce((m, x) => Math.max(m, x.id || 0), 0) + 1;
        saveState(state);
        sessionStorage.removeItem(SS_UNLOCK);
        if (tables.security_config) state.masterPassword = state.masterPassword ?? 'preview';
        return ok(`已导入 ${incoming.length} 条资产（预览模式，${String(a['mode'] ?? 'merge') === 'replace' ? '覆盖' : '合并'}）`, '备份导入完成') as T;
      } catch (e) {
        return fail<null>('备份文件解析失败：' + String(e)) as T;
      }
    }

    case 'open_external': {
      const url = String(a['url'] ?? '');
      if (!/^https?:\/\//i.test(url)) return fail<null>('仅允许打开 http/https 链接') as T;
      window.open(url, '_blank', 'noopener');
      return ok(null, '已在浏览器打开') as T;
    }

    case 'get_heartbeat_config':
      // 演示环境的心跳状态：始终未配置，界面据此显示「家人收不到邮件」。
      // 名称与返回结构与桌面版命令一致（enabled / has_token / endpoint）。
      return ok({ enabled: false, has_token: false, endpoint: '' }) as T;

    case 'set_heartbeat_config':
      return ok(null, '演示模式不会真的发送心跳') as T;

    // 演示环境没有真实 SMTP：返回「未配置」让界面展示真实降级态，
    // 并让测试发送明确失败，而不是假装成功。
    case 'get_smtp_config':
      return ok({ host: '', port: 587, from_addr: '', username: '', has_password: false, allow_insecure: false }) as T;

    case 'set_smtp_config':
      return ok(null, '演示模式不会真的保存 SMTP 配置') as T;

    // 审核场景：演示环境只暴露一个「平台政策」场景，结构与桌面版一致。
    case 'list_review_profiles':
      // 与桌面版一致地暴露四个场景；法律文书声明为纯人工。
      return ok([
        { id: 'platform-policy', title_key: 'review.profile.policy', criteria: '检查来源标注与材料一致性', use_ai: true, strictness: 'standard', strict_checks: [] },
        {
          id: 'legal-template', title_key: 'review.profile.legal',
          criteria: '逐项检查签署主体、空白项、日期格式、金额一致、见证人、处置冲突',
          use_ai: true, strictness: 'strict',
          strict_checks: [
            { id: 'signer', label: '签署主体已填写' },
            { id: 'blanks', label: '应填空白项已填完' },
            { id: 'date', label: '日期格式统一' },
            { id: 'amounts', label: '金额/比例一致' },
            { id: 'witness', label: '见证人栏齐备' },
            { id: 'conflict', label: '无处置冲突' },
          ],
        },
        { id: 'family-note', title_key: 'review.profile.familyNote', criteria: '检查是否可执行、是否误导', use_ai: true, strictness: 'standard', strict_checks: [] },
        { id: 'asset-classification', title_key: 'review.profile.asset', criteria: '检查类别与平台能力是否一致', use_ai: true, strictness: 'standard', strict_checks: [] },
        {
          id: 'heir-comprehension', title_key: 'review.profile.heirComprehension',
          criteria: '假设读者是刚失去亲人的普通家人：每条资产是否有找谁/带什么/做什么，无术语、无虚假时间承诺、无凭据明文',
          use_ai: true, strictness: 'strict',
          strict_checks: [
            { id: 'next-step', label: '每条资产都有可执行的下一步' },
            { id: 'jargon', label: '无开发者术语或内部占位符' },
            { id: 'ordering', label: '有依赖的步骤已标顺序' },
            { id: 'promise', label: '无无法兑现的时间承诺' },
            { id: 'secret', label: '无凭据或密码明文' },
            { id: 'disclaimer', label: '未暗示可替代公证/律师' },
          ],
        },
      ]) as T;

    case 'review_progress': {
      const key = String(a['profileId'] ?? 'platform-policy');
      const list = state.reviews?.[key] || [];
      return ok({
        total: list.length,
        approved: list.filter(x => x.status === 'approved').length,
        awaiting_human: list.filter(x => x.status === 'awaiting_human').length,
        pending: list.filter(x => x.status === 'pending').length,
      }) as T;
    }

    case 'review_payload_fields': {
      // 每个场景的送审字段不同：这是「AI 看到什么」的真实契约，
      // 全部返回同一份会让法律文书场景错误地宣称会发政策字段。
      // 值是 i18n 标识而非中文文案——否则英文界面会把中文原样打印出来。
      const byProfile: Record<string, string[]> = {
        'platform-policy': [
          'payload.field.platformName', 'payload.field.category', 'payload.field.inheritable',
          'payload.field.requiredDocs', 'payload.field.procedure', 'payload.field.difficulty',
          'payload.field.estimatedTime', 'payload.field.officialUrl',
          'payload.field.sourceNote', 'payload.field.notes',
        ],
        'legal-template': ['payload.field.legalDocType', 'payload.field.legalDescription', 'payload.field.legalBody'],
        'family-note': ['payload.field.noteFieldName', 'payload.field.noteContent'],
        'asset-classification': [
          'payload.field.platformName', 'payload.field.assetCategory',
          'payload.field.disposalMethod', 'payload.field.platformCapability',
        ],
        'heir-comprehension': [
          'payload.field.heirKitBody', 'payload.field.heirAllocation', 'payload.field.heirIntro',
        ],
      };
      return ok(byProfile[String(a['profileId'] ?? '')] ?? []) as T;
    }

    case 'list_review_items': {
      if (!state.reviews) state.reviews = {};
      const key = String(a['profileId'] ?? 'platform-policy');
      // 首次进入场景时按真实规模造演示条目，否则演示页永远是「共 0 条」，
      // 看起来像功能坏了。
      if (!state.reviews[key]) {
        state.reviews[key] = seedReviewItems(key, state);
        saveState(state);
      }
      return ok(state.reviews[key]) as T;
    }

    case 'run_ai_review': {
      // 演示环境没有 AI 端点 → 与桌面版一致地降级为纯人工，不伪造结论。
      if (!state.reviews) state.reviews = {};
      const key = String(a['profileId'] ?? 'platform-policy');
      const list = state.reviews[key] || [];
      // 纯人工场景：后端根本不会跑 AI（Legal 场景 use_ai=false）
      for (const item of list) {
        if (item.status !== 'pending') continue;
        item.ai_verdict = 'skipped';
        item.ai_note = '未配置 AI 端点，已转为纯人工审核';
        // 没有真正审核过，就不该有「审核时间」——否则界面会误导
        item.reviewed_at = null;
      }
      saveState(state);
      return ok(list, '未配置 AI 端点，已转为纯人工审核') as T;
    }

    case 'submit_human_review': {
      if (!state.reviews) state.reviews = {};
      const key = String(a['profileId'] ?? 'platform-policy');
      const itemKey = String(a['itemKey'] ?? '');
      const verdict = String(a['verdict'] ?? '');
      const list = state.reviews[key] || [];
      const target = list.find(x => x.key === itemKey);
      if (target) {
        target.status = verdict as typeof target.status;
        target.human_note = String(a['note'] ?? '');
        target.reviewed_at = now();
        saveState(state);
      }
      return ok(null, '审核结论已记录（预览模式）') as T;
    }

    case 'review_history': {
      const key = String(a['profileId'] ?? 'platform-policy');
      const itemKey = String(a['itemKey'] ?? '');
      const list = state.reviews?.[key] || [];
      const target = list.find(x => x.key === itemKey);
      // 没有任何审核动作时返回空数组，不伪造一条「已审核」记录。
      const hasAi = Boolean(target?.ai_verdict);
      const hasHuman = Boolean(target?.reviewed_at);
      if (!target || (!hasAi && !hasHuman)) return ok([]) as T;
      const rows = [];
      if (hasAi) {
        rows.push({
          profile_id: key, item_key: itemKey, fingerprint: 'demo',
          ai_verdict: target.ai_verdict, ai_note: target.ai_note,
          human_verdict: null, human_note: '', decided_by: 'ai',
          created_at: now(),
        });
      }
      if (hasHuman) {
        rows.push({
          profile_id: key, item_key: itemKey, fingerprint: 'demo',
          ai_verdict: target.ai_verdict ?? 'skipped', ai_note: target.ai_note,
          human_verdict: target.status === 'approved' ? 'approved'
            : target.status === 'rejected' ? 'rejected'
            : target.status === 'needs_edit' ? 'needs_edit' : null,
          human_note: target.human_note, decided_by: 'human',
          created_at: target.reviewed_at,
        });
      }
      return ok(rows) as T;
    }

    case 'get_heirs': {
      if (!state.heirs) {
        // 从旧档案字段派一名执行人，保持演示界面的报告结构完整
        state.heirs = state.profile?.heir_primary_name
          ? [{
              id: 1, name: state.profile.heir_primary_name, relation: state.profile.heir_primary_relation,
              phone: state.profile.heir_primary_phone, email: state.profile.heir_primary_email,
              share_note: '', is_executor: true, sort_order: 0,
            }]
          : [];
      }
      return ok(state.heirs) as T;
    }

    case 'set_heirs': {
      const list = (a['heirs'] as Array<Record<string, unknown>>) || [];
      // 与后端同一校验：恰好一位执行人
      if (list.length > 0 && list.filter(h => Boolean(h.is_executor)).length !== 1) {
        return fail<null>('必须恰好指定一位遗产执行人') as T;
      }
      state.heirs = list.map((h, i) => ({
        id: i + 1,
        name: String(h.name ?? ''),
        relation: String(h.relation ?? ''),
        phone: String(h.phone ?? ''),
        email: String(h.email ?? ''),
        share_note: String(h.share_note ?? ''),
        is_executor: Boolean(h.is_executor),
        sort_order: i,
      }));
      saveState(state);
      return ok(null, '继承人名单已保存（预览模式）') as T;
    }

    // 演示环境没有 AI 服务：配置恒为「关闭且无端点」，提问明确失败。
    case 'get_assistant_config':
      return ok({ enabled: false, endpoint: '', model: '', has_key: false }) as T;

    case 'set_assistant_config':
      return ok(null, '演示模式不会真的保存助手配置') as T;

    case 'preview_assistant_payload':
      return fail<null>('演示模式没有 AI 端点，无法预览出境内容') as T;

    case 'ask_assistant':
      return fail<null>('演示模式没有 AI 端点。本应用不内置任何 AI 服务') as T;

    case 'get_custody_hints':
      return ok({ backup_location: '', recovery_code_location: '' }) as T;

    // 演示站不实现真正的 Shamir 拆分：浏览器端拆分只会给人「份额可用」的错觉。
    case 'heir_recovery_status':
      return ok({ exists: false, stale: false }) as T;

    case 'list_ai_chat':
      return ok(state.aiChat ?? []) as T;

    case 'clear_ai_chat':
      state.aiChat = [];
      saveState(state);
      return ok(null) as T;

    case 'generate_heir_recovery_kit':
    case 'clear_heir_recovery':
    case 'unlock_with_recovery_shares':
    case 'run_recovery_drill':
      return fail<null>('在线演示不提供恢复包功能，请在桌面版使用') as T;

    case 'set_custody_hints': {
      const hints = (a['hints'] || {}) as { backup_location?: string; recovery_code_location?: string };
      state.custody = {
        backup_location: String(hints.backup_location ?? ''),
        recovery_code_location: String(hints.recovery_code_location ?? ''),
      };
      saveState(state);
      return ok(null, '保管位置已保存（预览模式）') as T;
    }

    case 'render_action_guide': {
      const r = (await mockInvoke('get_inheritance_report')) as { data?: { assets?: unknown[] } };
      const count = r.data?.assets?.length ?? 0;
      const loc = String(a['locale'] ?? 'zh-CN');
      const title = loc.startsWith('en') ? 'Digital Legacy Action Guide' : '数字遗产执行清单';
      return ok(
        `<!DOCTYPE html><html lang="${loc.startsWith('en') ? 'en' : 'zh-CN'}"><head><meta charset="UTF-8"><title>${title}</title>` +
        `<style>@media print{body{padding:0}}</style></head><body><div class="doc-title">${title}</div>` +
        `<p>演示清单：共 ${count} 条资产。</p></body></html>`,
        '执行清单已生成',
      ) as T;
    }

    case 'run_family_drill': {
      const assets = state.assets;
      const noHeir = assets.filter(x => x.disposal_method === 'inherit' && !(x.heir_name || '').trim()).length;
      const checks = [
        { key: 'heir', ok: noHeir === 0, count: noHeir },
        { key: 'policy', ok: true, count: 0 },
        { key: 'verified', ok: false, count: assets.length },
        { key: 'trigger', ok: state.dms.enabled, count: state.dms.enabled ? 0 : 1 },
      ];
      return ok({
        platforms: new Set(assets.map(x => x.platform_id)).size,
        assets: assets.length,
        assets_without_heir: noHeir,
        assets_without_policy: 0,
        unverified_platforms: assets.length,
        trigger_ready: state.dms.enabled,
        trigger_channel: state.dms.enabled ? 'heartbeat' : 'none',
        checks,
      }) as T;
    }

    case 'send_test_email':
      return fail<null>('演示模式没有 SMTP 服务器，无法发送测试邮件') as T;

    case 'reveal_exported_file':
      return ok(null, '演示模式不支持打开文件夹') as T;

    // 检查点：演示态存在 sessionStorage，仅用于让界面可走通。
    // 明确不做持久化——演示数据本来就不该被当成真实快照。
    case 'create_checkpoint': {
      const list = readMockCheckpoints();
      const meta = {
        id: `demo-${Date.now()}`,
        label: String(a['label'] ?? ''),
        created_at: new Date().toISOString().slice(0, 19).replace('T', ' '),
        size: JSON.stringify(state).length,
        auto: false,
      };
      list.unshift(meta);
      sessionStorage.setItem('digilegacy_mock_checkpoints', JSON.stringify(list.slice(0, 12)));
      return ok(meta, '演示模式：检查点仅存在于本次会话') as T;
    }

    case 'list_checkpoints':
      return ok(readMockCheckpoints()) as T;

    case 'delete_checkpoint': {
      const list = readMockCheckpoints().filter((c: { id: string }) => c.id !== a['id']);
      sessionStorage.setItem('digilegacy_mock_checkpoints', JSON.stringify(list));
      return ok(null, '已删除') as T;
    }

    case 'restore_checkpoint':
      return ok('演示模式不会真的回滚数据', '已回滚（模拟）') as T;

    case 'get_backup_status':
      return ok({ count: 0, latest_name: null, latest_size: null }) as T;

    case 'get_recovery_seal_exists':
      return ok(state.masterPassword !== null) as T;

    case 'start_recovery':
      return fail<null>('在线演示不提供恢复密钥验证，请在桌面版中使用') as T;

    case 'complete_recovery':
      return fail<null>('在线演示不提供主密码重设，请在桌面版中使用') as T;

    case 'get_heartbeat_status':
      return ok({ enabled: false, endpoint: '（预览模式未配置）', last_attempt: null, last_result: 'never' }) as T;

    case 'send_heartbeat_now':
      return ok({ enabled: false, endpoint: '（预览模式未配置）', last_attempt: new Date().toLocaleString(), last_result: 'never' }) as T;

    case 'set_autostart':
      return ok(null, a['enabled'] ? '已设置开机自启' : '已关闭开机自启') as T;

    case 'hide_main_window':
      return ok(null, '已隐藏到托盘') as T;

    default:
      throw new Error(`Web 预览模式未实现命令: ${cmd}`);
  }
}

/** 演示态检查点列表读取：集中在 helper 里，避免三个 case 各写一遍解析。
 *  只存在 sessionStorage——演示数据不该被当成真实快照持久化。 */
function readMockCheckpoints(): Array<{ id: string; label: string; created_at: string; size: number; auto: boolean }> {
  try {
    const raw = sessionStorage.getItem('digilegacy_mock_checkpoints');
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}
