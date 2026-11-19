#!/usr/bin/env node
// 政策库溯源门禁。
//
// 为什么需要：内容型产品的信任来自可追溯。旧实现 152 条里只有 5 条带
// `data_source`，其余 147 条既无出处也无核对日期，界面却和「有据可查」
// 的条目长得一样——用户在需要准确性的时刻无法分辨。
//
// 设计原则：**绝不为了过门禁而编造出处或核对日期**。
// 因此分两类规则：
//
//   A. 硬规则（必须通过）：
//      1. 每条政策必须有 data_source；未核实的必须写显式的
//         `未核实（待人工核对官方公告）`，不允许留空。
//      2. 声明为可继承（inheritable=true 且难度不是「暂不支持」）的条目，
//         official_policy_url 必须非空。
//      3. 不允许出现占位式来源（如「TODO」「待补充」「xx」）。
//
//   B. 棘轮规则（只能变好，不能变差）：
//      4. 未核实条目数不得超过基线。
//      5. 有出处但缺核对日期（last_reviewed）的条目数不得超过基线。
//         核对日期只能由人工真实核对后填写，脚本不会自动填。
//
// 用法：
//   node scripts/check-policy-provenance.mjs            # 检查
//   node scripts/check-policy-provenance.mjs --json     # 机器可读
//   node scripts/check-policy-provenance.mjs --update-baseline
//        仅在**确实核对了更多条目**后使用；会在输出里显示前后差异，
//        若未核实数上升则拒绝写入。

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';
import { WEB } from './doc-facts-lib.mjs';

const SRC = path.join(WEB, 'src-tauri', 'src', 'platform_data.rs');
const BASELINE = path.join(WEB, 'scripts', 'policy-provenance-baseline.json');
const UNVERIFIED_MARK = '未核实（待人工核对官方公告）';

/** 逐条解析 PlatformEntry 调用的参数表（括号配平 + 字符串感知）。 */
function parseEntries(text) {
  const entries = [];
  const re = /PlatformEntry::(new|with_source)\(/g;
  let m;
  while ((m = re.exec(text)) !== null) {
    const kind = m[1];
    let i = re.lastIndex;
    let depth = 1;
    let inStr = false;
    let escaped = false;
    const start = i;
    while (i < text.length && depth > 0) {
      const ch = text[i];
      if (inStr) {
        if (escaped) escaped = false;
        else if (ch === '\\') escaped = true;
        else if (ch === '"') inStr = false;
      } else if (ch === '"') {
        inStr = true;
      } else if (ch === '(') {
        depth += 1;
      } else if (ch === ')') {
        depth -= 1;
      }
      i += 1;
    }
    entries.push({ kind, body: text.slice(start, i - 1) });
  }
  return entries;
}

function splitArgs(body) {
  const args = [];
  let cur = '';
  let depth = 0;
  let inStr = false;
  let escaped = false;
  for (const ch of body) {
    if (inStr) {
      cur += ch;
      if (escaped) escaped = false;
      else if (ch === '\\') escaped = true;
      else if (ch === '"') inStr = false;
      continue;
    }
    if (ch === '"') {
      inStr = true;
      cur += ch;
      continue;
    }
    if (ch === '(' || ch === '[' || ch === '{') depth += 1;
    if (ch === ')' || ch === ']' || ch === '}') depth -= 1;
    if (ch === ',' && depth === 0) {
      args.push(cur.trim());
      cur = '';
      continue;
    }
    cur += ch;
  }
  if (cur.trim()) args.push(cur.trim());
  return args;
}

const strOf = raw => {
  const m = /^"((?:[^"\\]|\\.)*)"$/.exec((raw ?? '').trim());
  return m ? m[1] : '';
};

