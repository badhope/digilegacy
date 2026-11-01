import assert from 'node:assert/strict';
import test from 'node:test';
import { mergeCoverage, validateCoverageData } from './collect-coverage.mjs';

function fragment(branches, statements = 1, functions = 1) {
  return {
    'src/example.js': {
      path: 'src/example.js',
      statementMap: {},
      fnMap: {},
      branchMap: {
        '0': { line: 1, type: 'if', locations: [{ line: 1 }, { line: 1 }] },
      },
      s: { '0': statements },
      f: { '0': functions },
      b: { '0': branches },
    },
  };
}

test('合并 synthetic coverage 时 s/f/b 均按计数相加', () => {
  const merged = mergeCoverage([
    { fileName: 'first.json', raw: fragment([1, 2], 2, 3) },
    { fileName: 'second.json', raw: fragment([4, 5], 7, 11) },
  ]);
  assert.deepEqual(merged['src/example.js'].s, { '0': 9 });
  assert.deepEqual(merged['src/example.js'].f, { '0': 14 });
  assert.deepEqual(merged['src/example.js'].b, { '0': [5, 7] });
});

test('拒绝缺失 branch map 计数和错误 branch 长度', () => {
  assert.throws(
    () => validateCoverageData({ 'src/bad.js': { s: {}, f: {}, b: {} } }, 'missing.json'),
    /branchMap 格式无效|计数无效/,
  );
  assert.throws(
    () => mergeCoverage([
      { fileName: 'bad-a.json', raw: fragment([1, 2]) },
      { fileName: 'bad-b.json', raw: fragment([3]) },
    ]),
    /branch 计数长度不一致|branch 计数长度无效/,
  );
});
