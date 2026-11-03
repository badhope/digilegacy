// 归零 · 云端心跳服务器（参考实现）
//
// 设计原则（与客户端 heartbeat.rs 对应）：
//   1. 零内容：只接收令牌，不接收/不存储任何资产、凭据、档案数据；
//   2. 失联提醒：令牌超期未心跳 → /check 返回提醒载荷，由你的定时任务转发 webhook；
//   3. 可自托管：单文件、无外部依赖，状态落一个 JSON 文件。
//
// 运行（产品服务默认使用 HTTPS）：
//   HEARTBEAT_ADMIN_TOKEN=<长随机串> HEARTBEAT_TLS_CERT=/path/cert.pem HEARTBEAT_TLS_KEY=/path/key.pem node heartbeat-server.mjs
//
// 客户端配置（应用内「我的档案 → 云端心跳」，或环境变量）：
//   endpoint = https://<host>:8080     token = <在 registrations.json 里预注册的令牌>
//
// 仅行为测试 fixture 可显式设置 HEARTBEAT_TEST_HTTP=1 使用 HTTP；该开关不得用于产品部署。
//
// registrations.json（与脚本同目录，不入库）：
//   { "<token>": { "notify_webhook": "https://…", "grace_days": 21 } }
//
// 安全边界（相比早期版本的三处加固）：
//   - `/check` 必须带 `Authorization: Bearer $HEARTBEAT_ADMIN_TOKEN`；
//     未配置管理令牌时直接 503，避免「谁都能读到全部令牌与 webhook」。
//   - 心跳只接受**已注册**令牌（除非显式 HEARTBEAT_ALLOW_ANY=1 用于本地试验），
//     否则任意字符串都会写进状态文件，可被用来撑爆磁盘。
//   - 告警状态随 state.json 持久化，并在收到新心跳时复位；
//     旧实现把 `_alerted` 放在内存里，进程重启后会重复轰炸，且第二次失联永不告警。

import http from 'node:http';
import https from 'node:https';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const PORT = Number(process.env.HEARTBEAT_PORT || 8080);
const ADMIN_TOKEN = process.env.HEARTBEAT_ADMIN_TOKEN || '';
const ALLOW_ANY = process.env.HEARTBEAT_ALLOW_ANY === '1';
const TEST_HTTP = process.env.HEARTBEAT_TEST_HTTP === '1';
const TLS_CERT = process.env.HEARTBEAT_TLS_CERT || '';
const TLS_KEY = process.env.HEARTBEAT_TLS_KEY || '';
if (!TEST_HTTP && (!TLS_CERT || !TLS_KEY)) {
  console.error('产品心跳服务器必须配置 HEARTBEAT_TLS_CERT 与 HEARTBEAT_TLS_KEY；HTTP 仅可由 HEARTBEAT_TEST_HTTP=1 测试 fixture 启用');
  process.exit(1);
}
const MAX_BODY = 4096;
const MAX_TOKENS = Number(process.env.HEARTBEAT_MAX_TOKENS || 10000);
const DEFAULT_GRACE_DAYS = 21;

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// 数据目录可覆盖：测试用临时目录，部署时一般保持默认（脚本同目录）
const DATA_DIR = process.env.HEARTBEAT_DATA_DIR || __dirname;
const REG_FILE = path.join(DATA_DIR, 'registrations.json');
const STATE_FILE = path.join(DATA_DIR, 'state.json');

function loadJson(file, fallback) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    return fallback;
  }
}

const registrations = loadJson(REG_FILE, {});
if (typeof registrations !== 'object' || registrations === null || Array.isArray(registrations)) {
  console.error('registrations.json 必须是 { "<token>": {...} } 形式的对象');
  process.exit(1);
}

/** state: { "<token>": { last_seen: ISO, alerted: boolean } } */
const state = loadJson(STATE_FILE, {});

