// 全功能截图核对：覆盖所有页面、所有主要交互状态。
//
// 目的不是「拍好看的图」，而是**在拍摄宣传片之前把每个界面看一遍**，
// 找出需要优化的问题。因此每张图都记录：
//   - 页面是否渲染（主体非空）
//   - 是否有控制台错误
//   - 是否有横向溢出
//
// 用法：node scripts/full-audit-shots.mjs   （需先 npm run dev）
// 输出：docs/screenshots/audit/*.png + 控制台汇总

import { chromium } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const OUT = path.resolve(HERE, '..', '..', 'docs', 'screenshots', 'audit');
const BASE = process.env.DEMO_BASE || 'http://localhost:1420';
const VIEWPORT = { width: 1440, height: 900 };

/** 所有要核对的界面与状态。 */
const SHOTS = [
  { name: '01-dashboard', path: '/dashboard', wait: '.dashboard' },
  { name: '02-discover-step0', path: '/discover', wait: '.discover' },
  { name: '03-assets', path: '/assets', wait: '.assets' },
  { name: '04-asset-detail', path: '/assets/1', wait: '.asset-detail' },
  { name: '05-profile', path: '/profile', wait: '.profile' },
  { name: '06-dms', path: '/dms', wait: '.dms' },
  { name: '07-report', path: '/report', wait: '.report-page' },
  { name: '08-heir-guide', path: '/heir', wait: '.heir' },
  { name: '09-legal', path: '/legal', wait: '.legal' },
  { name: '10-review', path: '/review', wait: '.review' },
];

/** 需要点击进入的状态（每项一个额外截图）。 */
const INTERACTIONS = [
  {
    name: '11-discover-categories',
    path: '/discover',
    wait: '.discover',
    async run(page) {
      await page.getByRole('button', { name: /继续/ }).click();
      await page.waitForTimeout(400);
    },
  },
  {
    name: '12-discover-platforms',
    path: '/discover',
    wait: '.discover',
    async run(page) {
      await page.getByRole('button', { name: /继续/ }).click();
      await page.waitForTimeout(300);
      const clear = page.getByRole('button', { name: /清空选择/ });
      if (await clear.isVisible()) await clear.click();
      await page.locator('.category-card').first().click();
      await page.getByRole('button', { name: /下一步：勾选平台/ }).click();
      await page.waitForTimeout(400);
    },
  },
  {
    name: '13-legal-preview',
    path: '/legal',
    wait: '.legal',
    async run(page) {
      const btn = page.getByRole('button', { name: '预览' }).first();
      if (await btn.count()) {
        await btn.click();
        await page.waitForTimeout(400);
      }
    },
  },
  {
    name: '14-heir-monetization',
    path: '/heir',
    wait: '.heir',
    async run(page) {
      const tabs = page.locator('[role="tab"]');
      if ((await tabs.count()) > 1) {
        await tabs.nth(1).click();
        await page.waitForTimeout(500);
        const expand = page.getByRole('button', { name: '展开全部' });
        if (await expand.count()) {
          await expand.click();
          await page.waitForTimeout(400);
        }
      }
    },
  },
  {
    name: '15-review-strict',
    path: '/review',
    wait: '.review',
    async run(page) {
      const tabs = page.locator('.review [role="tab"]');
      if ((await tabs.count()) > 1) {
        await tabs.nth(1).click();
        await page.waitForTimeout(600);
        const head = page.locator('.review-head-btn').first();
        if (await head.count()) {
          await head.click();
          await page.waitForTimeout(300);
        }
      }
    },
  },
  {
    name: '16-report-dark',
    path: '/report',
    wait: '.report-page',
    async run(page) {
      const theme = page.locator('button[aria-label*="深色"], button[aria-label*="Dark"]');
      if (await theme.count()) {
        await theme.first().click();
        await page.waitForTimeout(400);
      }
    },
  },
  {
    name: '17-review-en',
    path: '/review',
    wait: '.review',
    async run(page) {
      const lang = page.locator('[data-testid="lang-switch"]');
      if (await lang.count()) {
        await lang.click();
        await page.waitForTimeout(500);
      }
    },
  },
];

async function boot(page) {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
  });
  await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
  await page.waitForTimeout(800);
}

