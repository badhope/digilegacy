#!/usr/bin/env bash
# 本地门禁：与 CI 完全一致的检查序列，任一失败立即退出非零。
# 用法：bash scripts/verify.sh [--rust-only|--web-only]
#
# 关键约定：不使用 `cmd | tail` 这种写法判断成败——管道的退出码取自最后一个命令，
# 会把失败掩盖成 0（本项目曾因此误判「全绿」）。这里统一显式记录退出码。

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB="$ROOT/digilegacy"
TAURI="$WEB/src-tauri"
MODE="${1:-all}"
FAILED=0

run() {
  local label="$1"; shift
  printf '\n==> %s\n' "$label"
  if "$@"; then
    printf '  PASS: %s\n' "$label"
  else
    local code=$?
    printf '  FAIL(%d): %s\n' "$code" "$label"
    FAILED=1
  fi
}

# 可选检查：失败只告警，不计入 FAILED。
# 理由：权威来源核验依赖外部政务站点，网络抖动或对端限流不应把本地门禁判红——
# 否则「可选」二字就是假的，用户会习惯性忽略整条门禁。
run_optional() {
  local label="$1"; shift
  printf '
==> %s
' "$label"
  if "$@"; then
    printf '  PASS: %s
' "$label"
  else
    printf '  WARN: %s（可选检查失败，不计入门禁）
' "$label"
  fi
}

have() { command -v "$1" >/dev/null 2>&1; }

if [ "$MODE" != "--rust-only" ]; then
  echo "################ 前端 ################"
  # 用 npm install 而非 npm ci：后者会先删光 node_modules，
  # 在 Windows 上若仍有进程占用文件会中途失败并留下残缺目录（曾多次踩坑）。
  # CI 里仍用 npm ci（干净环境），本地校验用 install 更稳。
  run "依赖就绪" bash -c "cd '$WEB' && npm install --silent --no-audit --no-fund"
  run "类型检查" bash -c "cd '$WEB' && npx vue-tsc --noEmit"
  run "单元测试 + 覆盖率门禁" bash -c "cd '$WEB' && npm run test:coverage"
  run "生产构建" bash -c "cd '$WEB' && npm run build"
  run "演示构建（VITE_DEMO=1）" bash -c "cd '$WEB' && npm run build:demo"
  run "心跳服务器行为测试" bash -c "cd '$WEB' && node server/heartbeat-server.test.mjs"
  run "文档数字一致性" bash -c "cd '$WEB' && node scripts/check-doc-facts.mjs"
  run "政策库溯源" bash -c "cd '$WEB' && node scripts/check-policy-provenance.mjs"
  run_optional "权威来源核验（可选，需联网）" bash -c "cd '$WEB' && node scripts/verify-authorities.mjs"
  # E2E 默认跳过（需先 npx playwright install chromium）；--with-e2e 显式开启
  if [ "$MODE" = "--with-e2e" ]; then
    run "E2E（Playwright）" bash -c "cd '$WEB' && npx playwright test"
  fi
fi

if [ "$MODE" != "--web-only" ]; then
  echo "################ Rust ################"
  CARGO="cargo"
  have cargo || { [ -x "$HOME/.cargo/bin/cargo" ] && CARGO="$HOME/.cargo/bin/cargo"; }
  run "格式检查" bash -c "cd '$TAURI' && '$CARGO' fmt --check"
  run "Clippy（-D warnings）" bash -c "cd '$TAURI' && '$CARGO' clippy --all-targets -- -D warnings"
  run "测试" bash -c "cd '$TAURI' && '$CARGO' test --locked"
fi

echo
if [ "$FAILED" -ne 0 ]; then
  echo "结果：FAILED"
  exit 1
fi
echo "结果：ALL GREEN"
