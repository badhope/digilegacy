// E2E 覆盖率收集：Playwright 跑完后从页面里取 window.__coverage__，合并成报告。
//
// 用法：
//   E2E_COVERAGE=1 npx playwright test        # 先带插桩跑
//   node e2e/collect-coverage.mjs             # 再生成报告
//
// 说明：E2E 覆盖率与单元覆盖率是两套口径，不合并进 vitest 的阈值门禁。
// 它的价值是回答「用户真的点到的路径，代码执行了多少」，单测无法替代。

import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const COV_DIR = path.join(HERE, '.coverage');
const OUT = path.join(HERE, '..', 'coverage-e2e');

function message(error) {
  return error instanceof Error ? error.message : String(error);
}

function validateCounterMap(value, kind, fileName, sourceFile) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`E2E 覆盖率文件格式无效（${fileName}，${sourceFile}，${kind}）`);
  }
  for (const [id, counts] of Object.entries(value)) {
    const values = kind === 'b' ? counts : [counts];
    if (!Array.isArray(values) || values.some(count => !Number.isInteger(count) || count < 0)) {
      throw new Error(`E2E 覆盖率计数无效（${fileName}，${sourceFile}，${kind}.${id}）`);
    }
  }
}

export function validateCoverageData(raw, fileName = '<synthetic>') {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw) || Object.keys(raw).length === 0) {
    throw new Error(`E2E 覆盖率文件没有有效源文件数据：${fileName}`);
  }
  for (const [sourceFile, data] of Object.entries(raw)) {
    if (!data || typeof data !== 'object' || Array.isArray(data)) {
      throw new Error(`E2E 覆盖率文件格式无效（${fileName}，${sourceFile}）`);
    }
    for (const kind of ['s', 'f', 'b']) {
      validateCounterMap(data[kind], kind, fileName, sourceFile);
    }
    if (!data.branchMap || typeof data.branchMap !== 'object' || Array.isArray(data.branchMap)) {
      throw new Error(`E2E 覆盖率 branchMap 格式无效（${fileName}，${sourceFile}）`);
    }
    for (const [id, counts] of Object.entries(data.b)) {
      const branch = data.branchMap?.[id];
      if (branch?.locations && counts.length !== branch.locations.length) {
        throw new Error(`E2E 覆盖率 branch 计数长度无效（${fileName}，${sourceFile}，b.${id}）`);
      }
    }
  }
  return raw;
}

function addScalarCounters(target, source) {
  for (const [id, count] of Object.entries(source)) {
    target[id] = (target[id] ?? 0) + count;
  }
}

function addBranchCounters(target, source, fileName, sourceFile) {
  for (const [id, counts] of Object.entries(source)) {
    if (!target[id]) {
      target[id] = [...counts];
      continue;
    }
    if (target[id].length !== counts.length) {
      throw new Error(`E2E 覆盖率 branch 计数长度不一致（${fileName}，${sourceFile}，b.${id}）`);
    }
    target[id] = target[id].map((count, index) => count + counts[index]);
  }
}

export function mergeCoverage(fragments) {
  const merged = {};
  for (const { fileName, raw } of fragments) {
    validateCoverageData(raw, fileName);
    for (const [sourceFile, data] of Object.entries(raw)) {
      if (!merged[sourceFile]) {
        merged[sourceFile] = structuredClone(data);
        continue;
      }
      addScalarCounters(merged[sourceFile].s, data.s);
      addScalarCounters(merged[sourceFile].f, data.f);
      addBranchCounters(merged[sourceFile].b, data.b, fileName, sourceFile);
    }
  }
  if (Object.keys(merged).length === 0) {
    throw new Error('合并后的 E2E 覆盖率没有源文件数据');
  }
  return merged;
}

async function main() {
  let files;
  try {
    files = (await readdir(COV_DIR)).filter(file => file.endsWith('.json'));
  } catch (error) {
    if (error && typeof error === 'object' && error.code === 'ENOENT') {
      throw new Error(`未找到 E2E 覆盖率数据：${COV_DIR}。请先运行：E2E_COVERAGE=1 npx playwright test`);
    }
    throw new Error(`读取 E2E 覆盖率目录失败：${message(error)}`);
  }

  if (files.length === 0) {
    throw new Error(`E2E 覆盖率目录没有 JSON 数据文件：${COV_DIR}`);
  }

  const fragments = [];
  for (const fileName of files) {
    const filePath = path.join(COV_DIR, fileName);
    try {
      fragments.push({ fileName, raw: JSON.parse(await readFile(filePath, 'utf-8')) });
    } catch (error) {
      throw new Error(`读取 E2E 覆盖率文件失败（${fileName}）：${message(error)}`);
    }
  }
  const merged = mergeCoverage(fragments);

  let libCoverage;
  try {
    const { createRequire } = await import('node:module');
    const require = createRequire(import.meta.url);
    libCoverage = require('istanbul-lib-coverage');
  } catch (error) {
    throw new Error(`加载 istanbul-lib-coverage 失败：${message(error)}`);
  }

  let summary;
  try {
    const map = libCoverage.createCoverageMap(merged);
    const filesInMap = map.files();
    if (filesInMap.length === 0) throw new Error('覆盖率映射没有源文件');
    const sum = libCoverage.createCoverageSummary();
    filesInMap.forEach(file => sum.merge(map.fileCoverageFor(file).toSummary()));
    summary = {
      statements: sum.statements.pct,
      branches: sum.branches.pct,
      functions: sum.functions.pct,
      lines: sum.lines.pct,
      files: filesInMap.length,
    };
  } catch (error) {
    throw new Error(`汇总 E2E 覆盖率失败：${message(error)}`);
  }

  try {
    await mkdir(OUT, { recursive: true });
    await writeFile(path.join(OUT, 'coverage-final.json'), JSON.stringify(merged, null, 2));
    await writeFile(path.join(OUT, 'summary.json'), JSON.stringify(summary, null, 2));
  } catch (error) {
    throw new Error(`写入 E2E 覆盖率报告失败：${message(error)}`);
  }

  console.log('E2E 覆盖率已生成：', path.join(OUT, 'summary.json'));
  console.log(summary);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch(error => {
    console.error(`E2E 覆盖率收集失败：${message(error)}`);
    process.exitCode = 1;
  });
}
