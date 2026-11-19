# 架构说明

## 1. 总体形态

**模块化单体（modular monolith）**，不是微服务。

```text
┌───────────────┐  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐
│ Web Console   │  │ Builtin Agent │  │ MCP / External│  │ CLI / Desktop │
└───────┬───────┘  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘
        │                  │                  │                  │
        └──────────────────┴──────── Platform API ───────────────┘
                                      │
                              ┌───────┴───────┐
                              │  CommandBus   │  权限 / 审批 / 审计
                              └───────┬───────┘
                                      │
        ┌───────────────┬─────────────┼─────────────┬───────────────┐
        │ domains       │ agent       │ store       │ integrations  │
        │ assets/plan   │ runs/approve│ SQLite      │ mail/cloud/LLM│
        │ deliver/...   │ audit       │ SQLCipher   │               │
        └───────────────┴─────────────┴─────────────┴───────────────┘
```

M0 完成壳与边界；M1 完成安全/Store；M2 以 `core/application` 接真实领域，Web 默认走 Platform API；M2-B 加入政策库与交付物导出；M3 加入政策溯源写、写令牌、审计查询、Agent 审批队列、确定性 Runtime 与任务规划/取消/预算/重试。

## 2. Workspace 成员

### core（9）

| crate | 职责 | M2 状态 |
|---|---|---|
| `core/foundation` | Envelope、错误、权限、CommandBus、trace id | 已实现 |
| `core/contracts` | 生成后的命令/错误表 | 已实现 |
| `core/security` | Vault/密钥/恢复/备份/审计 | 已实现（M1） |
| `core/store` | SQLite/SQLCipher、迁移、导入、检查点 | 已实现（M1） |
| `core/domains` | assets/plan/overview/runs/policy/audit/approvals 真实领域 | 已实现（M2 首批 + M2-B 政策库 + M3 溯源写/审计/审批/运行详情） |
| `core/agent` | 规划器与 Agent 状态词汇 | 已实现（M3-F OpenAI 兼容规划器） |
| `core/application` | 用例层：共享 HTTP/CLI/MCP 的业务分派、审计、交付导出 | 已实现（M2/M2-B） |
| `core/integrations` | 连接类型/状态 | 枚举 |
| `core/agent` | Run/Approval 状态机 | 枚举 |
| `core/render` | 纯函数文档渲染 | 已实现 |

### apps（4）

| crate | 职责 | M2 状态 |
|---|---|---|
| `apps/server` | axum Platform API | health/commands + 真实读接口；`write` 命令 Bearer token 门禁；`draft` 按契约开放；`audit.list` 读哈希链 |
| `apps/cli` | 本地命令入口 | 真实 overview/assets/plan/security/runs；写入 `--allow-write` |
| `apps/mcp` | stdio JSON-RPC | initialize/tools list；tools/call 接真实只读用例 |
| `apps/desktop` | 桌面入口 | JSON 空壳，Tauri 留到 M3/M5 |

## 3. 依赖方向

硬规则：

```text
apps/*  →  core/application  →  core/domains / core/store / core/security
core/*  →  core/foundation, core/contracts
core/*  ✗→  apps/*
apps/*  ✗→  apps/*（除非明确的基础入口共享）
```

- 入口层不写业务规则。
- 领域 crate 不依赖 UI/HTTP/CLI。
- 生成代码不作为业务依赖的“第二事实源”。
- `tools/arch-check.mjs` 会检查 core 不依赖 apps、旧路径不进入新构建。

## 4. 契约管线

唯一事实源：

```text
contracts/commands.json
contracts/tools.json
contracts/errors.json
contracts/schemas/*.json
contracts/mocks.json
```

`tools/generate.mjs` 派生：

1. `web/src/platform/generated/ipc.ts`
2. `web/src/platform/generated/mock.ts`
3. `web/src/platform/generated/tools.ts`
4. `core/contracts/src/generated.rs`
5. `core/contracts/src/errors.rs`
6. `apps/cli/src/generated.rs`
7. `apps/server/src/generated.rs`
8. `apps/mcp/tools.generated.json`
9. `contracts/generated/tool-manifest.json`
10. `contracts/openapi.json`

规则：

- 命令名、工具名、错误码、schema 路径必须可解析。
- 工具只能引用已有命令。
- 写工具必须 `approval=required`。
- MCP 只暴露 `read`/`draft`。
- HTTP 传输必须是 `METHOD /api/v1/...`。
- 生成文件不手改；`--check` 失败即契约漂移。

## 5. 信封与错误

统一返回：

```json
{
  "success": true,
  "code": "OK",
  "message": "ok",
  "data": {},
  "details": null,
  "traceId": "tr_..."
}
```

失败：

```json
{
  "success": false,
  "code": "AUTH_REQUIRED",
  "message": "需要写权限 token",
  "data": null,
  "details": null,
  "traceId": "tr_..."
}
```

错误码来自 `contracts/errors.json`，HTTP 状态、CLI 退出码、用户文案键由同一张表派生。

## 6. 权限矩阵

| 权限 | 含义 | HTTP | CLI | MCP | Agent |
|---|---|---|---|---|---|
| `read` | 读平台数据 | 允许 | 允许 | 允许 | 允许 |
| `draft` | 生成草稿/预览，不落最终态 | 允许 | 允许 | 允许 | 允许 |
| `write` | 改业务数据 | 需写权限 token | 允许 `--allow-write` | 禁止 | 需审批 |
| `key` | 主密码/恢复/库密钥 | 永远 403 | 本地允许 | 禁止 | 禁止 |
| `network` | 出站请求 | 永远 403 | 允许 | 禁止 | 禁止 |
| `desktop` | 窗口/托盘/自启 | 不支持 | 不支持 | 禁止 | 禁止 |

