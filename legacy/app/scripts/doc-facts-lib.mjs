// 文档事实推导：单一实现，check / sync 两个脚本共用。
// 之所以抽出来：两边各写一份时，E2E 计数只有一边正确（一边 76 一边 38），
// 正是这类「同一事实两份实现」的漂移。

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const WEB = path.resolve(HERE, '..');
export const ROOT = path.resolve(WEB, '..');

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === 'dist' || name === 'target' || name === 'coverage') continue;
    const full = path.join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) walk(full, out);
    else out.push(full);
  }
  return out;
}

function countRustTests() {
  // 口径：源码里声明的全部 `#[test]` 用例（跨平台声明总数）。
  // 与「本机实际通过数」不同：带 `#[cfg(unix)]` 的用例在 Windows 不编译，
  // 带 `#[ignore]` 的默认不执行。文档同时给出声明数与实测跑法，避免夸大。
  const srcDir = path.join(WEB, 'src-tauri', 'src');
  let n = 0;
  for (const file of walk(srcDir)) {
    if (!file.endsWith('.rs')) continue;
    const text = readFileSync(file, 'utf8');
    n += (text.match(/^\s*#\[test\]\s*$/gm) || []).length;
  }
  return n;
}

function countFrontendTests() {
  let files = 0;
  let cases = 0;
  for (const file of walk(path.join(WEB, 'src'))) {
    if (!/\.test\.ts$/.test(file)) continue;
    files += 1;
    const text = readFileSync(file, 'utf8');
    // it( / it.each / test( —— 只数用例声明，不数 expect
    cases += (text.match(/^\s*(?:it|test)(?:\.each\([^)]*\))?\s*\(/gm) || []).length;
  }
  return { files, cases };
}

function countIpcCommands() {
  const lib = readFileSync(path.join(WEB, 'src-tauri', 'src', 'lib.rs'), 'utf8');
  const start = lib.indexOf('tauri::generate_handler![');
  const end = lib.indexOf('])', start);
  const block = lib.slice(start, end);
  return (block.match(/commands::\w+|notify::\w+/g) || []).length;
}

function countTables() {
  const db = readFileSync(path.join(WEB, 'src-tauri', 'src', 'db.rs'), 'utf8');
  return (db.match(/CREATE TABLE IF NOT EXISTS (\w+)/g) || []).length;
}

function countPlatforms() {
  const data = readFileSync(path.join(WEB, 'src-tauri', 'src', 'platform_data.rs'), 'utf8');
  const ids = new Set();
  for (const m of data.matchAll(/PlatformEntry::(?:new|with_source)\(\s*(\d+),/g)) ids.add(m[1]);
  return ids.size;
}

function countE2E() {
  // 必须问 Playwright：路由×语言×主题矩阵由 for 循环生成，静态数 test( 只有 38，实际 76。
  try {
    const out = execSync('npx playwright test --list', {
      cwd: WEB,
      encoding: 'utf8',
      timeout: 120_000,
    });
    const m = out.match(/Total:\s*(\d+)\s+tests?/);
    if (m) return Number(m[1]);
  } catch {
    // --list 不可用：返回 0，调用方按「未校验」处理，绝不用会漂移的静态值
  }
  return 0;
}

export function facts() {
  return {
    rustTests: countRustTests(),
    frontendCases: countFrontendTests().cases,
    frontendFiles: countFrontendTests().files,
    ipc: countIpcCommands(),
    tables: countTables(),
    platforms: countPlatforms(),
  };
}

export function e2eCount() {
  return countE2E();
}