function saveState() {
  const tmp = `${STATE_FILE}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(state, null, 2));
  fs.renameSync(tmp, STATE_FILE);
}

function json(res, code, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(code, {
    'Content-Type': 'application/json; charset=utf-8',
    'Cache-Control': 'no-store',
    'Content-Length': Buffer.byteLength(body),
  });
  res.end(body);
}

/** 定长比较，避免用 === 比较管理令牌时泄漏前缀长度信息 */
function safeEqual(a, b) {
  const ba = Buffer.from(String(a));
  const bb = Buffer.from(String(b));
  if (ba.length !== bb.length) return false;
  return crypto.timingSafeEqual(ba, bb);
}

function authorized(req) {
  const header = req.headers['authorization'] || '';
  const m = /^Bearer\s+(.+)$/i.exec(String(header));
  return !!ADMIN_TOKEN && !!m && safeEqual(m[1], ADMIN_TOKEN);
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    let overflow = false;
    req.on('data', (chunk) => {
      if (overflow) return; // 超限后继续排空但不累积，避免占用内存
      body += chunk;
      if (body.length > MAX_BODY) {
        overflow = true;
        body = '';
      }
    });
    req.on('end', () => {
      // 不 destroy 连接：直接断开会让客户端收到 "fetch failed"，
      // 拿不到明确的 413，排障时会被误判成服务器没起来。
      if (overflow) reject(Object.assign(new Error('body too large'), { code: 413 }));
      else resolve(body);
    });
    req.on('error', reject);
  });
}

const startedAt = Date.now();

const requestHandler = async (req, res) => {
  const url = new URL(req.url || '/', `http://localhost:${PORT}`);
  const route = url.pathname;

  if (req.method === 'GET' && (route === '/health' || route === '/')) {
    json(res, 200, { ok: true, uptime_seconds: Math.floor((Date.now() - startedAt) / 1000) });
    return;
  }

  if (req.method === 'POST' && route === '/heartbeat') {
    let body;
    try {
      body = await readBody(req);
    } catch (e) {
      if (e.code === 413) json(res, 413, { error: 'body too large' });
      else json(res, 400, { error: 'bad request' });
      return;
    }

    let token;
    try {
      token = JSON.parse(body).token;
    } catch {
      json(res, 400, { error: 'bad json' });
      return;
    }
    if (typeof token !== 'string' || token.length === 0 || token.length > 256) {
      json(res, 400, { error: 'token required' });
      return;
    }
    if (!ALLOW_ANY && !Object.prototype.hasOwnProperty.call(registrations, token)) {
      // 未注册令牌一律拒绝：既防状态文件被灌爆，也让配置错误立即暴露
      json(res, 403, { error: 'unknown token' });
      return;
    }
    if (!ALLOW_ANY && !Object.prototype.hasOwnProperty.call(state, token) && Object.keys(state).length >= MAX_TOKENS) {
      json(res, 429, { error: 'too many tokens' });
      return;
    }

    state[token] = { last_seen: new Date().toISOString(), alerted: false };
    saveState();
    json(res, 200, { ok: true });
    return;
  }

  if (req.method === 'GET' && route === '/check') {
    if (!ADMIN_TOKEN) {
      json(res, 503, { error: 'HEARTBEAT_ADMIN_TOKEN 未配置，/check 已禁用（fail-closed）' });
      return;
    }
    if (!authorized(req)) {
      json(res, 401, { error: 'unauthorized' });
      return;
    }

    const now = Date.now();
    const alerts = [];
    for (const [token, reg] of Object.entries(registrations)) {
      const entry = state[token];
      if (!entry || !entry.last_seen) continue;
      const graceDays = Number(reg.grace_days ?? DEFAULT_GRACE_DAYS);
      const silentDays = (now - new Date(entry.last_seen).getTime()) / 86_400_000;
      if (silentDays >= graceDays && !entry.alerted) {
        alerts.push({
          token,
          last_seen: entry.last_seen,
          silent_days: Math.floor(silentDays),
          webhook: reg.notify_webhook,
        });
        entry.alerted = true;
      }
    }
    if (alerts.length > 0) saveState();

    json(res, 200, { alerts, checked_at: new Date().toISOString() });
    return;
  }

  json(res, 404, { error: 'not found' });
};

const server = TEST_HTTP
  ? http.createServer(requestHandler)
  : https.createServer({
      cert: fs.readFileSync(TLS_CERT),
      key: fs.readFileSync(TLS_KEY),
    }, requestHandler);
const protocol = TEST_HTTP ? 'http' : 'https';

server.listen(PORT, () => {
  console.log(`心跳服务器已启动: ${protocol}://localhost:${PORT}`);
  console.log(`  POST /heartbeat  {"token":"..."}          （需已注册）`);
  console.log(`  GET  /check      检查超时令牌              （需 Bearer 管理令牌）`);
  console.log(`  GET  /health     存活探针`);
  if (!ADMIN_TOKEN) {
    console.warn('警告：未设置 HEARTBEAT_ADMIN_TOKEN，/check 将返回 503（这是有意的 fail-closed）。');
  }
  if (ALLOW_ANY) {
    console.warn('警告：HEARTBEAT_ALLOW_ANY=1，任意令牌都会被接受——仅用于本地试验。');
  }
});

for (const sig of ['SIGINT', 'SIGTERM']) {
  process.on(sig, () => {
    console.log(`收到 ${sig}，正在关闭…`);
    server.close(() => process.exit(0));
  });
}
