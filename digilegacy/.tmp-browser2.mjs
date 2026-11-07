import { chromium } from 'playwright';
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
page.on('pageerror', e => errors.push('PAGEERROR: ' + e.message));

await page.goto('http://127.0.0.1:8801/dashboard', { waitUntil: 'networkidle' });
await page.waitForTimeout(800);
const body = await page.textContent('body');
console.log('dashboard has asset count:', /1/.test(body));
console.log('completion shown:', body.includes('75') ? 'yes' : 'no');

await page.goto('http://127.0.0.1:8801/assets', { waitUntil: 'networkidle' });
await page.waitForTimeout(800);
const rows = await page.locator('.asset-row').count();
console.log('asset rows:', rows);
const rowText = rows ? await page.locator('.asset-row').first().textContent() : '';
console.log('row text snippet:', (rowText||'').trim().slice(0, 80));

// search debounce: type and verify filter applies after settle
const input = page.locator('input[type="search"]').first();
await input.fill('alipay');
await page.waitForTimeout(50);
const immediate = await page.locator('.asset-row').count();
await page.waitForTimeout(400);
const settled = await page.locator('.asset-row').count();
console.log('search immediate rows:', immediate, '| settled rows:', settled);

await page.screenshot({ path: '../.tmp/shot-assets-real.png' });
console.log('CONSOLE_ERRORS:', errors.length);
errors.slice(0,5).forEach(e => console.log(' !', e.slice(0,150)));
await browser.close();
