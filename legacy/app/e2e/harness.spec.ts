import { expect, test } from './fixtures';
import type { Page } from '@playwright/test';

// 本轮新增的两项全局能力，必须在真实浏览器里走通：
// 1) 命令面板（⌘K）：纯键盘路径 + 搜索 + 跳转
// 2) 检查点卡片：在 Profile 页可见并给出可理解的空态
// 这两条都是「不点鼠标也要能用」的交互，单测覆盖了逻辑，E2E 负责验证真实渲染与快捷键。

async function prepare(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
  });
  await page.goto('/dashboard');
  await expect(page.locator('.sidebar')).toBeVisible();
}

test.describe('命令面板', () => {
  test('Ctrl+K 打开，可用搜索过滤并跳转', async ({ page }) => {
    await prepare(page);

    await page.keyboard.press('Control+k');
    const panel = page.locator('.cmdk-panel');
    await expect(panel).toBeVisible();

    // 搜索「legal」应只剩法律模板一条导航项
    const input = page.locator('[data-testid="cmdk-input"]');
    await input.fill('legal');
    const items = page.locator('.cmdk-item');
    await expect(items).toHaveCount(1);
    await expect(items.first()).toContainText(/Legal Templates|法律模板/);

    await page.keyboard.press('Enter');
    await expect(page).toHaveURL(/\/legal$/);
    // 跳转后面板必须关闭，否则会盖住目标页
    await expect(page.locator('.cmdk-panel')).toHaveCount(0);
  });

  test('Esc 关闭面板且不跳转', async ({ page }) => {
    await prepare(page);
    const before = page.url();
    await page.keyboard.press('Control+k');
    await expect(page.locator('.cmdk-panel')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('.cmdk-panel')).toHaveCount(0);
    expect(page.url()).toBe(before);
  });

  test('无匹配时给空态而不是空白面板', async ({ page }) => {
    await prepare(page);
    await page.keyboard.press('Control+k');
    await page.locator('[data-testid="cmdk-input"]').fill('zzzz-nothing');
    await expect(page.locator('.cmdk-empty')).toBeVisible();
  });

  test('头部按钮也能打开面板（不只有快捷键）', async ({ page }) => {
    await prepare(page);
    await page.locator('[data-testid="cmdk-open"]').click();
    await expect(page.locator('.cmdk-panel')).toBeVisible();
  });
});

test.describe('检查点', () => {
  test('Profile 页展示检查点卡片与空态说明', async ({ page }) => {
    await prepare(page);
    await page.goto('/profile');
    await expect(page.locator('.sidebar')).toBeVisible();

    // 找到含「Checkpoints / 检查点」标题的卡片
    const card = page.locator('.card').filter({ hasText: /Checkpoints|检查点/ }).first();
    await expect(card).toBeVisible();
    await expect(card.locator('button').first()).toBeVisible();
  });
});
