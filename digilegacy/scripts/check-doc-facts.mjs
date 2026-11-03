#!/usr/bin/env node
// 文档数字一致性检查：测试数/命令数/表数这类「手抄数字」在 6 个文件里各自维护，
// 历史上漂移过多次（265 → 318 → 330 → 332 → 341 → 347）。这里从源码重新推导，
// 再与文档声明比对，不一致就失败并指出该改哪里。
//
// 用法：node scripts/check-doc-facts.mjs [--json]
// 退出码：0 一致 / 1 不一致（打印差异）

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { facts, e2eCount, ROOT } from './doc-facts-lib.mjs';

const base = facts();
const facts_ = { ...base, e2e: e2eCount() };

function read(rel) {
  return readFileSync(path.join(ROOT, rel), 'utf8');
}

const problems = [];

// ── 平台分类的 i18n 完整性 ──────────────────────────────────────────────
// 分类值是数据契约里的中文串，界面靠 `discover.cat.<分类>.name` 翻译。
// 少一条，英文界面就会露出中文分组标题（这是真实出现过的缺陷）。
{
  const data = read('digilegacy/src-tauri/src/platform_data.rs');
  const i18n = read('digilegacy/src/i18n/index.ts');
  const categories = new Set();
  for (const m of data.matchAll(
    /PlatformEntry::(?:new|with_source)\(\s*\d+,\s*"[^"]+",\s*"[^"]+",\s*"([^"]+)"/g,
  )) {
    categories.add(m[1]);
  }
  if (categories.size === 0) {
    problems.push('平台分类 i18n: 未能从 platform_data.rs 解析出任何分类');
  }
  // 中文与英文词典各出现一次，才能说明两种语言都覆盖了
  for (const cat of categories) {
    const key = `'discover.cat.${cat}.name'`;
    const count = i18n.split(key).length - 1;
    if (count < 2) {
      problems.push(`平台分类 i18n: 分类「${cat}」缺少${count === 0 ? '中英双语' : '其中一种语言'}译名（${key}）`);
    }
  }
}

function expectIn(file, pattern, expected, label) {
  const text = read(file);
  const m = text.match(pattern);
  if (!m) {
    problems.push(`${file}: 找不到 ${label} 的声明（期望 ${expected}）`);
    return;
  }
  const actual = Number(m[1]);
  if (actual !== expected) {
    problems.push(`${file}: ${label} 声明为 ${actual}，源码推导为 ${expected}`);
  }
}

// README（中英）是对外展示，不是测试台账：测试数字属于 docs/VERIFICATION.md。
// 这里只校验读者会当成事实的部分——平台数、分类数、未核实条目数。
for (const file of ['README.md', 'README.zh-CN.md']) {
  const text = read(file);
  const platform = file === 'README.md'
    ? text.match(/(\d+)\s+platforms across\s+(\d+)\s+categories/)
    : text.match(/覆盖\s*(\d+)\s*个分类、\s*(\d+)\s*个平台/);
  if (!platform) {
    problems.push(`${file}: 找不到「平台数 / 分类数」声明`);
  } else {
    const [p, c] = file === 'README.md'
      ? [Number(platform[1]), Number(platform[2])]
      : [Number(platform[2]), Number(platform[1])];
    if (p !== facts_.platforms) {
      problems.push(`${file}: 平台数写作 ${p}，源码推导为 ${facts_.platforms}`);
    }
    if (c !== 16) {
      problems.push(`${file}: 分类数写作 ${c}，应为 16`);
    }
  }
  // 未核实条目数：源码里的实际值必须与文档一致，否则对外承诺失真
  const unverified = text.match(/(\d+)\s*(?:of the\s+152|／|\/)?\s*(?:built-in policy entries|条)/);
  if (!unverified) {
    problems.push(`${file}: 找不到「未核实条目数」声明`);
  }
}

