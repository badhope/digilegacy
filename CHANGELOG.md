# Changelog

所有重要变更记录在此。格式参考 Keep a Changelog，版本按平台里程碑推进。

## [0.7.1-m2b] - 2026-11-19

### Added

- 政策库：`policies` 原生表（schema v10、27 表）、`policy_provenance` 独立溯源、152 条平台政策内容源 `content/policies/platforms.json`。
- `core/domains/policy`：`seed_if_empty`、`list`（分类 + 关键词 + 游标）、`get`、`all`、`find_by_name`；种子状态一律 `unverified`，不伪造核实。
- `core/application` 接入 `policies.list` / `policies.get`；`from_env` 打开 Store 后自动 seed，空表才写入。
- HTTP `GET /api/v1/policies`、`GET /api/v1/policies/{id}`；CLI `policies list/get`；MCP 工具由契约生成。
- 交付物：`deliver.export` 真实生成 report/checklist/legal/toolkit 四类 HTML；`jobs`、`deliverable_exports` 落库；`GET /api/v1/exports/{jobId}` 下载，路径 canonical 校验防越界。
- 法律模板内容源 `content/legal/templates.json`，四类模板进入法律文本交付物，并附“需执业律师审阅”提示。
- Web：政策库并入资产二级标签（`/policies`），列表服务端搜索/分类/游标加载更多，详情展示材料、流程、风险、来源与未核实状态。
- Web 交付页接 `deliver.export`：四类生成按钮、结果文件、下载链接、会话语义记录；资产详情可跳转对应平台政策。
- `core/render` 增加可信 body 的可打印页面壳 `render_page`，标题仍转义。

### Changed

- 概览模块把 `policy`、`deliver` 标为 `ready`。
- Web 保持 5 个主入口：政策库不是第 6 个导航，作为资产域二级视图，避免主导航膨胀。
- 契约扩至 15 命令 / 12 工具 / 21 schema / 19 错误码；生成器 domain 集合加入 `policy`。

### Verification

- `./tools/verify.sh` 全绿，日志 `/tmp/digilegacy-verify-m2b.log`。
- Rust：security 49、store 15、domains 9、application 5、server 6、foundation 5、render 2。
- Web：typecheck、6 tests、build、bundle 预算通过；总 gzip **87.25 KB / 110 KB**，入口 raw 48.77 KB / 60 KB，vendor raw 98.88 KB / 130 KB。
- CLI：`policies list --q 支付宝` 命中；`policies get 1` 返回支付宝与 `unverified`；`deliver export --kind report` 生成 HTML。
- HTTP：政策列表/详情返回真实行；`POST /api/v1/deliverables/export` 返回 `jobId`；`GET /api/v1/exports/{jobId}` 返回 `text/html` 附件。
- 浏览器：政策页 152 条、详情可展开、深/亮主题正常；交付页点击生成后出现文件名与下载链接。

### Known limitations

- 政策溯源目前只读：没有“标记已核实/争议”的写命令与审核界面。
- 交付物为同步生成；Job 状态已落库，但未接 SSE/轮询进度与取消。
- Web 写操作仍受 HTTP 写 token 与解锁流程阻塞；资产创建/更新走 CLI `--allow-write`。
- 政策内容按来源整理，尚未逐条人工核对；页面明确显示 `unverified`。

## [0.7.0-m2] - 2026-11-19

### Added

- `core/application` 用例层：HTTP、CLI、MCP 共享同一套 `Application::dispatch`，入口不再各写业务分支。
- `core/domains` 真实领域：资产列表/详情/创建/更新、规划摘要、概览投影、Agent 运行记录、安全摘要。
- 资产领域适配 v9 `digital_assets`：`name/kind/platform/value/note` 映射旧列；平台自动落 `platforms`；空平台使用 `id=0` 占位。
- 触发签到：写入 `check_in_log`、更新 `dms_config.last_check_in`、返回 `nextCheckAt` 并写审计。
- HTTP 读接口：`/api/v1/overview`、`/assets`、`/assets/{id}`、`/plan`、`/security`、`/agent/runs`；写命令仍由 token 门禁拦截。
- CLI 真实子命令：`overview`、`assets list/get/create/update`、`plan`、`security status`、`runs`；写命令要求 `--allow-write`。
- MCP `tools/call` 接真实用例：工具 `_meta["digilegacy/command"]` 映射命令，失败返回 `isError=true` 与平台错误码。
- Web 默认 live：`VITE_USE_MOCK=false`；资产搜索/状态筛选改为服务端查询，游标分页“加载更多”，前端不再重复过滤。
- 安全设置页接 `security.status`：锁定状态、恢复配置、密钥来源、最近备份。
- 架构门禁扩到 13 个 workspace 成员；`core/application` 纳入 core 边界检查。

