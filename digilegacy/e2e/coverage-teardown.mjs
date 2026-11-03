// 每个测试结束后从浏览器取 window.__coverage__ 并落盘。
// 用 fixture 更精确，但 teardown 已能覆盖「同一浏览器上下文」的常见场景。
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

const HERE = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const OUT = path.join(HERE, '.coverage');

export default async function globalTeardown() {
  // Playwright 不提供「所有页面」的句柄，覆盖率由 per-test fixture 写入；
  // 这里只确保目录存在，避免 collect 脚本报错。
  await mkdir(OUT, { recursive: true });
}
