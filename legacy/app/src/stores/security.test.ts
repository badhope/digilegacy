import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import type { Mock } from 'vitest';

// security store 是解锁/设密/锁定的唯一状态源：任何一处把 unlocked 判错，
// 用户要么被锁在门外，要么在锁定态看到明文。这里锁定该状态机。
vi.mock('../api/invoke', () => ({
  invoke: vi.fn(),
  isTauriEnv: () => false,
}));

import { invoke } from '../api/invoke';
import { useSecurityStore } from './security';
import { silenceExpectedErrors } from '../test-utils/silence';

silenceExpectedErrors();

const mockedInvoke = invoke as unknown as Mock;

beforeEach(() => {
  setActivePinia(createPinia());
  mockedInvoke.mockReset();
});

describe('security store 状态机', () => {
  it('未设主密码时默认视为已解锁', async () => {
    const s = useSecurityStore();
    expect(s.unlocked).toBe(true);

    mockedInvoke.mockResolvedValueOnce({
      success: true,
      message: '',
      data: { has_master_password: false, unlocked: false },
    });
    await s.refresh();

    expect(s.hasMasterPassword).toBe(false);
    expect(s.unlocked).toBe(true);
  });

  it('已设主密码且锁定 → unlocked=false', async () => {
    const s = useSecurityStore();
    mockedInvoke.mockResolvedValueOnce({
      success: true,
      message: '',
      data: { has_master_password: true, unlocked: false },
    });
    await s.refresh();

    expect(s.hasMasterPassword).toBe(true);
    expect(s.unlocked).toBe(false);
  });

  it('refresh 失败不改变既有状态（不误判为已解锁）', async () => {
    const s = useSecurityStore();
    s.hasMasterPassword = true;
    s.unlocked = false;

    mockedInvoke.mockRejectedValueOnce(new Error('IPC 断开'));
    await s.refresh();

    expect(s.hasMasterPassword).toBe(true);
    expect(s.unlocked).toBe(false);
  });

  it('setMasterPassword 成功时透传一次性恢复密钥', async () => {
    const s = useSecurityStore();
    mockedInvoke.mockResolvedValueOnce({
      success: true,
      message: '加密已启用',
      data: {
        status: { has_master_password: true, unlocked: true },
        recovery_code: 'AAAA-BBBB-CCCC-DDDD-EEEE-FFFF',
      },
    });

    const res = await s.setMasterPassword('master-pass-1');

    expect(res.ok).toBe(true);
    expect(res.recoveryCode).toBe('AAAA-BBBB-CCCC-DDDD-EEEE-FFFF');
    expect(s.hasMasterPassword).toBe(true);
    expect(s.unlocked).toBe(true);
  });

  it('setMasterPassword 失败时不得残留 hasMasterPassword', async () => {
    const s = useSecurityStore();
    mockedInvoke.mockResolvedValueOnce({ success: false, message: '主密码至少需要 8 个字符', data: null });

    const res = await s.setMasterPassword('short');

    expect(res.ok).toBe(false);
    expect(res.recoveryCode).toBe('');
    expect(s.hasMasterPassword).toBe(false);
  });

  it('unlockWith 错误密码返回后端消息且保持锁定', async () => {
    const s = useSecurityStore();
    s.unlocked = false;
    mockedInvoke.mockResolvedValueOnce({ success: false, message: '主密码错误', data: null });

    const msg = await s.unlockWith('wrong');

    expect(msg).toBe('主密码错误');
    expect(s.unlocked).toBe(false);
  });

  it('unlockWith 正确密码解除锁定', async () => {
    const s = useSecurityStore();
    s.unlocked = false;
    mockedInvoke.mockResolvedValueOnce({
      success: true,
      message: '',
      data: { has_master_password: true, unlocked: true },
    });

    const msg = await s.unlockWith('right');

    expect(msg).toBe('');
    expect(s.unlocked).toBe(true);
  });

  it('lockNow 即使后端调用失败也必须在本地置为锁定', async () => {
    const s = useSecurityStore();
    s.unlocked = true;
    mockedInvoke.mockRejectedValueOnce(new Error('IPC 断开'));

    await s.lockNow();

    expect(s.unlocked).toBe(false);
  });

  it('busy 在操作期间为 true、结束后复位', async () => {
    const s = useSecurityStore();
    // 用悬挂的 promise 卡住调用，观察「执行中」的 busy，而不是只看最终 false
    // （只看最终值的话，删掉 busy=true 用例仍会通过 —— 变异实测存活）。
    let release!: (v: unknown) => void;
    mockedInvoke.mockImplementationOnce(async () => new Promise(r => { release = r; }));

    const pending = s.setMasterPassword('master-pass-1');
    await Promise.resolve();
    expect(s.busy).toBe(true);

    release({ success: false, message: 'x', data: null });
    await pending;
    expect(s.busy).toBe(false);
  });

  it('unlockWith 执行期间 busy 为 true，失败后复位', async () => {
    const s = useSecurityStore();
    let release!: (v: unknown) => void;
    mockedInvoke.mockImplementationOnce(async () => new Promise(r => { release = r; }));

    const pending = s.unlockWith('x');
    await Promise.resolve();
    expect(s.busy).toBe(true);

    release({ success: false, message: '主密码错误', data: null });
    await pending;
    expect(s.busy).toBe(false);
  });
});
