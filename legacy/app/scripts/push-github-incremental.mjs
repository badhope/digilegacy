#!/usr/bin/env node
// 增量同步：只上传指定 tree 差异的文件，然后建 commit 更新 ref。
//
// 为什么需要它：全量上传 267 个 blob 在本机到 GitHub 的不稳定链路上
// 几乎必失败（实测反复卡在 210/267 附近并重试到超时）。而本轮实际
// 只改了 8 个文件——按 diff 上传能把请求数从 267+3 降到 8+3。
//
// 用法：
//   BASE_TREE=<远端当前 tree sha> OWNER=<账号> node scripts/push-github-incremental.mjs

import { execSync } from 'node:child_process';

const OWNER = process.env.OWNER;
const BASE_TREE = process.env.BASE_TREE;
const TOKEN = process.env[`GITHUB_TOKEN_${String(OWNER).toUpperCase()}`];

if (!OWNER || !BASE_TREE || !TOKEN) {
  console.error('需要 OWNER / BASE_TREE / GITHUB_TOKEN_<OWNER>');
  process.exit(1);
}

const API = 'https://api.github.com';
const REPO = `/repos/${OWNER}/digilegacy`;

async function gh(pathname, init = {}, attempt = 0) {
  try {
    const res = await fetch(`${API}${pathname}`, {
      ...init,
      headers: {
        Authorization: `Bearer ${TOKEN}`,
        Accept: 'application/vnd.github+json',
        'Content-Type': 'application/json',
        'X-GitHub-Api-Version': '2022-11-28',
      },
    });
    const text = await res.text();
    if (!res.ok) {
      if ((res.status === 400 || res.status === 401 || res.status === 403 || res.status === 429 || res.status >= 500) && attempt < 6) {
        const wait = Math.min(2000 * 2 ** attempt, 30000);
        console.log(`  ${res.status} 重试(${attempt + 1}) ${wait}ms`);
        await new Promise((r) => setTimeout(r, wait));
        return gh(pathname, init, attempt + 1);
      }
      throw new Error(`${init.method ?? 'GET'} ${pathname} -> ${res.status}: ${text.slice(0, 200)}`);
    }
    return text ? JSON.parse(text) : null;
  } catch (e) {
    if (e instanceof TypeError && attempt < 6) {
      const wait = Math.min(2000 * 2 ** attempt, 30000);
      console.log(`  网络错误重试(${attempt + 1}) ${wait}ms`);
      await new Promise((r) => setTimeout(r, wait));
      return gh(pathname, init, attempt + 1);
    }
    throw e;
  }
}

// 1) 找出与远端 tree 的差异文件（用本地 git 判定，不手写列表）
const changed = execSync(`git diff --name-only ${BASE_TREE} HEAD`, { cwd: process.cwd() })
  .toString('utf8')
  .split('\n')
  .filter(Boolean);

console.log(`[${OWNER}] 需要同步 ${changed.length} 个文件`);

// 2) 逐个上传 blob（串行：这链路经不起并发）
//
// 内容必须来自 **git 对象库**（`git cat-file blob <rev>:<path>`），不能读工作区文件。
// 原因：本仓库 .gitattributes 让文本以 LF 入库，而 Windows 工作区检出是 CRLF。
// 读工作区会把每个文本文件都变成「与远端不同」的假差异——历史上两次踩中，
// 表现为远端内容正确但 tree 哈希与本地不一致，排查极费时间。
const tree = [];
for (const f of changed) {
  const buf = execSync(`git cat-file blob HEAD:${f}`, {
    cwd: process.cwd(),
    maxBuffer: 64 * 1024 * 1024,
  });
  const blob = await gh(`${REPO}/git/blobs`, {
    method: 'POST',
    body: JSON.stringify({ content: buf.toString('base64'), encoding: 'base64' }),
  });
  tree.push({ path: f.replace(/\\/g, '/'), mode: '100644', type: 'blob', sha: blob.sha });
  console.log(`  ✓ ${f}`);
}

// 3) 建 tree / commit / 更新 ref
const newTree = await gh(`${REPO}/git/trees`, {
  method: 'POST',
  body: JSON.stringify({ base_tree: BASE_TREE, tree }),
});

const headRef = await gh(`${REPO}/git/ref/heads/main`);
const msg = execSync('git log -1 --pretty=%B', { cwd: process.cwd() }).toString('utf8').trim();

const commit = await gh(`${REPO}/git/commits`, {
  method: 'POST',
  body: JSON.stringify({ message: msg, tree: newTree.sha, parents: [headRef.object.sha] }),
});

await gh(`${REPO}/git/refs/heads/main`, {
  method: 'PATCH',
  body: JSON.stringify({ sha: commit.sha }),
});

console.log(`[${OWNER}] 完成 → tree ${newTree.sha.slice(0, 8)} / commit ${commit.sha.slice(0, 7)}`);