HTTP `write` 已接 `DIGILEGACY_API_TOKEN`：请求必须带 `Authorization: Bearer <token>`，比较为常量时间；未配置或令牌错误返回 `401 AUTH_REQUIRED`。`draft` 命令（政策溯源、交付导出）按契约开放，不要求 token。Web 在 设置 → 连接 中把令牌存 `sessionStorage`，只对 `write` 命令附加，不写本地持久化；CLI 的 `write` 仍用 `--allow-write`，桌面/本地进程不依赖 HTTP token。

## 7. 安全与数据边界

- **M0**：`core/security`、`core/store` 只有边界；Web 使用 mock；不声称已加密。
- **M1**：从 `legacy/app/src-tauri` 按模块迁入加密、Vault、恢复、备份、DB，不重写算法。
- **M2**：`core/application` 统一用例分派；入口只做权限/传输；Web 读真实 API，资产筛选服务端化。
- **M2-B**：schema v10（27 表）新增 `policies`/`policy_provenance`；政策库与交付导出走同一 `Application`；`deliver.export` 写 `jobs`/`deliverable_exports`，下载路由做路径边界校验。
- **M3 第一切片**：`policies.provenance.set` 允许 `unverified/source_cited/verified/disputed` 四态，来源为空回退官方 URL，写 `policy_provenance` 并追加审计 `policy.provenance.set`；HTTP `PATCH /api/v1/policies/{id}/provenance`，CLI `policies provenance`，Web 在详情内编辑。
- **M3-B**：HTTP `write` Bearer token（`DIGILEGACY_API_TOKEN`）；Web 连接设置存 `sessionStorage` 并只对 `write` 命令附加；资产新增/编辑表单接 `assets.create/update`，写后刷新列表并留审计。
- **M3-C**：`audit.list` 读审计链（游标 + 类型过滤），HTTP `GET /api/v1/audit`，CLI `audit list`，Web 设置审计页显示时间/类型/对象/结果/hash 前缀。
- **M3-D**：`agent.approvals.list/decide` 管理 Agent 写操作审批队列；决定更新 `agent_approvals` 状态并写 `agent.approval.decide` 审计；HTTP `GET /api/v1/agent/approvals`、`POST /api/v1/agent/approvals/{id}/decide`，CLI `approvals list/decide`，Web `/automation/approvals`。
- **M3-E**：`agent.runs.start/get` 驱动确定性 Runtime：按步骤执行注册工具，只读/草稿立即执行，`write` 工具创建审批并暂停；批准后从当前步骤继续执行，拒绝则终止。运行步骤落 `agent_run_log.steps_json`，Web `/automation/runs/new`、`/automation/runs/:id`、`/automation/tools` 接真实数据。
- **M3-F**：`agent.runs.plan` 用 OpenAI 兼容接口（默认 DeepSeek，`DIGILEGACY_LLM_BASE_URL/API_KEY/MODEL`）把任务拆成注册工具步骤，只返回计划、不执行；Web 人工确认后再 `start`。`agent.runs.cancel` 终止未终态运行并把待审批置 `expired`；`agent.runs.retry` 为 failed/rejected/cancelled 运行创建新运行并保留旧记录；`maxSteps/maxWriteSteps` 在启动前做静态预算校验。
- **数据归属**：单工作区抽象保留 `workspaces`，27 张表按领域拆；资产与政策各维护一份事实。
- **审计**：写操作、Agent 调用、审批决定必须可追溯；`audit=true` 的命令不允许静默执行。
- **密钥**：密钥不进日志、不进前端、不进 MCP tool 结果；`security.status` 只返回摘要。

## 8. 前端架构

- Vue 3 + Vite + TypeScript。
- 路由懒加载；5 个主入口 + 设置工具区。
- `platform/client.ts` 是唯一 API/mock 边界；页面不直接拼 URL。
- `useResource` 负责加载/错误/竞态序号；M0 不引入 TanStack Query。
- 设计系统用 token（暖纸/苔绿/陶土），亮暗双主题。
- 首屏用骨架屏，不用全页 spinner；M0 路由 chunk 均小于 24 KB raw。

## 9. 性能与加载

已设预算：

- Web 总 gzip 约 87 KB；入口约 49 KB；vendor 98.9 KB raw。
- CommandBus 约 250 ns/op（M0 release 基线）。
- 路由级拆包；页面级 CSS 随路由加载。
- 长任务走 Job/SSE，不让 HTTP 请求阻塞（M2+）。
- 性能回归超过 20% 必须解释或回滚（G13）。

## 10. 关键决策

- [ADR-0001 契约先行](adr/0001-contract-first.md)
- [ADR-0002 模块化单体](adr/0002-modular-monolith.md)
- [ADR-0003 Web 主客户端、Agent 工具化](adr/0003-web-primary-agent-tools.md)
- [ADR-0004 旧版不进入新包](adr/0004-legacy-not-in-new-build.md)

## 11. M0 未完成边界

- 领域 handler 未接；已知读命令返回 `UNSUPPORTED`。
- 安全内核、SQLite/SQLCipher 未接。
- 桌面 Tauri 未接。
- G03 目前只有 `vue-tsc`，ESLint/Prettier 未纳入；G04/G05 覆盖率未纳入；G08-G14 尚未形成完整流水线。
- `legacy/` 源码仍在，作为 M1 迁移参考，不参与构建。
