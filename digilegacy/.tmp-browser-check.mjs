import { chromium } from 'playwright';

const browser = await chromium.launch({ channel: 'msedge', headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [];
page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
page.on('pageerror', e => errors.push('PAGEERROR: ' + e.message));

const base = 'http://127.0.0.1:8797';
for (const path of ['/dashboard', '/assets', '/discover', '/report', '/profile']) {
  await page.goto(base + path, { waitUntil: 'networkidle' });
  await page.waitForTimeout(600);
  const title = await page.title();
  const bodyText = (await page.textContent('body')) || '';
  const rows = await page.locator('.asset-row').count();
  console.log(`${path} | title=${title} | textLen=${bodyText.trim().length} | rows=${rows}`);
  await page.screenshot({ path: `../.tmp/shot-${path.replace(/\//g,'_')}.png`, fullPage: false });
}
console.log('CONSOLE_ERRORS:', errors.length);
errors.slice(0, 10).forEach(e => console.log('  !', e.slice(0, 200)));
await browser.close();
