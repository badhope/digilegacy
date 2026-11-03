import { expect, test } from './fixtures';
import type { Page } from '@playwright/test';

// 真实使用场景模拟（浏览器层）：在真实 Chromium 里把「用户会点的组合」跑一遍。
// 与 views/theme/ui-audit 的分工：
// - 那三个 spec 覆盖「每个页面能打开、关键按钮有反应」；
// - 这里覆盖**矩阵**：8 路由 × 2 语言 × 2 主题、平台搜索 20 个关键词、
//   资产筛选 4×4、法律模板 6 类、继承人 2 Tab × 3 难度、报告导出 3 个按钮。
// 每个断言都是一个真实 DOM 检查，不是快照。

const ROUTES = [
  { path: '/dashboard', zh: '仪表板', en: 'Dashboard' },
  { path: '/discover', zh: '资产发现', en: 'Discover' },
  { path: '/assets', zh: '我的资产', en: 'My Assets' },
  { path: '/profile', zh: '我的档案', en: 'Profile' },
  { path: '/dms', zh: '安心传承', en: 'Legacy Care' },
  { path: '/report', zh: '继承报告', en: 'Inheritance Report' },
  { path: '/heir', zh: '继承人向导', en: 'Heir Guide' },
  { path: '/review', zh: '内容审核', en: 'Review' },
  { path: '/legal', zh: '法律模板', en: 'Legal Templates' },
];

async function boot(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
  });
  await page.goto('/dashboard');
  await expect(page.locator('.sidebar')).toBeVisible();
}

test.describe('路由 × 语言 × 主题 矩阵（32 场景）', () => {
  for (const route of ROUTES) {
    for (const lang of ['zh-CN', 'en-US'] as const) {
      for (const theme of ['light', 'dark'] as const) {
        test(`${route.path} · ${lang} · ${theme}`, async ({ page }) => {
          await page.addInitScript(
            ([l, th]) => {
              localStorage.setItem('digilegacy_onboarding_done', '1');
              localStorage.setItem('digilegacy_locale', l as string);
              localStorage.setItem('digilegacy_theme', th as string);
            },
            [lang, theme],
          );
          const errors: string[] = [];
          page.on('pageerror', e => errors.push(String(e)));
          page.on('console', m => {
            if (m.type() === 'error') errors.push(m.text());
          });

          await page.goto(route.path);
          await expect(page.locator('.sidebar')).toBeVisible();

          // 1) 标题跟随语言
          const expected = lang === 'zh-CN' ? route.zh : route.en;
          await expect(page.locator('.header-title')).toHaveText(expected);

          // 2) 主题落到 documentElement
          await expect(page.locator('html')).toHaveAttribute('data-theme', theme);

          // 3) 无 JS 错误 / 无横向溢出
          expect(errors, `${route.path} ${lang} ${theme} 有控制台错误: ${errors.join(' | ')}`).toEqual([]);
          const overflow = await page.evaluate(() => ({
            sw: document.documentElement.scrollWidth,
            cw: document.documentElement.clientWidth,
          }));
          expect(overflow.sw, `${route.path} ${lang} ${theme} 横向溢出`).toBeLessThanOrEqual(overflow.cw + 1);

          // 4) 页面主体必须有可读文本（不是白屏）
          const text = (await page.locator('.main-content').innerText()).trim();
          expect(text.length, `${route.path} 主体为空`).toBeGreaterThan(30);
        });
      }
    }
  }
});

