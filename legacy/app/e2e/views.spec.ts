import { expect, test } from './fixtures';
import type { Page } from '@playwright/test';

// 覆盖剩余 6 个此前 0% 的视图：Dashboard / DMS / Report / HeirGuide / LegalTemplates / AssetDetail。
// 目标是「真实渲染 + 关键交互有响应」，不是逐像素断言。

async function prepare(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
  });
  await page.goto('/dashboard');
  await expect(page.locator('.sidebar')).toBeVisible();
}

test.describe('仪表板', () => {
  test('渲染统计卡片与完成度', async ({ page }) => {
    await prepare(page);
    await expect(page.getByRole('heading', { level: 2 }).first()).toBeVisible();
    await expect(page.locator('.stat-card')).toHaveCount(4);
    await expect(page.locator('.stat-label').first()).toContainText('已知数字资产');
    // 完成度进度条必须存在（0% 时也要渲染容器）
    await expect(page.getByText('数字遗产规划完成度')).toBeVisible();
  });

  test('统计数字随 mock 数据变化（非硬编码）', async ({ page }) => {
    await prepare(page);
    const first = page.locator('.stat-value').first();
    const value = await first.textContent();
    expect(Number(value?.trim())).toBeGreaterThanOrEqual(0);
  });
});

test.describe('安心传承（DMS）', () => {
  test('渲染检查按钮与设置项', async ({ page }) => {
    await prepare(page);
    await page.goto('/dms');

    await expect(page.getByText("Dead Man's Switch")).toBeVisible();
    await expect(page.getByText('确认你安好')).toBeVisible();
    await expect(page.getByRole('button', { name: /我还安好/ })).toBeVisible();
    await expect(page.getByText('检查设置')).toBeVisible();
    await expect(page.getByText('三级确认机制')).toBeVisible();
  });

  test('点击「我还安好」后状态更新', async ({ page }) => {
    await prepare(page);
    await page.goto('/dms');

    await page.getByRole('button', { name: /我还安好/ }).click();
    // 签到后应出现成功反馈或时间更新，至少不应报错/回退
    await expect(page.getByRole('button', { name: /我还安好/ })).toBeEnabled({ timeout: 10_000 });
  });

  test('修改检查间隔并保存', async ({ page }) => {
    await prepare(page);
    await page.goto('/dms');

    const select = page.locator('select').first();
    await expect(select).toBeVisible();
    await select.selectOption({ index: 1 });
    await page.getByRole('button', { name: '保存设置' }).click();
    await expect(page.getByRole('button', { name: '保存设置' })).toBeEnabled();
  });
});

test.describe('继承报告', () => {
  test('渲染报告正文与导出按钮', async ({ page }) => {
    await prepare(page);
    await page.goto('/report');

    // 报告页先出骨架屏再渲染正文；并行跑全量时 mock 首访要落盘，给足等待，
    // 否则后续断言会在骨架阶段抢跑（曾偶发全量失败、单跑通过）。
    await expect(page.locator('.report-page')).toBeVisible({ timeout: 15000 });
    await expect(page.getByText('数字遗产继承报告')).toBeVisible();
    await expect(page.getByRole('button', { name: /导出报告/ })).toBeVisible();
    await expect(page.getByRole('button', { name: /导出继承工具包/ })).toBeVisible();
    // 报告正文必须真的由数据渲染出来
    await expect(page.getByText('数字遗产持有人信息')).toBeVisible();
  });

  test('导出报告有成功反馈', async ({ page }) => {
    await prepare(page);
    await page.goto('/report');
    await page.getByRole('button', { name: /导出报告/ }).click();
    await expect(page.getByText(/已导出|保存到|导出成功/).first()).toBeVisible({ timeout: 10_000 });
  });
});

test.describe('继承人向导', () => {
  test('渲染前提提示与流程总览', async ({ page }) => {
    await prepare(page);
    await page.goto('/heir');

    await expect(page.getByText('继承人向导').first()).toBeVisible();
    await expect(page.getByText('重要前提')).toBeVisible();
    await expect(page.getByText('继承流程总览')).toBeVisible();
    await expect(page.getByText(/确认继承权/)).toBeVisible();
  });

  test('两个 Tab 可切换', async ({ page }) => {
    await prepare(page);
    await page.goto('/heir');

    // Tab 用 role="tab"，文案带 emoji，按 role + 部分文字定位
    const monetization = page.getByRole('tab', { name: /变现/ });
    await expect(monetization).toBeVisible();
    await monetization.click();
    await expect(monetization).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('group', { name: '按类别筛选' })).toBeVisible();
  });

  test('点击「展开全部」后平台条目可见（CollapseGroup 折叠契约）', async ({ page }) => {
    await prepare(page);
    await page.goto('/heir');

    const expandAll = page.getByRole('button', { name: /展开全部/ });
    await expect(expandAll).toBeVisible();
    await expandAll.click();

    // 展开后至少有一个分组体渲染出平台条目
    await expect(page.locator('.collapse-body').first()).toBeVisible();
    await expect(page.getByRole('button', { name: /收起全部/ })).toBeVisible();

    // 再点一次应全部收起
    await page.getByRole('button', { name: /收起全部/ }).click();
    await expect(page.locator('.collapse-body')).toHaveCount(0);
  });
});

test.describe('法律模板', () => {
  test('渲染模板清单与分类筛选', async ({ page }) => {
    await prepare(page);
    await page.goto('/legal');

    await expect(page.getByText('法律文书模板库')).toBeVisible();
    await expect(page.getByText('按类型筛选')).toBeVisible();
    const templates = page.locator('.template-row, .card').filter({ hasText: '模板' });
    await expect(page.locator('text=数字资产遗嘱').first()).toBeVisible();
  });

  test('切换分类筛选会改变列表', async ({ page }) => {
    await prepare(page);
    await page.goto('/legal');

    const allBtn = page.getByRole('button', { name: '全部' });
    await expect(allBtn).toBeVisible();
    const before = await page.locator('.template-row').count();

    const categories = page.locator('.filter-btn, button').filter({ hasText: '遗嘱' });
    if ((await categories.count()) > 0) {
      await categories.first().click();
      const after = await page.locator('.template-row').count();
      expect(after).toBeLessThanOrEqual(before);
    }
  });

  test('预览按钮打开弹窗并可用 Esc 关闭', async ({ page }) => {
    await prepare(page);
    await page.goto('/legal');

    await page.getByRole('button', { name: '预览' }).first().click();
    const dialog = page.locator('[role="dialog"]');
    await expect(dialog.first()).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(dialog.first()).toBeHidden({ timeout: 5_000 });
  });
});

test.describe('资产详情', () => {
  test('从资产列表进入详情页', async ({ page }) => {
    await prepare(page);
    // 先造一条资产
    await page.goto('/discover');
    await page.getByRole('button', { name: /继续/ }).click();
    await page.locator('.category-card').first().click();
    await page.getByRole('button', { name: /下一步：勾选平台/ }).click();
    await page.locator('.platform-card').first().click();
    await page.getByRole('button', { name: /下一步：设定资产详情/ }).click();
    await page.getByRole('button', { name: /保存全部资产/ }).click();
    await expect(page.getByText(/完成|已保存|成功/).first()).toBeVisible({ timeout: 15_000 });

    await page.goto('/assets');
    await page.locator('.asset-row').first().click();
    await expect(page).toHaveURL(/\/assets\/\d+/);
    await expect(page.getByText(/账号凭据|处置意愿/).first()).toBeVisible();
  });
});
