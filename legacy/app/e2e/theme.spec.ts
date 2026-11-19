import { expect, test } from './fixtures';

test.use({ viewport: { width: 1200, height: 800 } });

test('深色模式视觉验收', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
    localStorage.setItem('digilegacy_theme', 'dark');
  });
  await page.goto('/dashboard');
  await expect(page.locator('.sidebar')).toBeVisible();
  await page.waitForTimeout(600);

  const theme = await page.evaluate(() => document.documentElement.getAttribute('data-theme'));
  expect(theme, 'data-theme 必须为 dark').toBe('dark');

  await page.screenshot({ path: '../out/ui-audit/dark-dashboard.png' });

  await page.goto('/assets');
  await page.waitForTimeout(500);
  await page.screenshot({ path: '../out/ui-audit/dark-assets.png' });

  await page.goto('/dms');
  await page.waitForTimeout(500);
  await page.screenshot({ path: '../out/ui-audit/dark-dms.png' });
});

test('浅色模式 + 空态插画', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
    localStorage.setItem('digilegacy_theme', 'light');
    localStorage.removeItem('digilegacy-web-preview-v1');
  });
  await page.goto('/assets');
  await expect(page.locator('.sidebar')).toBeVisible();
  await page.waitForTimeout(700);
  await page.screenshot({ path: '../out/ui-audit/light-empty-assets.png' });
});

test('主题切换按钮可用', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
    localStorage.setItem('digilegacy_theme', 'light');
  });
  await page.goto('/dashboard');
  await expect(page.locator('.sidebar')).toBeVisible();

  const btn = page.getByRole('button', { name: /深色模式|浅色模式/ });
  await expect(btn).toBeVisible();
  await btn.click();
  await page.waitForTimeout(300);

  const after = await page.evaluate(() => document.documentElement.getAttribute('data-theme'));
  expect(after, '点击后主题必须切换').toBe('dark');
});
