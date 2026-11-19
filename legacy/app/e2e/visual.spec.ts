import { expect, test } from './fixtures';

// 视觉验收：截图 Toast、确认框、新品牌图标在真实渲染下的样子。
test.use({ viewport: { width: 1200, height: 800 } });

test('Toast 与确认框视觉验收', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('digilegacy_onboarding_done', '1'));
  await page.goto('/dms');
  await expect(page.locator('.sidebar')).toBeVisible();

  // 触发成功 Toast（保存设置）
  await page.getByRole('button', { name: '保存设置' }).click();
  await expect(page.locator('.toast')).toBeVisible({ timeout: 5000 });
  // 等淡入动画（0.24s）完全结束后再截，否则会截到半透明的中间帧
  await page.waitForTimeout(500);
  await page.screenshot({ path: '../out/ui-audit/toast-success.png' });

  // 造一条资产后进入详情页，触发「删除此资产」的危险确认框
  await page.goto('/discover');
  await page.getByRole('button', { name: /继续/ }).click();
  await page.locator('.category-card').first().click();
  await page.getByRole('button', { name: /下一步：勾选平台/ }).click();
  await page.locator('.platform-card').first().click();
  await page.getByRole('button', { name: /下一步：设定资产详情/ }).click();
  await page.getByRole('button', { name: /保存全部资产/ }).click();
  await expect(page.getByText(/完成|已保存|成功/).first()).toBeVisible({ timeout: 15000 });

  await page.goto('/assets');
  await page.locator('.asset-row').first().click();
  await page.getByRole('button', { name: '删除此资产' }).click();
  await expect(page.locator('.confirm-card')).toBeVisible({ timeout: 5000 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: '../out/ui-audit/confirm-danger.png' });
});

test('品牌图标与侧栏视觉验收', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('digilegacy_onboarding_done', '1'));
  await page.goto('/dashboard');
  await expect(page.locator('.sidebar')).toBeVisible();
  await page.locator('.sidebar-header').screenshot({ path: '../out/ui-audit/brand-logo.png' });
});

test('骨架屏视觉验收', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('digilegacy_onboarding_done', '1'));
  // 拦截 IPC 延迟加载，以捕捉骨架屏
  await page.route('**/*', r => r.continue());
  await page.goto('/assets');
  await page.screenshot({ path: '../out/ui-audit/assets-page.png', fullPage: false });
});