/** 页面健康检查：主体文本、横向溢出、可见的原始 i18n 键名。 */
/** 等主体文本稳定：连续两次采样长度相同且非空，避免读到「元素已挂载、内容未渲染」的中间态。 */
async function waitForStableText(page, timeoutMs = 8000) {
  const deadline = Date.now() + timeoutMs;
  let last = -1;
  while (Date.now() < deadline) {
    const len = await page.evaluate(() => {
      const main = document.querySelector('.main-content') || document.body;
      return (main.textContent || '').replace(/\s+/g, ' ').trim().length;
    });
    if (len > 0 && len === last) return len;
    last = len;
    await page.waitForTimeout(250);
  }
  return last;
}

async function inspect(page) {
  return page.evaluate(() => {
    const main = document.querySelector('.main-content') || document.body;
    const text = (main.textContent || '').replace(/\s+/g, ' ').trim();
    const overflow = document.documentElement.scrollWidth - document.documentElement.clientWidth;
    // 原始键名特征：形如 review.xxx.yyy 且出现在可见文本里
    const rawKeys = (text.match(/\b[a-z][a-zA-Z]+(\.[a-zA-Z]+){2,}\b/g) || []).filter(
      k => !k.includes('http') && !k.includes('.com') && !k.includes('.cn'),
    );
    return {
      textLen: text.length,
      overflow,
      rawKeys: Array.from(new Set(rawKeys)).slice(0, 8),
      title: document.title,
    };
  });
}

async function main() {
  await mkdir(OUT, { recursive: true });
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: VIEWPORT });

  const errors = [];
  page.on('pageerror', e => errors.push(String(e).slice(0, 200)));
  page.on('console', m => {
    if (m.type() === 'error') errors.push(`console: ${m.text().slice(0, 200)}`);
  });

  const findings = [];

  for (const shot of SHOTS) {
    await boot(page);
    await page.goto(`${BASE}${shot.path}`, { waitUntil: 'networkidle' });
    try {
      await page.locator(shot.wait).first().waitFor({ timeout: 15000 });
    } catch {
      findings.push({ name: shot.name, issue: `未找到 ${shot.wait}` });
    }
    await page.waitForTimeout(400);
    await waitForStableText(page);
    const info = await inspect(page);
    const file = path.join(OUT, `${shot.name}.png`);
    await page.screenshot({ path: file });

    if (info.textLen < 80) findings.push({ name: shot.name, issue: `主体文本过短(${info.textLen})` });
    if (info.overflow > 2) findings.push({ name: shot.name, issue: `横向溢出 ${info.overflow}px` });
    if (info.rawKeys.length) findings.push({ name: shot.name, issue: `疑似裸 i18n 键: ${info.rawKeys.join(', ')}` });
    console.log(`captured ${shot.name}  text=${info.textLen} overflow=${info.overflow}`);
  }

  for (const item of INTERACTIONS) {
    await boot(page);
    await page.goto(`${BASE}${item.path}`, { waitUntil: 'networkidle' });
    try {
      await page.locator(item.wait).first().waitFor({ timeout: 15000 });
      await item.run(page);
    } catch (e) {
      findings.push({ name: item.name, issue: `交互失败: ${String(e).slice(0, 120)}` });
    }
    await waitForStableText(page);
    const info = await inspect(page);
    const file = path.join(OUT, `${item.name}.png`);
    await page.screenshot({ path: file });
    if (info.textLen < 80) findings.push({ name: item.name, issue: `主体文本过短(${info.textLen})` });
    if (info.overflow > 2) findings.push({ name: item.name, issue: `横向溢出 ${info.overflow}px` });
    if (info.rawKeys.length) findings.push({ name: item.name, issue: `疑似裸 i18n 键: ${info.rawKeys.join(', ')}` });
    console.log(`captured ${item.name}  text=${info.textLen} overflow=${info.overflow}`);
  }

  await browser.close();

  const report = {
    total: SHOTS.length + INTERACTIONS.length,
    findings,
    consoleErrors: Array.from(new Set(errors)),
  };
  await writeFile(path.join(OUT, 'audit-report.json'), JSON.stringify(report, null, 2) + '\n', 'utf8');

  console.log('\n=== 核对结果 ===');
  console.log(`截图 ${report.total} 张`);
  if (findings.length === 0) console.log('界面问题：无');
  else findings.forEach(f => console.log(`  ! ${f.name}: ${f.issue}`));
  if (report.consoleErrors.length === 0) console.log('控制台错误：无');
  else report.consoleErrors.forEach(e => console.log(`  ! ${e}`));

  process.exit(findings.length === 0 && report.consoleErrors.length === 0 ? 0 : 1);
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
