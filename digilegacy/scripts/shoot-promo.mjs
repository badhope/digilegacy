// 宣传片拍摄：按分镜脚本逐场景录制，输出无声视频片段 + 字幕时间轴。
//
// 为什么不用「一路录到底」：宣传片需要精确的分镜时长与节奏，
// 一路录制无法在后期对齐字幕与转场。这里按场景切段，每段单独录，
// 再由 build-promo.mjs 用 ffmpeg 拼接、加字幕与背景音乐。
//
// 用法：node scripts/shoot-promo.mjs （需先 npm run dev）
// 输出：.tmp/promo/clip-XX.webm + .tmp/promo/manifest.json

import { chromium } from '@playwright/test';
import { mkdir, writeFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const OUT = path.join(ROOT, '.tmp', 'promo');
const RAW = path.join(OUT, 'raw');
const BASE = process.env.DEMO_BASE || 'http://localhost:1420';

// 画布尺寸：1920x1080 会让 webm 编码很慢；1280x800 足够且与现有截图一致。
const VIEWPORT = { width: 1280, height: 800 };

/** 分镜脚本。每段：进入某页面 → 执行脚本化操作 → 停留 → 截取。 */
const SCENES = [
  {
    id: '00-intro',
    title: '归零 DigiLegacy',
    subtitle: '规划你的数字遗产，给家人留下工具包',
    duration: 3800,
    async setup(page) {
      await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
      // 片头必须等仪表板真正渲染完，否则会拍到空白骨架（实测第一版就是空白）
      await page.locator('.dashboard').waitFor({ timeout: 15000 });
      await page.waitForTimeout(2500);
    },
  },
  {
    id: '01-dashboard',
    title: '一屏看清家底',
    subtitle: '资产总数、预估价值、规划完成度、四维检查',
    duration: 4200,
    async setup(page) {
      await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(900);
      await page.mouse.move(400, 420, { steps: 12 });
    },
  },
  {
    id: '02-discover',
    title: '资产发现：从自检开始',
    subtitle: '十项自检问卷，帮你想起早已忘记的账号',
    duration: 5000,
    async setup(page) {
      await page.goto(`${BASE}/discover`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(700);
      const boxes = page.locator('.checklist input[type="checkbox"]');
      if (await boxes.count()) {
        await boxes.nth(2).check();
        await page.waitForTimeout(600);
      }
      const next = page.getByRole('button', { name: /继续/ });
      if (await next.count()) {
        await next.click();
        await page.waitForTimeout(900);
      }
    },
  },
  {
    id: '03-categories',
    title: '按类型筛选平台',
    subtitle: '问卷答案自动预选大类，仍可自由增减',
    duration: 4200,
    async setup(page) {
      await page.goto(`${BASE}/discover`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(600);
      await page.getByRole('button', { name: /继续/ }).click();
      await page.waitForTimeout(1000);
    },
  },
  {
    id: '04-platforms',
    title: '152 个平台的政策库',
    subtitle: '勾选你拥有的平台，逐条登记',
    duration: 4600,
    async setup(page) {
      await page.goto(`${BASE}/discover`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(500);
      await page.getByRole('button', { name: /继续/ }).click();
      await page.waitForTimeout(500);
      const clear = page.getByRole('button', { name: /清空选择/ });
      if (await clear.isVisible()) await clear.click();
      await page.locator('.category-card').first().click();
      await page.getByRole('button', { name: /下一步：勾选平台/ }).click();
      await page.waitForTimeout(900);
      const cards = page.locator('.platform-card');
      if (await cards.count()) {
        await cards.nth(0).click();
        await page.waitForTimeout(400);
        await cards.nth(1).click();
        await page.waitForTimeout(800);
      }
    },
  },
  {
    id: '05-assets',
    title: '资产清单',
    subtitle: '搜索、筛选、四种排序，状态一眼可见',
    duration: 4200,
    async setup(page) {
      await page.goto(`${BASE}/assets`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1100);
      const search = page.locator('input[type="search"]').first();
      if (await search.count()) {
        await search.fill('支付');
        await page.waitForTimeout(900);
        await search.fill('');
        await page.waitForTimeout(400);
      }
    },
  },
  {
    id: '06-asset-detail',
    title: '加密保管凭据',
    subtitle: 'AES-256-GCM 字段级加密，仅本机可解',
    duration: 4600,
    async setup(page) {
      await page.goto(`${BASE}/assets/1`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1400);
      await page.mouse.wheel(0, 320);
      await page.waitForTimeout(900);
    },
  },
  {
    id: '07-profile',
    title: '档案与继承人',
    subtitle: '多位继承人、独立执行人、保管位置',
    duration: 5000,
    async setup(page) {
      await page.goto(`${BASE}/profile`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1200);
      await page.mouse.wheel(0, 260);
      await page.waitForTimeout(900);
    },
  },
  {
    id: '08-dms',
    title: '安心传承',
    subtitle: '三级确认链：本人 → 紧急联系人 → 执行人',
    duration: 4800,
    async setup(page) {
      await page.goto(`${BASE}/dms`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1400);
      await page.mouse.wheel(0, 300);
      await page.waitForTimeout(900);
    },
  },
  {
    id: '09-report',
    title: '继承报告',
    subtitle: '可打印文书，含平台办理指引与权威来源',
    duration: 5200,
    async setup(page) {
      await page.goto(`${BASE}/report`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1600);
      await page.mouse.wheel(0, 520);
      await page.waitForTimeout(700);
      await page.mouse.wheel(0, 520);
      await page.waitForTimeout(900);
    },
  },
  {
    id: '10-heir',
    title: '继承人向导',
    subtitle: '按平台查材料、流程、过户路径',
    duration: 4800,
    async setup(page) {
      await page.goto(`${BASE}/heir`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1200);
      const tabs = page.locator('[role="tab"]');
      if ((await tabs.count()) > 1) {
        await tabs.nth(1).click();
        await page.waitForTimeout(1000);
      }
    },
  },
  {
    id: '11-legal',
    title: '法律模板',
    subtitle: '六类文书，填上就能用',
    duration: 4600,
    async setup(page) {
      await page.goto(`${BASE}/legal`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1000);
      const preview = page.getByRole('button', { name: '预览' }).first();
      if (await preview.count()) {
        await preview.click();
        await page.waitForTimeout(1400);
      }
    },
  },
  {
    id: '12-review',
    title: 'AI 初审 + 人工终审',
    subtitle: '四个审核场景，AI 只提疑点，人工才放行',
    duration: 5400,
    async setup(page) {
      await page.goto(`${BASE}/review`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1200);
      const tabs = page.locator('.review [role="tab"]');
      if ((await tabs.count()) > 1) {
        await tabs.nth(1).click();
        await page.waitForTimeout(1400);
        const head = page.locator('.review-head-btn').first();
        if (await head.count()) {
          await head.click();
          await page.waitForTimeout(1200);
        }
      }
    },
  },
  {
    id: '13-privacy',
    title: '本地优先',
    subtitle: '0 服务器 · 0 遥测 · 数据不出本机',
    duration: 4400,
    async setup(page) {
      await page.goto(`${BASE}/profile`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1000);
      await page.mouse.wheel(0, 900);
      await page.waitForTimeout(1200);
    },
  },
  {
    id: '14-outro',
    title: '归零 DigiLegacy',
    subtitle: 'MIT 开源 · Windows 桌面 · 无需联网',
    duration: 4000,
    async setup(page) {
      await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
      await page.waitForTimeout(1400);
    },
  },
];

async function boot(page) {
  await page.addInitScript(() => {
    localStorage.setItem('digilegacy_onboarding_done', '1');
  });
  await page.goto(`${BASE}/dashboard`, { waitUntil: 'networkidle' });
  await page.waitForTimeout(900);
}

async function main() {
  await rm(RAW, { recursive: true, force: true });
  await mkdir(RAW, { recursive: true });

  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: VIEWPORT,
    recordVideo: { dir: RAW, size: VIEWPORT },
    deviceScaleFactor: 1,
  });
  const page = await context.newPage();
  await boot(page);

  // 同步卡：Playwright 的录像包含浏览器启动与首帧加载，导致录制成片比
  // 脚本时间轴提前/延后数秒（实测约 4 秒），字幕会整体错位。
  // 做法：开头先显示一张纯洋红色卡 1.2 秒，剪辑时用帧检测定位它的真实起点，
  // 再据此计算全片偏移。这比猜偏移可靠。
  await page.setContent(
    '<body style="margin:0;background:#FF00FF;width:100vw;height:100vh"></body>',
    { waitUntil: 'domcontentloaded' },
  );
  await page.waitForTimeout(1200);

  const manifest = [];
  const t0 = Date.now();

  for (const scene of SCENES) {
    const sceneStart = (Date.now() - t0) / 1000;
    const start = Date.now();
    try {
      await scene.setup(page);
    } catch (e) {
      console.warn(`场景 ${scene.id} 操作失败（继续录制）: ${String(e).slice(0, 100)}`);
    }
    // 保证每段至少达到目标时长
    const elapsed = Date.now() - start;
    const remain = Math.max(0, scene.duration - elapsed);
    if (remain > 0) await page.waitForTimeout(remain);

    const sceneEnd = (Date.now() - t0) / 1000;
    manifest.push({
      id: scene.id,
      title: scene.title,
      subtitle: scene.subtitle,
      duration: scene.duration,
      start: Number(sceneStart.toFixed(3)),
      end: Number(sceneEnd.toFixed(3)),
    });
    console.log(`shot ${scene.id}  ${scene.duration}ms  ${scene.title}`);
  }

  // 关闭上下文才会把视频落盘
  await context.close();
  await browser.close();

  await writeFile(
    path.join(OUT, 'manifest.json'),
    JSON.stringify({ viewport: VIEWPORT, scenes: manifest }, null, 2) + '\n',
    'utf8',
  );
  console.log(`\n完成 ${manifest.length} 段，原始视频在 ${RAW}`);
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