// 两个构造函数的差异只在末尾：
//   new(id..notes)                       14 个参数，data_source 由构造函数写成「未核实」
//   with_source(id..notes, data_source)  15 个参数，data_source 是真实出处
function analyze() {
  const text = readFileSync(SRC, 'utf8');
  const raw = parseEntries(text);
  const rows = [];
  const problems = [];

  for (const e of raw) {
    const a = splitArgs(e.body);
    const id = Number(a[0] ?? 0);
    const name = strOf(a[1] ?? '');
    const inheritable = (a[7] ?? '').trim() === 'true';
    const difficulty = strOf(a[10] ?? '');
    const url = strOf(a[11] ?? '');
    // new 构造的条目 data_source 在构造函数里被写成「未核实」，参数里没有这一项
    const dataSource = e.kind === 'with_source' ? strOf(a[14] ?? '') : UNVERIFIED_MARK;

    rows.push({ id, name, dataSource, inheritable, difficulty, url });

    if (!dataSource.trim()) {
      problems.push(`#${id} ${name}：data_source 为空（必须是真实出处或显式「未核实」）`);
    } else if (/^(todo|tbd|待补充|待定|xx+)$/i.test(dataSource.trim())) {
      problems.push(`#${id} ${name}：data_source 是占位文本「${dataSource}」`);
    }
    if (inheritable && difficulty !== '暂不支持' && !url.trim()) {
      problems.push(`#${id} ${name}：声明可继承但 official_policy_url 为空`);
    }
  }

  // last_reviewed 通过 enrich 或字段赋值出现；统计带非空日期的条目数。
  const reviewedCount = (text.match(/last_reviewed:\s*"[^"]+"/g) || []).length;
  const unverified = rows.filter(r => r.dataSource.trim() === UNVERIFIED_MARK).length;
  const withRealSource = rows.length - unverified;
  // 有真实出处但没核对日期 = 状态不完整，计入棘轮
  const sourceWithoutReview = Math.max(0, withRealSource - reviewedCount);

  return { total: rows.length, unverified, reviewedCount, sourceWithoutReview, problems };
}

function main() {
  const r = analyze();
  const baseline = existsSync(BASELINE)
    ? JSON.parse(readFileSync(BASELINE, 'utf8'))
    : null;

  const ratchetProblems = [];
  if (baseline) {
    if (r.unverified > baseline.unverified) {
      ratchetProblems.push(
        `未核实条目从基线 ${baseline.unverified} 增加到 ${r.unverified}（不允许变差）`,
      );
    }
    if (r.sourceWithoutReview > baseline.sourceWithoutReview) {
      ratchetProblems.push(
        `有出处但缺核对日期的条目从基线 ${baseline.sourceWithoutReview} 增加到 ${r.sourceWithoutReview}`,
      );
    }
  }

  const all = [...r.problems, ...ratchetProblems];

  if (process.argv.includes('--update-baseline')) {
    if (!baseline) {
      writeFileSync(
        BASELINE,
        JSON.stringify({ unverified: r.unverified, sourceWithoutReview: r.sourceWithoutReview }, null, 2) + '\n',
        'utf8',
      );
      console.log(`已写入基线：未核实 ${r.unverified} / 缺核对日期 ${r.sourceWithoutReview}`);
      process.exit(0);
    }
    if (r.unverified > baseline.unverified || r.sourceWithoutReview > baseline.sourceWithoutReview) {
      console.error('拒绝更新基线：未核实或未核对条目数上升，这说明有政策被改回未核实状态。');
      console.error(`  基线：未核实 ${baseline.unverified} / 缺日期 ${baseline.sourceWithoutReview}`);
      console.error(`  当前：未核实 ${r.unverified} / 缺日期 ${r.sourceWithoutReview}`);
      process.exit(1);
    }
    writeFileSync(
      BASELINE,
      JSON.stringify({ unverified: r.unverified, sourceWithoutReview: r.sourceWithoutReview }, null, 2) + '\n',
      'utf8',
    );
    console.log('基线已收紧：');
    console.log(`  未核实   ${baseline.unverified} → ${r.unverified}`);
    console.log(`  缺日期   ${baseline.sourceWithoutReview} → ${r.sourceWithoutReview}`);
    process.exit(0);
  }

  if (process.argv.includes('--json')) {
    console.log(JSON.stringify({ ...r, baseline, problems: all }, null, 2));
  } else {
    console.log('政策库溯源检查：');
    console.log(`  条目总数                ${r.total}`);
    console.log(`  未核实（显式标注）      ${r.unverified}${baseline ? ` / 基线 ${baseline.unverified}` : ''}`);
    console.log(`  有出处但缺核对日期      ${r.sourceWithoutReview}${baseline ? ` / 基线 ${baseline.sourceWithoutReview}` : ''}`);
    if (all.length === 0) {
      console.log('\n硬规则通过，棘轮指标未变差。');
    } else {
      console.log(`\n不通过（${all.length} 项）：`);
      for (const p of all.slice(0, 40)) console.log(`  - ${p}`);
      if (all.length > 40) console.log(`  ... 另有 ${all.length - 40} 项`);
    }
  }
  process.exit(all.length === 0 ? 0 : 1);
}

main();