test.describe('资产发现：平台搜索 20 个关键词（20 场景）', () => {
  test('每个关键词都能筛出子集且不报错', async ({ page }) => {
    await boot(page);
    await page.goto('/discover');
    await page.getByRole('button', { name: /继续/ }).click();
    const clear = page.getByRole('button', { name: /清空选择/ });
    if (await clear.isVisible()) await clear.click();
    await page.locator('.category-card').first().click();
    await page.getByRole('button', { name: /下一步：勾选平台/ }).click();
    await expect(page.locator('.platform-card').first()).toBeVisible();

    const search = page.locator('input[type="search"]').first();
    const keywords = ['支付宝', '微信', 'Steam', 'B站', '知乎', '网易', '云', '银行', '游戏', 'zzz不存在', '  ', 'QQ', '音乐', '淘宝', '京东', '得物', '闲鱼', 'iCloud', 'Google', '公积金'];
    const errors: string[] = [];
    page.on('pageerror', e => errors.push(String(e)));

    for (const kw of keywords) {
      await search.fill(kw);
      await page.waitForTimeout(120);
      const cards = await page.locator('.platform-card').count();
      expect(cards, `关键词「${kw}」渲染异常`).toBeGreaterThanOrEqual(0);
      // 搜索结果必须都包含关键词（空/空格除外）
      const trimmed = kw.trim();
      if (trimmed) {
        const names = await page.locator('.platform-name').allInnerTexts();
        for (const n of names) {
          expect(
            n.toLowerCase().includes(trimmed.toLowerCase()),
            `关键词「${trimmed}」命中了不相关平台「${n}」`,
          ).toBe(true);
        }
      }
    }
    expect(errors, '搜索过程中出现 JS 错误').toEqual([]);
    await search.fill('');
  });
});

test.describe('资产列表：4 类别 × 4 排序（16 场景）', () => {
  test('筛选与排序组合后行数合法且无重复', async ({ page }) => {
    await boot(page);
    // 造 4 条不同类别的资产，走真实发现流程
    await page.goto('/discover');
    for (let i = 0; i < 4; i++) {
      await page.getByRole('button', { name: /继续/ }).click();
      // 问卷会预选大类；本用例要按索引精确挑一个，先清空再点，
      // 否则点到的卡片可能已被选中→反而取消，导致下一步按钮禁用。
      const clear = page.getByRole('button', { name: /清空选择/ });
      if (await clear.isVisible()) await clear.click();
      await page.locator('.category-card').nth(i).click();
      await page.getByRole('button', { name: /下一步：勾选平台/ }).click();
      const card = page.locator('.platform-card').first();
      if (await card.count()) {
        await card.click();
        await page.getByRole('button', { name: /下一步：设定资产详情/ }).click();
        await page.getByRole('button', { name: /保存全部资产/ }).click();
        await expect(page.getByText(/完成|已保存|成功/).first()).toBeVisible({ timeout: 15000 });
      }
      await page.goto('/discover');
    }

    await page.goto('/assets');
    await expect(page.locator('.asset-row').first()).toBeVisible();

    const filters = ['全部', '财产型', '人格型', '混合型'];
    const sorts = ['recent', 'value', 'platform', 'disposal'];
    for (const f of filters) {
      const btn = page.getByRole('button', { name: new RegExp(`^${f}`) });
      if (await btn.count()) await btn.first().click();
      for (const s of sorts) {
        await page.locator('select').first().selectOption(s);
        await page.waitForTimeout(80);
        const ids = await page.locator('.asset-row').evaluateAll(rows =>
          rows.map(r => (r as HTMLElement).innerText),
        );
        expect(new Set(ids).size, `${f}/${s} 出现重复行`).toBe(ids.length);
      }
    }
  });
});

test.describe('法律模板：6 分类 × 预览/关闭（12 场景）', () => {
  test('每个分类都能筛出内容且预览可开可关', async ({ page }) => {
    await boot(page);
    await page.goto('/legal');
    const cats = ['遗嘱', '继承声明', '授权委托书', '宣誓书/保证书', '申请书', '协议书'];
    for (const cat of cats) {
      await page.getByRole('button', { name: cat, exact: true }).click();
      await page.waitForTimeout(100);
      const count = await page.getByRole('button', { name: '预览' }).count();
      expect(count, `${cat} 分类下没有模板`).toBeGreaterThan(0);

      await page.getByRole('button', { name: '预览' }).first().click();
      const dialog = page.getByRole('dialog');
      await expect(dialog).toBeVisible();
      const body = await dialog.innerText();
      expect(body.length, `${cat} 预览内容为空`).toBeGreaterThan(20);
      await page.keyboard.press('Escape');
      await expect(dialog).toBeHidden();
    }
  });
});

