// 审核页实拍：四个场景各截一张，外加一个「AI 初审后」的状态。
// 用法：node scripts/review-shots.mjs   （需先 npm run dev）
import { chromium } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
// 截图属于交付物，放在 docs/ 下随仓库走，而不是临时目录。
const OUT = path.resolve(HERE, '..', '..', 'docs', 'screenshots');
const BASE = process.env.DEMO_BASE || 'http://localhost:1420';

const SCENES = [
  { file: 'review-01-policy', label: '平台政策', index: 0 },
  { file: 'review-02-legal', label: '法律文书', index: 1 },
  { file: 'review-03-family-note', label: '家人说明', index: 2 },
  { file: 'review-04-asset', label: '资产分类', index: 3 },
];

async function main() {
  await mkdir(OUT, { recursive: true });
  const browser = await chromium.launch();
  // 侧栏是 position:fixed，fullPage 截图只会把固定元素画在首屏高度内，
  // 下方留大片空白。做法：先用小视口渲染，量出内容高度，再把视口设成该高度
  // 后截非 fullPage 图——固定侧栏就会铺满整张图。
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });

  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
  });
  await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
  await page.waitForTimeout(600);

  await page.goto(`${BASE}/review`, { waitUntil: 'networkidle' });
  await page.locator('.review').waitFor();
  await page.waitForTimeout(800);

  const shots = [];
  for (const scene of SCENES) {
    const tabs = page.locator('.review [role="tab"]');
    const count = await tabs.count();
    if (scene.index >= count) {
      console.warn(`跳过 ${scene.label}: 只有 ${count} 个场景`);
      continue;
    }
    await page.setViewportSize({ width: 1280, height: 900 });
    await tabs.nth(scene.index).click();
    await page.waitForTimeout(700);
    // 展开第一条，让操作区与留痕在截图里可见
    const head = page.locator('.review-head-btn').first();
    if (await head.count()) {
      await head.click();
      await page.waitForTimeout(300);
      const historyBtn = page.getByRole('button', { name: '审核历史' }).first();
      if (await historyBtn.count()) {
        await historyBtn.click();
        await page.waitForTimeout(300);
      }
    }
    // 先量内容高度，再按需扩展视口（上限 1600，避免超长页面）
    const contentHeight = await page.evaluate(() => {
      const el = document.querySelector('.review');
      return el ? Math.ceil(el.getBoundingClientRect().height + 120) : 900;
    });
    await page.setViewportSize({ width: 1280, height: Math.min(Math.max(contentHeight, 900), 1600) });
    await page.waitForTimeout(200);
    const file = path.join(OUT, `${scene.file}.png`);
    await page.screenshot({ path: file });
    shots.push(file);
    console.log('captured', file);
  }

  // 跑一次 AI 初审（演示环境无端点 → 应显示纯人工降级），再截一张
  const runBtn = page.getByRole('button', { name: /运行 AI 初审/ }).first();
  if (await runBtn.count()) {
    await runBtn.click();
    await page.waitForTimeout(1200);
    const h5 = await page.evaluate(() => {
      const el = document.querySelector('.review');
      return el ? Math.ceil(el.getBoundingClientRect().height + 120) : 900;
    });
    await page.setViewportSize({ width: 1280, height: Math.min(Math.max(h5, 900), 1600) });
    await page.waitForTimeout(200);
    const file = path.join(OUT, 'review-05-after-ai.png');
    await page.screenshot({ path: file });
    shots.push(file);
    console.log('captured', file);
  }

  // 控制台错误检查：截图好看不代表页面没报错
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  await page.reload({ waitUntil: 'networkidle' });
  await page.waitForTimeout(800);
  if (errors.length) {
    console.error('CONSOLE ERRORS:', errors);
    process.exitCode = 1;
  } else {
    console.log('no console errors');
  }

  await browser.close();
  console.log('total', shots.length);
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
