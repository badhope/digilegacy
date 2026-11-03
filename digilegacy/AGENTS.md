# AGENTS.md — DigiLegacy 归零

数字遗产管理桌面应用。**本地优先 / 无服务器 / 无遥测**，数据全部在本机加密（AES-256-GCM）。Tauri 2 + Vue 3 + Rust + SQLite。

## 结构

- `digilegacy/` — 前端（Vue 3 + Vite + TS）
  - `digilegacy/src/` — 视图 / 组件 / stores / api
  - `digilegacy/src-tauri/` — Rust 后端（crypto / db / backup / heartbeat / commands）
  - `digilegacy/e2e/` — Playwright
- `docs/` — 站点与 `VERIFICATION.md`（验证证据表）
- `scripts/verify.sh` — 本地门禁，**与 CI 完全一致**

## 启动

```bash
cd digilegacy && npm install
npm run dev          # 浏览器预览
npm run tauri:dev    # 桌面壳
```

## 验证（改完必须跑）

```bash
bash scripts/verify.sh --with-e2e   # 全量：install → typecheck → coverage → build → demo build → 心跳服务器 → e2e → fmt → clippy → cargo test
bash scripts/verify.sh --web-only   # 只改前端时
bash scripts/verify.sh --rust-only  # 只改 Rust 时
```

只跑单项：`npm run typecheck` / `npm run test:coverage` / `npx playwright test` / `cargo test --locked` / `cargo clippy --all-targets -- -D warnings`。

**覆盖率阈值是硬门禁**（statements 78 / branches 78 / functions 71 / lines 79），新增代码若拉低覆盖率会失败。

## 硬约束

- 加密相关代码（`src-tauri/src/crypto.rs`）改动等于动安全边界：必须走 `deep` 审计，且不得降低密钥派生强度
- 数据库 schema 变更要配迁移，不能直接删表/改列
- **禁止引入任何网络上报**：本项目承诺 no-telemetry / no-server，加任何 HTTP 上报都是破坏产品契约
- `scripts/verify.sh` 的历史教训：不要用 `cmd | tail` 判断成败（管道退出码取最后一个命令，会把失败掩盖成 0）
- 数据文件路径走 `src-tauri/src/platform_data.rs`，不要硬编码

## 不要碰

- `node_modules/`、`dist/`、`src-tauri/target/`、`coverage/`、`test-results/`
- `**/src-tauri/data/`、`*.db*`（用户真实数据）
- 已发布的版本号（改版本要走 release 流程）
