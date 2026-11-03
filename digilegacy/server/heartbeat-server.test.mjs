// 心跳服务器的行为测试：不依赖外部服务，直接起进程 + 打 HTTP。
// 覆盖：未授权 /check 被拒、未注册令牌被拒、正常心跳、413 超限、健康探针。
//
// 运行：node server/heartbeat-server.test.mjs

import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ADMIN = 'test-admin-token-0123456789';
const PORT = 18081;
const BASE = `http://127.0.0.1:${PORT}`;

const work = mkdtempSync(path.join(tmpdir(), 'hb-test-'));
const serverSource = readFileSync(path.join(HERE, 'heartbeat-server.mjs'), 'utf8');
const integrationGuide = readFileSync(path.join(HERE, '..', '..', 'docs', 'AI_INTEGRATION.md'), 'utf8');
const serverOutput = [];

function startServer() {
  const child = spawn(process.execPath, [path.join(HERE, 'heartbeat-server.mjs')], {
    env: {
      ...process.env,
      HEARTBEAT_PORT: String(PORT),
      HEARTBEAT_ADMIN_TOKEN: ADMIN,
      HEARTBEAT_TEST_HTTP: '1',
      // 测试用独立的注册表与状态文件：把 server 目录下的文件复制过去太重，
      // 这里通过临时工作目录 + 环境变量覆盖（见下方 server 支持 HEARTBEAT_DATA_DIR）
      HEARTBEAT_DATA_DIR: work,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', (chunk) => serverOutput.push(String(chunk)));
  return child;
}

async function waitReady(timeoutMs = 8000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const r = await fetch(`${BASE}/health`);
      if (r.ok) return;
    } catch {
      // 还没起来
    }
    await new Promise((r) => setTimeout(r, 120));
  }
  throw new Error('服务器未在超时内就绪');
}

async function main() {
  assert.match(serverSource, /https\.createServer/, 'reference server must provide an HTTPS product server');
  assert.match(serverSource, /HEARTBEAT_TEST_HTTP/, 'HTTP must be explicitly limited to the test fixture');
  assert.match(integrationGuide, /仅允许 HTTPS/, 'AI integration guide must state the heartbeat HTTPS contract');
  assert.doesNotMatch(integrationGuide, /heartbeat[^\n]*http:\/\//i, 'product heartbeat guidance must not advertise HTTP');

  writeFileSync(
    path.join(work, 'registrations.json'),
    JSON.stringify({ 'my-token': { grace_days: 21, notify_webhook: 'https://example.org/hook' } }, null, 2),
  );

  const child = startServer();
  try {
    await waitReady();

    // 1. 健康探针
    let r = await fetch(`${BASE}/health`);
    assert.equal(r.status, 200);
    assert.equal((await r.json()).ok, true);

    // 2. /check 未带管理令牌 → 401（不能泄漏任何令牌）
    r = await fetch(`${BASE}/check`);
    assert.equal(r.status, 401, '/check 无授权必须 401');

    // 3. 带错令牌 → 401
    r = await fetch(`${BASE}/check`, { headers: { Authorization: 'Bearer wrong' } });
    assert.equal(r.status, 401);

    // 4. 未注册令牌 → 403（防状态文件被灌爆）
    r = await fetch(`${BASE}/heartbeat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token: 'not-registered' }),
    });
    assert.equal(r.status, 403);

    // 5. 缺失 token → 400
    r = await fetch(`${BASE}/heartbeat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}',
    });
    assert.equal(r.status, 400);

    // 6. 正常心跳 → 200
    r = await fetch(`${BASE}/heartbeat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token: 'my-token' }),
    });
    assert.equal(r.status, 200);
    assert.equal((await r.json()).ok, true);

    // 7. 授权后 /check 可用，且刚心跳过不会告警
    r = await fetch(`${BASE}/check`, { headers: { Authorization: `Bearer ${ADMIN}` } });
    assert.equal(r.status, 200);
    const payload = await r.json();
    assert.deepEqual(payload.alerts, []);
    assert.ok(payload.checked_at);

    // 8. 超大 body → 413
    r = await fetch(`${BASE}/heartbeat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token: 'x'.repeat(5000) }),
    });
    assert.equal(r.status, 413);

    // 9. 不存在的路径 → 404
    r = await fetch(`${BASE}/nope`);
    assert.equal(r.status, 404);

    console.log('heartbeat-server: all assertions passed');
  } finally {
    child.kill();
    await new Promise((r) => setTimeout(r, 200));
    rmSync(work, { recursive: true, force: true });
  }
}

main().catch((e) => {
  console.error('heartbeat-server tests failed:', e.message);
  process.exitCode = 1;
});
