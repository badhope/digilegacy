#!/usr/bin/env node
// 本地 Web 服务启动器：把已构建的 dist 交给 digilegacy-server 托管。
//
// 为什么不直接用 `vite preview`：preview 只发静态文件，没有 /api/invoke，
// 前端会退回「未连接本地后端」的错误页。这个脚本启动的是**真的后端**，
// 数据落在本地 SQLite，与桌面版同一个库。
//
// 用法：
//   npm run build:web            # 生成 dist（生产构建 → 走 HTTP 后端）
//   npm run serve:web            # 启动服务（默认 http://127.0.0.1:8787）
//   npm run serve:web -- --port 9000 --allow-write
//
// 二进制查找顺序：
//   1. $DIGILEGACY_SERVER_BIN
//   2. src-tauri/target/release/digilegacy-server(.exe)
//   3. src-tauri/target/debug/digilegacy-server(.exe)
//   4. PATH 上的 digilegacy-server
// 找不到就明确报错并给出构建命令，不静默降级成「只发静态文件」。

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '..');
const isWindows = process.platform === 'win32';
const exe = isWindows ? 'digilegacy-server.exe' : 'digilegacy-server';

const candidates = [
  process.env.DIGILEGACY_SERVER_BIN,
  join(root, 'src-tauri', 'target', 'release', exe),
  join(root, 'src-tauri', 'target', 'debug', exe),
].filter(Boolean);

const bin = candidates.find((p) => existsSync(p));

if (!bin) {
  console.error(
    [
      '找不到 digilegacy-server 可执行文件。',
      '',
      '先构建它：',
      '  cargo build --release --bin digilegacy-server --manifest-path src-tauri/Cargo.toml',
      '',
      '或用环境变量指定：',
      '  DIGILEGACY_SERVER_BIN=/path/to/digilegacy-server npm run serve:web',
    ].join('\n')
  );
  process.exit(1);
}

const dist = join(root, 'dist');
if (!existsSync(join(dist, 'index.html'))) {
  console.error('找不到 dist/index.html。先运行：npm run build:web');
  process.exit(1);
}

const passthrough = process.argv.slice(2);
const hasWebDir = passthrough.includes('--web-dir');
const args = hasWebDir ? passthrough : ['--web-dir', dist, ...passthrough];

console.log(`启动 ${bin}`);
console.log(`前端目录 ${dist}`);
console.log('提示：默认只读；写操作需显式加 --allow-write');

const child = spawn(bin, args, { stdio: 'inherit' });

for (const sig of ['SIGINT', 'SIGTERM']) {
  process.on(sig, () => child.kill(sig));
}

child.on('exit', (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  else process.exit(code ?? 0);
});
