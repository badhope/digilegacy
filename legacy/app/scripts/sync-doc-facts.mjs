#!/usr/bin/env node
// 文档数字同步：把源码推导出的测试数写进 README/官网/架构文档。
//
// 为什么需要：这些数字在 6 个文件里各写一遍，历史上漂移过 6 次。
// `check-doc-facts.mjs` 负责「发现不一致」，本脚本负责「按源码修正」。
// CI 只跑 check（不允许自动改文件）；本地改完测试后跑本脚本再提交。
//
// 用法：node scripts/sync-doc-facts.mjs [--dry-run]

import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { facts, e2eCount, ROOT } from './doc-facts-lib.mjs';

const dryRun = process.argv.includes('--dry-run');
const f = facts();
const e2e = e2eCount();
const changed = [];

function patch(rel, replacer) {
  const full = path.join(ROOT, rel);
  const before = readFileSync(full, 'utf8');
  const after = replacer(before);
  if (after !== before) {
    changed.push(rel);
    if (!dryRun) writeFileSync(full, after, 'utf8');
  }
}

const readmePatch = text =>
  text
    .replace(/tests-\d+%20Rust%20%C2%B7%20\d+%20frontend%20%C2%B7%20\d+%20E2E/g,
      `tests-${f.rustTests}%20Rust%20%C2%B7%20${f.frontendCases}%20frontend%20%C2%B7%20${e2e}%20E2E`)
    // Rust 行保留「通过数 + ignored + 声明数」的诚实格式：
    // 通过数依赖平台（cfg(unix)）与 #[ignore]，无法从源码静态推导，
    // 因此只回填「声明数」，通过数保持原值。
    .replace(
      /((?:Rust tests|Rust 测试)\s*\|\s*\*\*)(\d+)([^\n]*?)(\d+)(\s*(?:declared|声明))/g,
      (_m, head, passed, mid, _declared, tail) => `${head}${passed}${mid}${f.rustTests}${tail}`,
    )
    .replace(/(\| Frontend unit tests \| \*\*)\d+/, `$1${f.frontendCases}`)
    .replace(/(\| 前端单元测试 \| \*\*)\d+/, `$1${f.frontendCases}`)
    .replace(/(\| Frontend unit tests \| \*\*\d+ passing\*\* across )\d+( files \|)/, `$1${f.frontendFiles}$2`)
    .replace(/(\| 前端单元测试 \| \*\*\d+ 通过\*\*，)\d+( 个文件 \|)/, `$1${f.frontendFiles}$2`)
    .replace(/(\| End-to-end tests \| \*\*)\d+/, `$1${e2e}`)
    .replace(/(\| 端到端测试 \| \*\*)\d+/, `$1${e2e}`)
    .replace(/\*\*\d+\*\* IPC commands/g, `**${f.ipc}** IPC commands`)
    .replace(/\*\*\d+\*\* 个 IPC 命令/g, `**${f.ipc}** 个 IPC 命令`)
    .replace(/\*\*\d+\*\* platforms in the built-in/g, `**${f.platforms}** platforms in the built-in`)
    .replace(/\*\*\d+\*\* 个平台、/g, `**${f.platforms}** 个平台、`);

for (const file of ['README.md', 'README.zh-CN.md']) patch(file, readmePatch);

const sitePatch = text =>
  text
    .replace(/(<li class="pill">)\d+( Rust · | 项 Rust · )\d+( frontend tests| 项前端测试)/g,
      `$1${f.rustTests}$2${f.frontendCases}$3`)
    .replace(/(<strong>已验证<\/strong>)\d+( 项 Rust \+ )\d+( 项前端 \+ )\d+( 项 E2E)/g,
      `$1${f.rustTests}$2${f.frontendCases}$3${e2e}$4`)
    .replace(/(<strong>Verified<\/strong>)\d+( Rust \+ )\d+( frontend \+ )\d+( E2E)/g,
      `$1${f.rustTests}$2${f.frontendCases}$3${e2e}$4`)
    .replace(/。\d+ 项 Rust 测试、\d+ 项前端测试、\d+ 项端到端测试/g,
      `。${f.rustTests} 项 Rust 测试、${f.frontendCases} 项前端测试、${e2e} 项端到端测试`)
    .replace(/\. \d+ Rust tests, \d+ frontend tests and \d+ end-to-end tests/g,
      `. ${f.rustTests} Rust tests, ${f.frontendCases} frontend tests and ${e2e} end-to-end tests`);

for (const file of ['docs/index.html', 'docs/zh/index.html']) patch(file, sitePatch);

patch('docs/ARCHITECTURE.md', text =>
  text
    .replace(/Tauri IPC \(\d+ commands\)/g, `Tauri IPC (${f.ipc} commands)`)
    .replace(/built-in \d+-platform policy database/g, `built-in ${f.platforms}-platform policy database`),
);

// 路线图：只回填受门禁保护的两个数字；其余数字是基线提交的快照，不自动改写。
patch('docs/ROADMAP.md', text =>
  text
    .replace(/\d+ 个 IPC 命令/g, `${f.ipc} 个 IPC 命令`)
    .replace(/\d+ 条平台库/g, `${f.platforms} 条平台库`),
);

console.log(`源码推导：Rust ${f.rustTests} · 前端 ${f.frontendCases}（${f.frontendFiles} 文件）· E2E ${e2e} · IPC ${f.ipc} · 表 ${f.tables} · 平台 ${f.platforms}`);
if (changed.length === 0) {
  console.log('所有文档已是最新。');
} else {
  console.log(`${dryRun ? '需要更新' : '已更新'}：`);
  for (const c of changed) console.log(`  - ${c}`);
}
