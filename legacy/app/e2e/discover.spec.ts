import { expect, test } from './fixtures';
import type { Page } from '@playwright/test';

// 资产发现是产品的核心链路：自检 → 选类别 → 勾平台 → 填详情 → 保存 → 进资产列表。
// 这条路径此前 0% 覆盖（DiscoverView 121 条语句无任何测试）。

/** 跳过首次引导并确保停在未加密的明文预览态（mock 后端）。 */
async function prepare(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
  });
  await page.goto('/dashboard');
  await expect(page.locator('.sidebar')).toBeVisible();
}

test.describe('资产发现主链路', () => {
  test('从自检到保存资产，全流程可走通', async ({ page }) => {
    await prepare(page);
    await page.goto('/discover');

    // 步骤 0：自检问卷
    await expect(page.getByText('资产概览自检')).toBeVisible();
    // 问卷默认预勾选 7 项（checklistItems 的 checkedInit）。
    // 不写死绝对数，改为验证「勾选/取消会改变计数」这一行为本身。
    const footer = page.locator('.checklist-footer');
    await expect(footer).toContainText('已勾选');
    const readCount = async () => {
      const t = await footer.textContent();
      return Number(/(\d+)\s*\/\s*10/.exec(t ?? '')?.[1] ?? -1);
    };
    const initial = await readCount();
    expect(initial).toBeGreaterThanOrEqual(0);

    await page.locator('#check-2').check(); // 加密货币钱包，默认未勾
    await expect.poll(readCount).toBe(initial + 1);

    await page.locator('#check-0').uncheck();
    await expect.poll(readCount).toBe(initial);
    await page.getByRole('button', { name: /继续/ }).click();

    // 步骤 1：选资产类别
    // 问卷会按勾选项预选大类（这是新行为），本用例验证的是手动选择路径，
    // 先清空预选，保证「选一个类别」的断言稳定。
    await expect(page.getByText('你拥有哪些类型的数字资产')).toBeVisible();
    const clear = page.getByRole('button', { name: /清空选择/ });
    if (await clear.isVisible()) await clear.click();
    await page.locator('.category-card').first().click();
    await expect(page.locator('.category-card.selected')).toHaveCount(1);
    await page.getByRole('button', { name: /下一步：勾选平台/ }).click();

    // 步骤 2：勾选平台（分组默认展开 —— CollapseGroup defaultOpen 回归）
    await expect(page.getByText('勾选你拥有账号的每个平台')).toBeVisible();
    const platformCards = page.locator('.platform-card');
    await expect(platformCards.first()).toBeVisible();
    const visibleCount = await platformCards.count();
    expect(visibleCount, '平台分组必须默认展开，卡片应可见').toBeGreaterThan(0);

    await platformCards.first().click();
    await expect(page.locator('.platform-card.selected')).toHaveCount(1);
    await page.getByRole('button', { name: /下一步：设定资产详情/ }).click();

    // 步骤 3：填写资产详情
    await expect(page.getByText(/资产详情|填写/).first()).toBeVisible();
    await page.getByRole('button', { name: /保存全部资产/ }).click();

    // 步骤 4：完成页
    await expect(page.getByText(/完成|已保存|成功/).first()).toBeVisible({ timeout: 15_000 });
  });

  test('未选类别时「下一步」按钮禁用', async ({ page }) => {
    await prepare(page);
    await page.goto('/discover');
    await page.getByRole('button', { name: /继续/ }).click();

    // 问卷预选会让按钮一开始就可用；先清空以验证「真的没选时禁用」
    const clear = page.getByRole('button', { name: /清空选择/ });
    if (await clear.isVisible()) await clear.click();

    const next = page.getByRole('button', { name: /下一步：勾选平台/ });
    await expect(next).toBeDisabled();

    await page.locator('.category-card').first().click();
    await expect(next).toBeEnabled();
  });

  test('未勾平台时无法进入详情步骤', async ({ page }) => {
    await prepare(page);
    await page.goto('/discover');
    await page.getByRole('button', { name: /继续/ }).click();
    const clear = page.getByRole('button', { name: /清空选择/ });
    if (await clear.isVisible()) await clear.click();
    await page.locator('.category-card').first().click();
    await page.getByRole('button', { name: /下一步：勾选平台/ }).click();

    await expect(page.getByRole('button', { name: /下一步：设定资产详情/ })).toBeDisabled();
  });
});

test.describe('资产列表联动', () => {
  test('保存后的资产出现在「我的资产」里', async ({ page }) => {
    await prepare(page);
    await page.goto('/discover');
    await page.getByRole('button', { name: /继续/ }).click();
    await page.locator('.category-card').first().click();
    await page.getByRole('button', { name: /下一步：勾选平台/ }).click();
    await page.locator('.platform-card').first().click();
    await page.getByRole('button', { name: /下一步：设定资产详情/ }).click();
    await page.getByRole('button', { name: /保存全部资产/ }).click();
    await expect(page.getByText(/完成|已保存|成功/).first()).toBeVisible({ timeout: 15_000 });

    await page.goto('/assets');
    await expect(page.locator('.asset-row').first()).toBeVisible();
    await expect(page.getByText(/共 \d+ 项/)).toBeVisible();
  });
});
