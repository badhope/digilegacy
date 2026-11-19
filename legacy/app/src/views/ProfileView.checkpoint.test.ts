import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';

// ProfileView 的检查点区块：新建 / 列表 / 回滚确认 / 删除。
// 回滚是破坏性操作，必须验证「用户取消时不执行」这条分支——
// 这是最容易写成「点了就回滚」的地方。
import ProfileView from './ProfileView.vue';

const invokeMock = vi.fn();

vi.mock('../api/invoke', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauriEnv: () => false,
}));

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
  RouterLink: { template: '<a><slot /></a>' },
}));

function okResponse(data: unknown, message = 'ok') {
  return { success: true, data, message };
}

function mountView() {
  return mount(ProfileView, {
    attachTo: document.body,
    global: { stubs: { RouterLink: { template: '<a><slot /></a>' } } },
  });
}

beforeEach(() => {
  setActivePinia(createPinia());
  document.body.innerHTML = '';
  invokeMock.mockReset();
  localStorage.clear();

  // 默认兜底：任何未显式 mock 的命令返回空成功，避免未处理 rejection 干扰断言
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === 'list_checkpoints') return Promise.resolve(okResponse([]));
    if (cmd === 'create_checkpoint') {
      return Promise.resolve(
        okResponse(
          { id: 'cp-1', label: '手动检查点', created_at: '2026-11-03 10:00:00', size: 2048, auto: false },
          '检查点已创建',
        ),
      );
    }
    if (cmd === 'delete_checkpoint') return Promise.resolve(okResponse(null, '已删除'));
    if (cmd === 'restore_checkpoint') return Promise.resolve(okResponse('已回滚', '已回滚到检查点'));
    if (cmd === 'get_security_status') {
      return Promise.resolve(okResponse({ has_master_password: false, unlocked: true }));
    }
    return Promise.resolve(okResponse(null));
  });
});

describe('检查点区块', () => {
  it('无检查点时展示引导空态', async () => {
    const w = mountView();
    await flushPromises();
    expect(w.text()).toContain('检查点');
    expect(w.text()).toContain('还没有检查点');
  });

  it('创建成功后刷新列表并显示新条目', async () => {
    const w = mountView();
    await flushPromises();

    let listed = false;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_checkpoints') {
        // 第一次空，创建后再查返回一条——模拟真实刷新
        return Promise.resolve(
          okResponse(
            listed
              ? [{ id: 'cp-1', label: '手动检查点', created_at: '2026-11-03 10:00:00', size: 2048, auto: false }]
              : [],
          ),
        );
      }
      if (cmd === 'create_checkpoint') {
        listed = true;
        return Promise.resolve(okResponse({ id: 'cp-1', label: '手动检查点', created_at: '', size: 2048, auto: false }, '检查点已创建'));
      }
      if (cmd === 'get_security_status') return Promise.resolve(okResponse({ has_master_password: false, unlocked: true }));
      return Promise.resolve(okResponse(null));
    });

    const createBtn = w.findAll('button').find((b) => b.text().includes('创建检查点'));
    expect(createBtn, '创建检查点按钮必须存在').toBeTruthy();
    await createBtn!.trigger('click');
    await flushPromises();

    expect(w.text()).toContain('手动检查点');
    expect(w.text()).toContain('2.0 KB');
  });

  it('回滚：用户取消确认时不得调用后端', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_checkpoints') {
        return Promise.resolve(
          okResponse([{ id: 'cp-1', label: '旧状态', created_at: '2026-11-03', size: 100, auto: false }]),
        );
      }
      if (cmd === 'get_security_status') return Promise.resolve(okResponse({ has_master_password: false, unlocked: true }));
      return Promise.resolve(okResponse(null));
    });

    const w2 = mountView();
    await flushPromises();

    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);
    const restoreBtn = w2.findAll('button').find((b) => b.text().includes('回滚到此'));
    expect(restoreBtn, '回滚按钮必须存在').toBeTruthy();
    await restoreBtn!.trigger('click');
    await flushPromises();

    expect(confirmSpy).toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalledWith('restore_checkpoint', expect.anything());
    confirmSpy.mockRestore();
  });

  it('回滚：用户确认后调用后端并带上检查点 id', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_checkpoints') {
        return Promise.resolve(
          okResponse([{ id: 'cp-9', label: '旧状态', created_at: '2026-11-03', size: 100, auto: false }]),
        );
      }
      if (cmd === 'get_security_status') return Promise.resolve(okResponse({ has_master_password: false, unlocked: true }));
      return Promise.resolve(okResponse(null));
    });

    const w = mountView();
    await flushPromises();

    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    const restoreBtn = w.findAll('button').find((b) => b.text().includes('回滚到此'));
    await restoreBtn!.trigger('click');
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('restore_checkpoint', expect.objectContaining({ id: 'cp-9' }));
    confirmSpy.mockRestore();
  });

  it('字节格式化：B / KB / MB 三档都正确', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_checkpoints') {
        return Promise.resolve(
          okResponse([
            { id: 'a', label: 'small', created_at: 'x', size: 512, auto: false },
            { id: 'b', label: 'mid', created_at: 'x', size: 2048, auto: false },
            { id: 'c', label: 'big', created_at: 'x', size: 3 * 1024 * 1024, auto: true },
          ]),
        );
      }
      if (cmd === 'get_security_status') return Promise.resolve(okResponse({ has_master_password: false, unlocked: true }));
      return Promise.resolve(okResponse(null));
    });

    const w = mountView();
    await flushPromises();

    expect(w.text()).toContain('512 B');
    expect(w.text()).toContain('2.0 KB');
    expect(w.text()).toContain('3.0 MB');
    // 自动检查点必须带标记，否则用户分不清哪些是系统打的
    expect(w.text()).toContain('自动');
  });
});
