# DigiLegacy

数字遗产平台：把资产、继承人、触发链与交付物收进同一套平台能力里。

**定位：API-first 平台。Web 控制台是主客户端，Agent 是平台的一类工具调用者，不是另一套后端。**

- 平台负责：数据、权限、审批、审计、任务、导出与运行态。
- Agent 负责：通过工具读取平台、生成草稿、发起待审批动作。
- Web 负责：让人看见同一份数据、完成审批与编辑。
- 本地部署不是卖点；平台主要接 API，本地只是部署形态之一。

旧版应用不作为新架构的一部分保留。旧路由全部 404，不做兼容重定向。旧源码暂存在 `legacy/`，仅用于 M1 迁移安全内核与领域规则，不参与新构建。

---

## 1. 当前状态

| 项 | 状态 |
|---|---|
| 仓库重组 | 已做完 |
| 契约单一事实源 + 生成器 | 已做完 |
| Rust workspace + 四入口空壳 | 已做完 |
| Web 控制台壳（5 主入口 + 设置工具区） | 已做完 |
| 架构测试 / 性能基线 / CI / verify 脚本 | 已做完 |
| 工程治理文档与模板 | 已做完 |
| 安全内核 / 数据库 | 已做完（M1） |
| 真实领域服务 | 已接入（M2 资产/规划/触发/安全读链路 + 写用例） |
| 政策库 | 已做完（M2-B 152 条 + M3 溯源写；审核工作台待做） |
| 交付物导出 | 已做完（M2-B HTML 报告/清单/法律文本/工具包 + HTTP 下载） |
| Agent Runtime / 审批恢复 | 已做完（M3-E：运行步骤、工具注册表、审批暂停/恢复） |
| Agent 规划 / 取消 / 预算 / 重试 | 已做完（M3-F） |
| 桌面 Tauri 窗口与托盘 | M3/M5 |

当前版本：`0.7.7-m3f`。

M1 已落地：AES-256-GCM / Argon2id / HKDF 两层密钥、恢复码 + Shamir 3-of-5、SQLCipher schema v9、旧备份导入、认证备份/检查点、审计链与诊断包。

M2 已落地：`core/application` 用例层接 M1 Store；HTTP/CLI/MCP 共享同一套领域调用；资产列表/详情/创建/更新、规划、概览、安全摘要、签到、运行记录均读真实表；Web 默认走 Platform API，资产筛选改为服务端查询与游标分页。

M2-B 已落地：政策库 schema v10（27 表）、152 条平台政策与独立 `policy_provenance`、`policies.list/get` 全入口；交付物四类 HTML 导出、`jobs`/`deliverable_exports` 落库、`GET /api/v1/exports/{jobId}` 下载；Web 把政策库并入资产二级标签，交付页可直接生成与下载。

M3 已落地六段：政策溯源写、平台写令牌与资产新增/编辑、审计链查询、Agent 审批队列、确定性 Agent Runtime，以及 M3-F 的规划器/取消/预算/重试。规划器接 OpenAI 兼容接口（默认 DeepSeek），只生成注册工具步骤；人工确认后再启动，写操作仍进审批。取消会把待审批置为 `expired`，重试会创建新运行并保留旧记录。

契约规模：**23 个命令、16 个工具、30 个 schema、19 个错误码**。

---

## 2. 架构

```text
Web Console        Agent / MCP        CLI             Desktop
     │                 │               │                │
     └────────────── Platform API / CommandBus ─────────┘
                              │
                    ┌─────────┴─────────┐
                domains             agent runtime
                    │                   │
              store / security    approvals / audit
                    │
             integrations (mail, cloud, LLM, ...)
```

### 核心约束

1. **契约先行**：命令、工具、错误、schema 只写在 `contracts/`，由 `tools/generate.mjs` 派生到 Web、Rust、CLI、HTTP、MCP、mock 与 OpenAPI。
2. **一个 CommandBus**：Web、CLI、HTTP、MCP 不各写一套业务逻辑，最终都落到同一命令分发。
3. **权限分档**：`read` / `draft` / `write` / `key` / `network` / `desktop`。
   - HTTP：read/draft 开放；write 需写权限 token；key/network/desktop 永远 403。
   - MCP：只暴露 read/draft。
   - write 工具默认 `approval=required`，Agent 不能自批。