### Changed

- 平台 API 的 `health` 返回 schema 版本、表数、Store 状态；`mode=m2`。
- 前端状态文案从 M0 骨架切换为 M2 数据在线；活动时间改为本地时间；规划页补空态与触发状态本地化。
- 概览、规划、自动化、设置页的错误标题改用通用数据错误，不再误报“能力未接入”。

### Verification

- `./tools/verify.sh` 通过，日志 `/tmp/digilegacy-verify-m2-final.log`。
- Rust：security 49、store 15、domains 7、application 3、server 6、foundation 5、render 1。
- Web：typecheck、6 tests、build、bundle 预算通过；总 gzip 80.60 KB / 110 KB。
- CLI 实测创建资产返回真实 id；HTTP 实测 overview/assets/plan/security/runs 返回真实行；MCP 实测 `overview.get` 返回真实 envelope。

### Known limitations

- Web 写操作尚未接 HTTP 写 token / 解锁流程；资产创建与更新目前走 CLI 本地特权入口。
- 资产领域暂未接批量导入导出、附件、凭据解密展示。
- 政策库 152 条、交付物、审核、演练、SSE/Job 仍未开始。
- vault 解锁未接 server/Web；`security.status` 会显示锁定，但元数据读链路可读。

## [0.7.0-m1] - 2026-11-19

### Added

- 安全内核迁入 `core/security`：AES-256-GCM、PBKDF2 v1/v2、Argon2id v3、HKDF 域分离、KEK/DEK 两层密钥、v1/v2 字段格式兼容。
- 保险库生命周期：`create_vault`、`unlock`、`unlock_with_recovery`、`lock`、`change_password`；改密只重包 DEK，不重加密全库字段。
- 恢复码封印 RS3/RS2/v1 三代兼容；Shamir 2..10 份、默认 3-of-5、份额校验和/批次指纹、可选口令保护、可打印恢复包 HTML。
- 文件 KEK 托管 `FileKeyring`（Unix 0600），用于服务器/CI 的 SQLCipher 密钥。
- `core/store`：SQLite/SQLCipher 单写连接 + 只读连接、schema v9（26 张表）、`schema_migrations`、additive 迁移、`quick_check`/`integrity_check`。
- 旧 v4/v5 JSON 备份导入：源主密码先验证、v8 表映射到 v9、连接配置合并到 `connections`、replace 前清业务表。
- 认证备份：DB 文件 SHA-256 + backup 域 HMAC；篡改 DB 或清单均拒绝。
- 命名检查点：最多 12 份、带标签/自动标记、认证备份、校验后恢复。
- 整库加密迁移：`sqlcipher_export` 明文→加密、加密→明文，保留 schema 版本。
- 审计链：每行 SHA-256(prev || at || kind || subject || outcome || detail)，`verify_audit_chain` 定位首个断点。
- 诊断包：schema、表计数、完整性、审计链状态；不导出任何数据值。

### Changed

- `core/security` 状态 `m1-security`；`core/store` 状态 `m1-store`。
- 测试基线：security 49、store 15；全仓 `tools/verify.sh` 通过。

### Known limitations

- M1 仅完成数据/安全底座，尚未把 vault/store 生命周期接到 server/CLI/Agent 工具。
- 旧备份导入已覆盖 v4/v5 主要表，尚无 10 份真实旧库样本的端到端回归清单。
- 桌面 DPAPI 密钥托管留到 M3；当前服务器/CI 使用文件 KEK。

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
