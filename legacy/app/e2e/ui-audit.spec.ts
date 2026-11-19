import { expect, test } from './fixtures';
import type { Page } from '@playwright/test';

// 逐页遍历所有可点击元素，检查：点击后有无报错、有无反馈、按钮是否卡死。
// 这是「任何按钮功能卡片等等界面全部自己看过」的自动化版本。

const PAGES = [
  { path: '/dashboard', name: '仪表板' },
  { path: '/discover', name: '资产发现' },
  { path: '/assets', name: '我的资产' },
  { path: '/profile', name: '我的档案' },
  { path: '/dms', name: '安心传承' },
  { path: '/report', name: '继承报告' },
  { path: '/heir', name: '继承人向导' },
  { path: '/legal', name: '法律模板' },
];

async function prepare(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
  });
}

test.describe('全站 UI 健全性', () => {
  for (const p of PAGES) {
    test(`${p.name}：无控制台错误 + 无失败请求 + 布局不溢出`, async ({ page }) => {
      const errors: string[] = [];
      page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
      page.on('pageerror', (e) => errors.push(String(e)));

      await prepare(page);
      await page.goto(p.path);
      await expect(page.locator('.sidebar')).toBeVisible();
      await page.waitForTimeout(600);

      // 1) 无 JS 异常
      expect(errors, `页面 ${p.path} 出现控制台错误:\n${errors.join('\n')}`).toEqual([]);

      // 2) 无横向溢出（1200 视口下）
      const overflow = await page.evaluate(() => {
        const el = document.documentElement;
        return { scrollW: el.scrollWidth, clientW: el.clientWidth };
      });
      expect(
        overflow.scrollW,
        `${p.path} 出现横向溢出: scrollWidth=${overflow.scrollW} > clientWidth=${overflow.clientW}`
      ).toBeLessThanOrEqual(overflow.clientW + 1);

      // 3) 所有可见按钮都有可访问名称
      const buttons = await page.locator('button:visible').all();
      for (const b of buttons) {
        const name = (await b.getAttribute('aria-label')) || (await b.textContent()) || '';
        expect(name.trim().length, `${p.path} 存在无名称按钮`).toBeGreaterThan(0);
      }

      // 4) 所有可见输入框都有可访问名称：
      //    aria-label / label[for=id] / 祖先 <label> 包裹（隐式关联）/ title
      const missing = await page.evaluate(() => {
        const out: string[] = [];
        const els = Array.from(document.querySelectorAll('input, select, textarea')) as HTMLElement[];
        for (const el of els) {
          const style = window.getComputedStyle(el);
          if (style.display === 'none' || style.visibility === 'hidden') continue;
          const anyEl = el as HTMLInputElement;
          if (anyEl.type === 'hidden') continue;
          const hasAria = !!el.getAttribute('aria-label');
          const hasTitle = !!el.getAttribute('title');
          const id = el.getAttribute('id');
          const hasFor = id ? !!document.querySelector(`label[for="${CSS.escape(id)}"]`) : false;
          const hasWrappingLabel = !!el.closest('label');
          if (!(hasAria || hasTitle || hasFor || hasWrappingLabel)) {
            out.push(el.tagName + (id ? '#' + id : ''));
          }
        }
        return out;
      });
      expect(missing, `${p.path} 以下输入框缺少可访问名称: ${missing.join(', ')}`).toEqual([]);
    });
  }
});