4. **模块化单体**：M0-M2 不拆微服务。先保证一个进程内的领域边界、契约与测试，再按真实热点拆。
5. **前端按入口拆包**：路由懒加载，骨架屏优先，避免全页 spinner 与大 bundle。

---

## 3. 仓库结构

```text
core/
  foundation/      信封、错误、权限、CommandBus、trace id
  contracts/       生成后的命令/错误表
  security/        Vault 生命周期、恢复、备份、文件 KEK
  store/           SQLCipher、schema v9、迁移、检查点、审计
  domains/         DOM-01..07 领域清单
  integrations/    邮件、云盘、LLM 等连接边界
  agent/           Run / Approval 状态机
  render/          纯函数文档渲染
apps/
  server/          axum Platform API
  cli/             clap 本地入口
  mcp/             stdio MCP server
  desktop/         桌面空壳（M3/M5 接 Tauri）
web/               Vue 3 + Vite 控制台
contracts/         commands/tools/errors/schemas/mocks/openapi
content/           i18n 文案
tools/             生成器、架构门禁、性能基线、verify
docs/              定位、架构、验收、ADR、RFC、v1.2 计划
legacy/            旧应用迁移参考，不参与新构建
```

### 五个主入口

- `/overview` 概览
- `/assets` 资产
- `/planning` 规划
- `/deliverables` 交付
- `/automation` 自动化

设置是右上角全局工具区，不占业务导航。旧路由 `/dashboard`、`/discover`、`/profile`、`/dms`、`/legal`、`/report`、`/heir`、`/review` 全部 404。

---

## 4. 快速启动

### 生成契约

```bash
node tools/generate.mjs
node tools/generate.mjs --check
```

### 启动平台 API

```bash
source ~/.cargo/env
DIGILEGACY_BIND=127.0.0.1:8787 DIGILEGACY_API_TOKEN=<write-token> cargo run -p digilegacy-server
```

`write` 命令要求 `Authorization: Bearer <write-token>`；`draft` 命令与读接口不要求。未设置 token 时 `write` 一律 401，不会假成功。

```bash
curl http://127.0.0.1:8787/api/v1/health
```

### 启动 Web 控制台

```bash
cd web
npm install
npm run dev
# http://127.0.0.1:5173
```

Web 默认走真实 API；需要无后端演示时设置 `VITE_USE_MOCK=true`。写操作先在 设置 → 连接 配置平台写令牌。

### CLI / MCP

```bash
cargo run -p digilegacy-cli -- commands --json
cargo run -p digilegacy-cli -- health
cargo run -p digilegacy-cli -- invoke assets.list --args '{}'

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | cargo run -p digilegacy-mcp
```

---

## 5. 验证

```bash
./tools/verify.sh
```

包含：

- 契约生成一致性
- 架构门禁
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Web `typecheck` / `vitest` / `build`
- Web bundle 预算

单独命令：

```bash
node tools/arch-check.mjs
node tools/bench/web-budget.mjs
./tools/bench/core-bus.sh
```

M3 基线：

- Rust 测试 109 项：security 49、store 15、domains 13、application 10、agent 3、server 12、foundation 5、render 2（全仓 verify 通过）
- Web 总 gzip **104.15 KB / 110 KB**，CSS **18.12 KB / 19 KB**
- 入口 chunk **62.82 KB raw / 64 KB**，vendor **98.88 KB raw / 130 KB**
- CommandBus 约 **250 ns/op**（本机 release 基线，M2 测得）

---

## 6. 文档

- [平台定位](docs/POSITIONING.md)
- [架构说明](docs/ARCHITECTURE.md)
- [验收与门禁](docs/VERIFICATION.md)
- [DoR / DoD 与协作规则](docs/GOVERNANCE.md)
- [ADR](docs/adr/)
- [RFC 模板](docs/rfc/0001-template.md)
- [v1.2 工程计划](docs/plan/README.md)

计划口径：**54 个模块、80.5 人日 ≈ 16.1 周、26 张表、G01–G19 十九道门禁**。M3/M4 可并行，目标约 14.4 周。

---

## 7. 许可证

MIT。见 [LICENSE](LICENSE)。
