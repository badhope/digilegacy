import { beforeEach, describe, expect, it, vi } from 'vitest';

// router.ts 决定三件事，任何一处写错都会让用户看到白屏：
//   1. 未知路径落到 404 兜底（旧链接/手输 URL）
//   2. 每个页面把标题写进 document.title
//   3. 演示构建用 hash 模式（静态托管没有 rewrite），桌面版用 history 模式
// 这里验证 1 和 2；模式选择是构建期常量，由 `npm run build:demo` 的产物检查覆盖。

describe('router', () => {
  beforeEach(() => {
    vi.resetModules();
    document.title = '';
    // jsdom 没有实现滚动 API；scrollBehavior 会调用 scrollTo，不打桩会污染 stderr
    vi.stubGlobal('scrollTo', vi.fn());
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
  });

  async function loadRouter() {
    const mod = await import('./router');
    return mod.default;
  }

  it('未知路径匹配到 NotFound 兜底路由', async () => {
    const router = await loadRouter();
    const resolved = router.resolve('/this/path/does/not/exist');
    expect(resolved.name).toBe('NotFound');
    expect(resolved.meta.titleKey).toBe('nav.notFound');
  });

  it('深层未知路径同样命中兜底（例如旧的 /assets/abc/def）', async () => {
    const router = await loadRouter();
    expect(router.resolve('/assets/1/extra/deep').name).toBe('NotFound');
  });

  it('导航后 document.title 变成「页面标题 · 归零 DigiLegacy」', async () => {
    const router = await loadRouter();
    await router.push('/assets').catch(() => {});
    expect(document.title).toContain('我的资产');
    expect(document.title).toContain('归零 DigiLegacy');
  });

  it('根路径重定向到仪表板', async () => {
    const router = await loadRouter();
    const resolved = router.resolve('/');
    const redirected = resolved.matched.flatMap(r => (r.redirect ? [r.redirect] : []));
    expect(redirected).toContain('/dashboard');
  });

  it('resolveTitleKey：缺失 titleKey 时回退应用名（空标题分支）', async () => {
    const { resolveTitleKey } = await import('./router');
    expect(resolveTitleKey({})).toBe('');
    expect(resolveTitleKey({ titleKey: 42 })).toBe('');
    expect(resolveTitleKey({ titleKey: 'nav.dashboard' })).toContain('仪表板');
  });

  it('scrollBehavior：无历史位置时回到顶部，有则恢复', async () => {
    const router = await loadRouter();
    const behavior = router.options.scrollBehavior;
    expect(typeof behavior).toBe('function');
    expect(behavior!({} as never, {} as never, null)).toEqual({ top: 0 });
    expect(behavior!({} as never, {} as never, { left: 0, top: 240 })).toEqual({ left: 0, top: 240 });
  });
});

describe('审核路由', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal('scrollTo', vi.fn());
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
  });

  it('/review 解析到审核页，且标题走 i18n 键', async () => {
    const router = await import('./router').then(m => m.default);
    const resolved = router.resolve('/review');
    expect(resolved.name).toBe('Review');
    expect(resolved.meta.titleKey).toBe('nav.review');
  });
});
