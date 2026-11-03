import { describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { createRouter, createMemoryHistory } from 'vue-router';

import NotFoundView from './NotFoundView.vue';

// 兜底路由必须真的可达：深层链接/旧书签落到空白页会让用户以为应用坏了。
describe('NotFoundView', () => {
  it('渲染 404 文案与返回入口', async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/dashboard', component: { template: '<div>dash</div>' } },
        { path: '/:pathMatch(.*)*', component: NotFoundView },
      ],
    });
    await router.push('/no-such-page');
    await router.isReady();

    const w = mount(NotFoundView, { global: { plugins: [router] } });
    expect(w.text()).toContain('这个页面不存在');
    expect(w.text()).toContain('回到仪表板');
    expect(w.find('a[href="/dashboard"]').exists()).toBe(true);
  });
});

describe('NotFoundView 返回行为', () => {
  async function mountNotFound(extraRoutes: { path: string; component: unknown }[] = []) {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/dashboard', component: { template: '<div>dash</div>' } },
        ...extraRoutes,
        { path: '/:pathMatch(.*)*', component: NotFoundView },
      ],
    } as Parameters<typeof createRouter>[0]);
    await router.push('/no-such-page');
    await router.isReady();
    const w = mount(NotFoundView, { global: { plugins: [router] } });
    return { w, router };
  }

  it('无历史记录时「返回上一页」跳转仪表板', async () => {
    const { w, router } = await mountNotFound();
    const back = w.findAll('button').find(b => b.text().includes('返回上一页'))!;
    await back.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.path).toBe('/dashboard');
  });

  it('有历史记录时「返回上一页」调用 router.back', async () => {
    const { w, router } = await mountNotFound();
    const spy = vi.spyOn(router, 'back').mockImplementation(() => Promise.resolve());
    Object.defineProperty(window.history, 'length', { value: 3, configurable: true });
    const btn = w.findAll('button').find(b => b.text().includes('返回上一页'))!;
    await btn.trigger('click');
    await flushPromises();
    expect(spy).toHaveBeenCalled();
    spy.mockRestore();
  });
});
