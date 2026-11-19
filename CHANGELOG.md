# Changelog

所有重要变更记录在此。格式参考 Keep a Changelog，版本按平台里程碑推进。

## [0.7.0-m0] - 2026-11-19

### Added

- 新仓库骨架：`core/` 8 个 crate、`apps/` 4 个入口、`web/`、`contracts/`、`content/`、`tools/`、`docs/`。
- 契约单一事实源：13 个命令、10 个工具、18 个 schema、19 个错误码。
- `tools/generate.mjs`：生成 Web IPC/mock/tools、Rust 命令表/错误表、CLI/Server/MCP manifest、OpenAPI。
- Rust `CommandBus`、Envelope、错误映射、权限分档、trace id。
- axum Platform API：health、commands、overview、invoke；写入在 M0 返回 `AUTH_REQUIRED`，network/key/desktop 返回 `FORBIDDEN`。
- clap CLI：`commands --json`、`health`、`invoke`、生成式 help。
- stdio MCP：initialize、ping、tools/list、resources/list；`tools/call` 返回真实 tool error，不伪造成功。
- Vue 3 + Vite 控制台：概览、资产、规划、交付、自动化、设置、详情/空壳、404。
- Agent 抽屉、命令面板 `⌘K`、锁定层、亮暗主题、中英文案、mock 数据、骨架/错误/空态。
- `tools/arch-check.mjs`、`tools/bench/web-budget.mjs`、`tools/bench/core-bus.sh`、`tools/verify.sh`。
- GitHub Actions CI：contracts、rust、web 三组门禁。
- 工程治理文档：定位、架构、验收、DoR/DoD、ADR、RFC 模板、v1.2 计划。

### Changed

- 旧应用整体移入 `legacy/`，退出新构建；旧路由全部 404，不做兼容重定向。
- 平台定位明确为 API-first，Web 为主客户端，Agent 工具一等公民。
- HTTP 写权限口径改为“需要写权限 token”；M0 未接 token，因此写命令保持关闭。
- CLI 移除与 clap 内置 `help` 冲突的显式子命令。

### Known limitations

- M0 领域命令仍返回 `UNSUPPORTED`；真实数据在 M1/M2 接入。
- 安全内核与 SQLite/SQLCipher 尚未迁移，`core/security`、`core/store` 只有边界。
- 桌面 Tauri 窗口/托盘留到 M3/M5。
- `legacy/` 源码尚未删除，仅作迁移参考。