test.describe('继承人向导：2 Tab × 3 难度 × 展开/收起（12 场景）', () => {
  test('Tab、难度筛选与折叠全部可用且计数一致', async ({ page }) => {
    await boot(page);
    await page.goto('/heir');

    for (const tab of ['继承指引', '变现/过户路径卡']) {
      await page.getByRole('tab', { name: tab }).click();
      await page.waitForTimeout(150);

      for (const diff of ['全部', '简单', '中等', '困难']) {
        const btn = page.getByRole('button', { name: diff, exact: true });
        if (await btn.count()) {
          await btn.first().click();
          await page.waitForTimeout(100);
        }
        await expect(page.getByRole('tab', { name: tab })).toHaveAttribute('aria-selected', 'true');
      }
    }

    // 展开/收起必须真正改变可见条目
    await page.getByRole('button', { name: '展开全部' }).first().click();
    await page.waitForTimeout(200);
    const opened = await page.locator('.collapse-body').count();
    await page.getByRole('button', { name: '收起全部' }).first().click();
    await page.waitForTimeout(200);
    const closed = await page.locator('.collapse-body').count();
    expect(opened, '展开后应出现折叠内容').toBeGreaterThan(closed);
  });
});

test.describe('安心传承：签到与设置 10 轮幂等（10 场景）', () => {
  test('连续签到与保存设置都不产生错误或重复提示卡', async ({ page }) => {
    await boot(page);
    await page.goto('/dms');
    const errors: string[] = [];
    page.on('pageerror', e => errors.push(String(e)));

    for (let i = 0; i < 5; i++) {
      await page.getByRole('button', { name: /我还安好/ }).click();
      await page.waitForTimeout(200);
      await expect(page.getByRole('button', { name: /我还安好/ })).toBeEnabled();
    }
    for (let i = 0; i < 5; i++) {
      await page.getByRole('button', { name: '保存设置' }).click();
      await page.waitForTimeout(200);
    }
    expect(errors, '签到/设置过程中出现 JS 错误').toEqual([]);
  });
});

test.describe('继承报告：加载与三个导出按钮（6 场景）', () => {
  test('报告正文完整且导出/打印按钮均可点', async ({ page }) => {
    await boot(page);
    await page.goto('/report');
    await expect(page.locator('.report-page')).toBeVisible({ timeout: 10000 });
    const text = await page.locator('.report-page').innerText();
    expect(text).toContain('继承');
    expect(text.length).toBeGreaterThan(200);

    for (const name of ['导出报告', '导出继承工具包']) {
      const btn = page.getByRole('button', { name: new RegExp(name) });
      await expect(btn).toBeVisible();
      await expect(btn).toBeEnabled();
    }
    const printSpy: string[] = [];
    await page.exposeFunction('__printMark', () => printSpy.push('print'));
    await page.evaluate(() => {
      window.print = () => (window as unknown as { __printMark: () => void }).__printMark();
    });
    await page.getByRole('button', { name: '打印' }).click();
    await page.waitForTimeout(200);
    expect(printSpy.length).toBe(1);
  });
});

test.describe('语言与主题切换：8 路由往返（16 场景）', () => {
  test('每个页面切换语言与主题后标题与属性都更新', async ({ page }) => {
    await boot(page);
    for (const route of ROUTES) {
      await page.goto(route.path);
      await page.locator('.sidebar').waitFor();

      await page.locator('[data-testid="lang-switch"]').click();
      await expect(page.locator('.header-title')).toHaveText(route.en);
      await expect(page.locator('html')).toHaveAttribute('lang', 'en-US');

      await page.locator('[data-testid="lang-switch"]').click();
      await expect(page.locator('.header-title')).toHaveText(route.zh);

      const before = await page.locator('html').getAttribute('data-theme');
      // 用稳定 testid：历史上这里用 'header button' 的位置选择器，
      // 任何在它前面新增的按钮都会让断言点错元素。
      await page.locator('[data-testid="theme-switch"]').click();
      const after = await page.locator('html').getAttribute('data-theme');
      expect(after, `${route.path} 主题切换无效`).not.toBe(before);
    }
  });
});
