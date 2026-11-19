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

M0 只实现壳与边界；领域服务在 M2 起接入，安全内核在 M1 迁入。

## 2. Workspace 成员

### core（8）

| crate | 职责 | M0 状态 |
|---|---|---|
| `core/foundation` | Envelope、错误、权限、CommandBus、trace id | 已实现 |
| `core/contracts` | 生成后的命令/错误表 | 已实现 |
| `core/security` | Vault/密钥/恢复边界 | 边界 |
| `core/store` | SQLite/SQLCipher、迁移、仓储边界 | 边界 |
| `core/domains` | DOM-01..07 领域清单 | 清单 |
| `core/integrations` | 连接类型/状态 | 枚举 |
| `core/agent` | Run/Approval 状态机 | 枚举 |
| `core/render` | 纯函数文档渲染 | 已实现 |

### apps（4）

| crate | 职责 | M0 状态 |
|---|---|---|
| `apps/server` | axum Platform API | health/commands/overview/invoke |
| `apps/cli` | 本地命令入口 | commands/health/invoke |
| `apps/mcp` | stdio JSON-RPC | initialize/tools list/resources |
| `apps/desktop` | 桌面入口 | JSON 空壳，Tauri 留到 M3/M5 |

## 3. 依赖方向

硬规则：

```text
apps/*  →  core/*
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

M0 未实现写权限 token，因此 HTTP `write` 返回 `401 AUTH_REQUIRED`，不是假成功。

## 7. 安全与数据边界

- **M0**：`core/security`、`core/store` 只有边界；Web 使用 mock；不声称已加密。
- **M1**：从 `legacy/app/src-tauri` 按模块迁入加密、Vault、恢复、备份、DB，不重写算法。
- **数据归属**：单工作区抽象保留 `workspaces`，26 张表按领域拆；资产只维护一份事实。
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

- Web 总 gzip 约 81 KB；入口约 48 KB；vendor 98.5 KB raw。
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