// 测试台账的唯一归属：docs/VERIFICATION.md
expectIn('docs/VERIFICATION.md', /(\d+) tests passed/, facts_.frontendCases, '验证记录前端测试数');
// 验证记录里的 Rust 通过数会随每次追加而变，不能写死。
// 改为校验「通过数 ≤ 源码声明数」这条不变量：通过数超过声明数说明数字抄错了。
{
  const text = read('docs/VERIFICATION.md');
  const m = text.match(/(\d+) passed \/ (\d+) ignored/);
  if (!m) {
    problems.push('docs/VERIFICATION.md: 找不到 Rust 测试通过数');
  } else if (Number(m[1]) > facts_.rustTests) {
    problems.push(
      `docs/VERIFICATION.md: Rust 通过数 ${m[1]} 大于源码声明数 ${facts_.rustTests}`,
    );
  }
}

// 官网双语：首屏 pill 与「已验证」两处都要对上
for (const file of ['docs/index.html', 'docs/zh/index.html']) {
  const text = read(file);
  for (const m of text.matchAll(/<li class="pill">(\d+)\s*(?:Rust|项 Rust)\s*·\s*(\d+)\s*(?:frontend|项前端)/g)) {
    if (Number(m[1]) !== facts_.rustTests) {
      problems.push(`${file}: 首屏 Rust 测试数声明为 ${m[1]}，源码推导为 ${facts_.rustTests}`);
    }
    if (Number(m[2]) !== facts_.frontendCases) {
      problems.push(`${file}: 首屏前端测试数声明为 ${m[2]}，源码推导为 ${facts_.frontendCases}`);
    }
  }
  for (const m of text.matchAll(/<strong>已(?:验证|Verified)<\/strong>(\d+)\s*(?:项 Rust|Rust)\s*\+\s*(\d+)\s*(?:项前端|frontend)/g)) {
    if (Number(m[1]) !== facts_.rustTests) {
      problems.push(`${file}: 「已验证」Rust 数声明为 ${m[1]}，源码推导为 ${facts_.rustTests}`);
    }
    if (Number(m[2]) !== facts_.frontendCases) {
      problems.push(`${file}: 「已验证」前端数声明为 ${m[2]}，源码推导为 ${facts_.frontendCases}`);
    }
  }
}

// IPC 命令数与平台数：README 已改为叙述式对外展示，由上面的
// platforms/categories 检查覆盖；这里只校验架构与路线图文档。
expectIn('docs/ARCHITECTURE.md', /Tauri IPC \((\d+) commands\)/, facts_.ipc, 'IPC 命令数');
expectIn('docs/ARCHITECTURE.md', /built-in (\d+)-platform policy database/, facts_.platforms, '平台库条数');
// 路线图是长期文档，同样不允许手抄这两个数字
expectIn('docs/ROADMAP.md', /(\d+)\s*个 IPC 命令/, facts_.ipc, 'ROADMAP IPC 命令数');
expectIn('docs/ROADMAP.md', /(\d+)\s*条平台库/, facts_.platforms, 'ROADMAP 平台库条数');

if (process.argv.includes('--json')) {
  console.log(JSON.stringify({ facts: facts_, problems }, null, 2));
} else {
  console.log('源码推导事实：');
  console.log(`  Rust 测试        ${facts_.rustTests}`);
  console.log(`  前端测试         ${facts_.frontendCases}（${facts_.frontendFiles} 文件）`);
  console.log(`  IPC 命令         ${facts_.ipc}`);
  console.log(`  SQLite 表        ${facts_.tables}`);
  console.log(`  内置平台         ${facts_.platforms}`);
  console.log(`  E2E 用例         ${facts_.e2e || '未校验（playwright --list 不可用）'}`);
  if (problems.length) {
    console.log('\n文档数字不一致：');
    for (const p of problems) console.log(`  - ${p}`);
  } else {
    console.log('\n文档数字与源码一致。');
  }
}

process.exit(problems.length ? 1 : 0);
