// 展示物料生成器：截图 + 演示视频。
//
// 用法（需要先启动站点）：
//   1) npm run dev                       # 开发预览，http://localhost:1420
//   2) node scripts/demo-media.mjs       # 截图到 docs/screenshots，视频到 docs/media
//
// 可选：DEMO_BASE=http://localhost:4173/digilegacy/demo/ 针对静态演示构建出图。
//
// 为什么用 Playwright 而不是裸 CDP：视频录制需要浏览器上下文级 API，
// 且 Playwright 已是本项目的 E2E 依赖，不再引入新工具。

import { chromium } from '@playwright/test';
import { mkdir, readFile, unlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');
const SHOTS = path.join(ROOT, 'docs', 'screenshots');
const MEDIA = path.join(ROOT, 'docs', 'media');
const BASE = process.env.DEMO_BASE || 'http://localhost:1420';

// 每张图都要出两种语言：官网与 README 都是双语的，英文页贴中文截图
// 等于对外承诺和实际展示不符。`title` 是该语言下的页面标题，
// 用来断言「截到的确实是这个页面」，而不是一张 404 或上一页的残留。
const PAGES = [
  { file: '01-dashboard', path: '/dashboard', zh: '仪表板', en: 'Dashboard' },
  { file: '02-discover', path: '/discover', zh: '资产发现', en: 'Discover' },
  { file: '03-assets', path: '/assets', zh: '我的资产', en: 'My Assets' },
  { file: '04-asset-detail', path: '/assets/1', zh: '资产详情', en: 'Asset Detail' },
  { file: '05-profile', path: '/profile', zh: '我的档案', en: 'Profile' },
  { file: '06-dms', path: '/dms', zh: '安心传承', en: 'Legacy Care' },
  { file: '07-report', path: '/report', zh: '继承报告', en: 'Inheritance Report' },
  { file: '08-heir-guide', path: '/heir', zh: '继承人向导', en: 'Heir Guide' },
  { file: '09-legal', path: '/legal', zh: '法律模板', en: 'Legal Templates' },
];

/** 出图语言：默认两种都出；SHOT_LOCALES=en 只出英文，便于快速迭代。 */
const LOCALES = (process.env.SHOT_LOCALES || 'zh,en').split(',').map((x) => x.trim()).filter(Boolean);
const LOCALE_TAG = { zh: 'zh-CN', en: 'en-US' };
/** 英文截图落到 docs/screenshots/en/，中文留在原目录，路径稳定不破坏已有引用。 */
const shotDir = (locale) => (locale === 'en' ? path.join(SHOTS, 'en') : SHOTS);

const VIEWPORT = { width: 1200, height: 800 };

async function seed(page, locale) {
  // 必须先让应用建出演示状态，再改写它——首次访问时 localStorage 还是空的，
  // 直接 addInitScript 拿不到 state，截图就会显示「未启用加密」。
  await page.goto(`${BASE}/dashboard`, { waitUntil: 'domcontentloaded' });
  await page.waitForTimeout(900);
  // 注意：传给 page.evaluate 的参数必须显式透传，闭包变量在浏览器上下文里不可见
  await page.evaluate((loc) => {
    const KEY = 'digilegacy-web-preview-v1';
    const raw = localStorage.getItem(KEY);
    if (raw) {
      try {
        const state = JSON.parse(raw);
        state.masterPassword = 'demo-capture-pass';
        localStorage.setItem(KEY, JSON.stringify(state));
      } catch {
        /* 损坏的演示状态会在应用侧重建 */
      }
    }
    sessionStorage.setItem('digilegacy-web-preview-unlocked', '1');
    localStorage.setItem('digilegacy_onboarding_done', '1');
    localStorage.setItem('digilegacy_locale', loc);
  }, locale);
  await page.reload({ waitUntil: 'networkidle' });
}

async function captureScreenshots(browser) {
  const allErrors = [];
  for (const locale of LOCALES) {
    const dir = shotDir(locale);
    await mkdir(dir, { recursive: true });
    const context = await browser.newContext({ viewport: VIEWPORT, deviceScaleFactor: 2 });
    const page = await context.newPage();

    page.on('console', (m) => {
      if (m.type() === 'error') allErrors.push(`[${locale}] ${m.text()}`);
    });
    page.on('pageerror', (e) => allErrors.push(`[${locale}] PAGEERROR ${e.message}`));

    await seed(page, LOCALE_TAG[locale]);
    for (const item of PAGES) {
      const expected = item[locale];
      await page.goto(`${BASE}${item.path}`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(700);
      const heading = (await page.locator('.header-title').textContent().catch(() => '')) ?? '';
      if (!heading.trim()) throw new Error(`${item.file}[${locale}]: 页面标题为空，未渲染完成`);
      // 标题不一致通常意味着路由没生效或截到了 404/上一页，直接失败比交付错图好
      if (!heading.includes(expected)) {
        throw new Error(`${item.file}[${locale}]: 期望标题含「${expected}」，实际「${heading.trim()}」`);
      }
      await page.screenshot({ path: path.join(dir, `${item.file}.png`) });
      console.log(`captured ${locale}/${item.file}.png  ← ${item.path} 「${heading.trim()}」`);
    }

    // 深色模式截图（展示品牌在暗色下的表现）
    await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
    await page.evaluate(() => {
      localStorage.setItem('digilegacy_theme', 'dark');
      document.documentElement.setAttribute('data-theme', 'dark');
      document.documentElement.style.colorScheme = 'dark';
    });
    await page.waitForTimeout(700);
    await page.screenshot({ path: path.join(dir, '10-dashboard-dark.png') });
    console.log(`captured ${locale}/10-dashboard-dark.png`);

    await context.close();
  }
  if (allErrors.length > 0) {
    console.warn(`注意：截图过程中出现 ${allErrors.length} 条控制台错误：`);
    for (const e of allErrors.slice(0, 5)) console.warn('  ' + e);
  }
}

async function recordWalkthrough(browser) {
  await mkdir(MEDIA, { recursive: true });
  const context = await browser.newContext({
    viewport: VIEWPORT,
    recordVideo: { dir: MEDIA, size: VIEWPORT },
  });
  const page = await context.newPage();
  await seed(page, LOCALE_TAG.zh);

  const pause = (ms) => page.waitForTimeout(ms);

  await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
  await pause(1600);

  // 资产发现：选类别 → 勾平台 → 填详情 → 保存
  await page.goto(`${BASE}/discover`, { waitUntil: 'networkidle' });
  await pause(900);
  await page.getByRole('button', { name: /继续/ }).click();
  await pause(700);
  await page.locator('.category-card').first().click();
  await pause(500);
  await page.getByRole('button', { name: /下一步：勾选平台/ }).click();
  await pause(900);
  await page.locator('.platform-card').first().click();
  await pause(500);
  await page.getByRole('button', { name: /下一步：设定资产详情/ }).click();
  await pause(900);
  const heir = page.locator('input[id^="heir-name-"]').first();
  if (await heir.count()) await heir.fill('家人');
  await pause(600);
  await page.getByRole('button', { name: /保存全部资产/ }).click();
  await pause(1400);

  // 资产列表 → 详情
  await page.goto(`${BASE}/assets`, { waitUntil: 'networkidle' });
  await pause(1000);
  await page.locator('.asset-row').first().click();
  await pause(1400);
  await page.locator('#detail-disposal').selectOption('inherit');
  await pause(500);
  await page.getByRole('button', { name: '保存修改' }).click();
  await pause(1200);

  // 安心传承
  await page.goto(`${BASE}/dms`, { waitUntil: 'networkidle' });
  await pause(1000);
  await page.getByRole('button', { name: /我还安好/ }).click();
  await pause(1200);

  // 继承报告
  await page.goto(`${BASE}/report`, { waitUntil: 'networkidle' });
  await pause(1800);
  await page.mouse.wheel(0, 700);
  await pause(900);
  await page.mouse.wheel(0, 700);
  await pause(1200);

  // 继承人向导 + 法律模板
  await page.goto(`${BASE}/heir`, { waitUntil: 'networkidle' });
  await pause(1200);
  await page.getByRole('tab', { name: /变现/ }).click();
  await pause(1400);
  await page.goto(`${BASE}/legal`, { waitUntil: 'networkidle' });
  await pause(900);
  await page.getByRole('button', { name: '预览' }).first().click();
  await pause(1500);
  await page.keyboard.press('Escape');
  await pause(600);

  // 回到仪表板收尾
  await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
  await pause(1600);

  const video = page.video();
  await context.close();
  if (!video) throw new Error('未生成视频');
  const src = await video.path();
  const target = path.join(MEDIA, 'demo-walkthrough.webm');
  const buf = await readFile(src);
  await writeFile(target, buf);
  // Playwright 会额外留下 page@<hash>.webm，删除避免仓库里出现重复文件
  if (path.resolve(src) !== path.resolve(target)) {
    await unlink(src).catch(() => {});
  }
  console.log(`recorded ${path.relative(ROOT, target)}`);
}

const browser = await chromium.launch();
try {
  await captureScreenshots(browser);
  if (process.env.SKIP_VIDEO !== '1') await recordWalkthrough(browser);
} finally {
  await browser.close();
}
