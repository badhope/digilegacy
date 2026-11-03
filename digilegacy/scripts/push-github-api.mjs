#!/usr/bin/env node
// 通过 GitHub REST API 同步提交到仓库。
//
// 为什么不用 `git push`：本机到 github.com:443 的 git 通道被阻断
// （TLS 握手被 reset），但 api.github.com 可达。git push 无法完成，
// 因此改用 Git Data API：blobs → tree → commit → ref 更新。
//
// 限制与取舍：
// - 逐文件上传，100 个文件约 1-2 分钟；本地网络下可接受。
// - 空目录（如 docs/screenshots/en）不能通过 API 创建，但里面都有文件，不受影响。
// - 二进制文件走 base64，单文件上限 100MB（本仓库最大约 700KB）。

import { execSync } from 'node:child_process';
import { readFileSync, statSync } from 'node:fs';
import path from 'node:path';

const REPOS = [
  { owner: 'badhope', tokenVar: 'GITHUB_TOKEN_BADHOPE' },
  { owner: 'x33834', tokenVar: 'GITHUB_TOKEN_X33834' },
  { owner: 'Morningstar202604', tokenVar: 'GITHUB_TOKEN_MORNINGSTAR202604' },
];

const BRANCH = 'main';
const TOKEN = process.env.GITHUB_TOKEN_BADHOPE;
if (!TOKEN) {
  console.error('缺少 GITHUB_TOKEN_BADHOPE，无法调用 API');
  process.exit(1);
}

const API = 'https://api.github.com';

async function gh(pathname, init = {}, token = TOKEN, attempt = 0) {
  const res = await fetch(`${API}${pathname}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: 'application/vnd.github+json',
      'Content-Type': 'application/json',
      'X-GitHub-Api-Version': '2022-11-28',
      ...(init.headers ?? {}),
    },
  });
  // 二级限流与 5xx 都值得重试：等待时间指数增长（2s, 4s, 8s, 16s, 32s）
  if ((res.status === 403 || res.status === 429 || res.status >= 500) && attempt < 5) {
    const waitMs = 2000 * 2 ** attempt;
    console.log(`
  ${res.status} 限流/瞬断，${waitMs / 1000}s 后重试（第 ${attempt + 1} 次）`);
    await new Promise((r) => setTimeout(r, waitMs));
    return gh(pathname, init, token, attempt + 1);
  }
  const text = await res.text();
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    body = { raw: text };
  }
  if (!res.ok) {
    throw new Error(`${init.method ?? 'GET'} ${pathname} -> ${res.status}: ${text.slice(0, 300)}`);
  }
  return body;
}

/** 列出当前 worktree 的全部已跟踪文件（用 git 判定，不手写 glob）。 */
function listTrackedFiles() {
  const out = execSync('git ls-files -z', { cwd: process.cwd(), maxBuffer: 64 * 1024 * 1024 });
  return out.toString('utf8').split('\0').filter(Boolean);
}

async function uploadBlob(repoOwner, token, filePath) {
  const abs = path.resolve(filePath);
  const stat = statSync(abs);
  const buf = readFileSync(abs);
  // 大于 10MB 走 API 会慢，但本仓库没有这种文件；保留检查以便早失败
  if (buf.length > 90 * 1024 * 1024) {
    throw new Error(`${filePath} 超过 90MB，跳过 API 上传`);
  }
  const body = await gh(
    `/repos/${repoOwner}/digilegacy/git/blobs`,
    {
      method: 'POST',
      body: JSON.stringify({
        content: buf.toString('base64'),
        encoding: 'base64',
      }),
    },
    token,
  );
  return { path: filePath.replace(/\\/g, '/'), mode: '100644', type: 'blob', sha: body.sha, size: stat.size };
}

async function syncRepo({ owner, tokenVar }) {
  const token = process.env[tokenVar];
  if (!token) throw new Error(`缺少 ${tokenVar}`);

  const repoPath = `/repos/${owner}/digilegacy`;
  const ref = await gh(`${repoPath}/git/ref/heads/${BRANCH}`, {}, token);
  const baseCommitSha = ref.object.sha;
  const baseCommit = await gh(`${repoPath}/git/commits/${baseCommitSha}`, {}, token);
  const baseTreeSha = baseCommit.tree.sha;

  const files = listTrackedFiles();
  console.log(`[${owner}] 待上传 ${files.length} 个文件`);

  // 并发上传 blob（8 路），避免串行太慢
  const entries = [];
  // GitHub 的二级限流对「短时间大量写请求」很敏感。8 路并发会在 ~180 个
  // blob 后触发 403；3 路 + 退避重试能稳定跑完，代价只是慢一点。
  const CONCURRENCY = 3;
  for (let i = 0; i < files.length; i += CONCURRENCY) {
    const chunk = files.slice(i, i + CONCURRENCY);
    const results = await Promise.all(chunk.map((f) => uploadBlob(owner, token, f)));
    entries.push(...results);
    process.stdout.write(`\r[${owner}] blob ${Math.min(i + CONCURRENCY, files.length)}/${files.length}`);
  }
  process.stdout.write('\n');

  const tree = await gh(
    `${repoPath}/git/trees`,
    {
      method: 'POST',
      body: JSON.stringify({ base_tree: baseTreeSha, tree: entries }),
    },
    token,
  );

  const headMsg = execSync('git log -1 --pretty=%B', { cwd: process.cwd() }).toString('utf8').trim();
  const commit = await gh(
    `${repoPath}/git/commits`,
    {
      method: 'POST',
      body: JSON.stringify({
        message: headMsg,
        tree: tree.sha,
        parents: [baseCommitSha],
      }),
    },
    token,
  );

  await gh(
    `${repoPath}/git/refs/heads/${BRANCH}`,
    { method: 'PATCH', body: JSON.stringify({ sha: commit.sha, force: false }) },
    token,
  );

  // tag：走 refs API（annotated tag 需要先建 tag 对象）
  try {
    await gh(
      `${repoPath}/git/refs`,
      { method: 'POST', body: JSON.stringify({ ref: 'refs/tags/v0.3.0', sha: commit.sha }) },
      token,
    );
  } catch (e) {
    if (!String(e).includes('422')) throw e;
    console.log(`[${owner}] tag v0.3.0 已存在，跳过`);
  }

  console.log(`[${owner}] 完成 → ${commit.sha.slice(0, 7)}`);
  return commit.sha;
}

for (const repo of REPOS) {
  await syncRepo(repo);
}