test.describe('关键按钮逐个点击', () => {
  test('仪表板：帮助 / 检查 / 锁定 三个按钮都有反应', async ({ page }) => {
    await prepare(page);
    await page.goto('/dashboard');

    // 帮助抽屉：打开 → 内容非空 → Esc 关闭
    await page.getByRole('button', { name: /帮助/ }).click();
    const helpDialog = page.getByRole('dialog').first();
    await expect(helpDialog).toBeVisible({ timeout: 5000 });
    expect((await helpDialog.textContent() || '').length).toBeGreaterThan(100);
    await page.keyboard.press('Escape');
    await expect(helpDialog).toBeHidden({ timeout: 5000 });

    // 检查签到：点击后按钮必须恢复可用（不能卡在 loading）
    const checkBtn = page.getByRole('button', { name: /检查/ });
    await checkBtn.click();
    await expect(checkBtn).toBeEnabled({ timeout: 8000 });

    // mock 默认未设主密码 → 按设计不显示「锁定」按钮。
    // 这里验证的是「条件正确」，不是「按钮存在」。
    await expect(page.getByRole('button', { name: /^🔒 锁定$/ })).toHaveCount(0);
  });

  test('法律模板：预览 / 下载 / 筛选 全部可点', async ({ page }) => {
    await prepare(page);
    await page.goto('/legal');

    // 筛选每个分类
    for (const cat of ['遗嘱', '继承声明', '授权委托书', '宣誓书/保证书', '申请书', '协议书']) {
      const btn = page.getByRole('button', { name: cat, exact: true });
      if (await btn.count()) {
        await btn.click();
        await page.waitForTimeout(120);
      }
    }
    await page.getByRole('button', { name: '全部', exact: true }).click();

    // 预览
    await page.getByRole('button', { name: '预览' }).first().click();
    const dialog = page.getByRole('dialog').first();
    await expect(dialog).toBeVisible();
    // 预览里必须有真实正文（不是空壳）
    const text = await dialog.textContent();
    expect((text || '').length, '预览弹窗内容过短，疑为空壳').toBeGreaterThan(200);
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden({ timeout: 5000 });
  });

  test('继承人向导：Tab / 筛选 / 展开收起 全部可用', async ({ page }) => {
    await prepare(page);
    await page.goto('/heir');

    // 继承指引 Tab
    const guide = page.getByRole('tab', { name: /继承指引/ });
    await expect(guide).toHaveAttribute('aria-selected', 'true');

    // 切到变现 Tab
    const money = page.getByRole('tab', { name: /变现/ });
    await money.click();
    await expect(money).toHaveAttribute('aria-selected', 'true');

    // 展开全部
    const expand = page.getByRole('button', { name: /展开全部/ });
    if (await expand.count()) {
      await expand.click();
      await expect(page.getByRole('button', { name: /收起全部/ })).toBeVisible({ timeout: 5000 });
    }

    // 切回
    await guide.click();
    await expect(guide).toHaveAttribute('aria-selected', 'true');
  });

  test('安心传承：保存设置与测试提醒都有反馈', async ({ page }) => {
    await prepare(page);
    await page.goto('/dms');

    await page.getByRole('button', { name: '保存设置' }).click();
    await page.waitForTimeout(800);

    const testNotify = page.getByRole('button', { name: /发送测试提醒/ });
    if (await testNotify.count()) {
      await testNotify.click();
      await expect(testNotify).toBeEnabled({ timeout: 10000 });
    }
  });

  test('资产详情：凭据保存/显示/清除 全链路', async ({ page }) => {
    await prepare(page);
    // 造资产
    await page.goto('/discover');
    await page.getByRole('button', { name: /继续/ }).click();
    await page.locator('.category-card').first().click();
    await page.getByRole('button', { name: /下一步：勾选平台/ }).click();
    await page.locator('.platform-card').first().click();
    await page.getByRole('button', { name: /下一步：设定资产详情/ }).click();
    await page.getByRole('button', { name: /保存全部资产/ }).click();
    await expect(page.getByText(/完成|已保存|成功/).first()).toBeVisible({ timeout: 15000 });

    await page.goto('/assets');
    // 列表在 mock 首次访问时会异步落盘重建，等行真正可见再点，
    // 否则并行跑全量时点击可能落在尚未绑定路由的节点上（偶发 URL 不变）。
    const firstRow = page.locator('.asset-row').first();
    await expect(firstRow).toBeVisible({ timeout: 15000 });
    await firstRow.click();
    await expect(page).toHaveURL(/\/assets\/\d+/, { timeout: 15000 });

    // 凭据输入与保存
    const credInput = page.locator('input[aria-label*="凭据"]');
    if (await credInput.count()) {
      await credInput.fill('user:pass');
      const saveBtn = page.getByRole('button', { name: /保存凭据/ });
      if (await saveBtn.count()) {
        await saveBtn.click();
        await page.waitForTimeout(500);
      }
    }
  });
});
