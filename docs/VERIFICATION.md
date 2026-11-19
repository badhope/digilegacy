# 验收与门禁

## 1. 门禁总表

计划口径为 G01–G19。M0 是工程壳阶段，只强制其中一部分；未纳入的项在 M1/M2 逐步补齐，不能把“M0 通过”误读成“全部门禁已上线”。

| 编号 | 门禁 | M0 状态 | 实现位置 |
|---|---|---|---|
| G01 | `cargo fmt --check` | 已接入 | `tools/verify.sh` |
| G02 | `cargo clippy -D warnings` | 已接入 | `tools/verify.sh` |
| G03 | 前端类型/风格 | 部分：`vue-tsc` 已接入；ESLint/Prettier 未接入 | `web` scripts |
| G04 | Rust 单测/集成 + 覆盖率 | 部分：单测已接入；覆盖率未接入 | `cargo test` |
| G05 | 前端单测 + 覆盖率 | 部分：vitest 已接入；覆盖率未接入 | `web/src/**/*.test.ts` |
| G06 | 契约检查（生成物 + OpenAPI） | 已接入生成一致性；OpenAPI 已派生 | `tools/generate.mjs --check`、`arch-check` |
| G07 | 生产构建 + 无 mock 分块 | 部分：build 已接入；mock 分块未强制排除 | `web build` |
| G08 | 文档事实一致性 | 未接入 | 计划 M2 |
| G09 | 政策溯源棘轮 | 未接入 | 计划 M2 |
| G10 | 依赖漏洞（npm/cargo） | 未接入 | 计划 M1 |
| G11 | 密钥扫描 | 未接入 | 计划 M1 |
| G12 | E2E 主路径 + 可访问性 | 未接入 | 计划 M2/M3 |
| G13 | 性能冒烟（回归 >20%） | 部分：bundle 预算 + bus 基线 | `tools/bench/*` |
| G14 | 视觉回归（关键 6 页） | 未接入 | 计划 M2 |
| G15 | Agent 工具权限与审批 | 已接入 | 生成器 + `arch-check` + MCP 过滤 |
| G16 | 页面→命令映射 | 部分：路由 meta 已声明；未做自动校验 | `web/src/app/router.ts` |
| G17 | 加载预算与首屏 | 部分：bundle 预算；CLS/长任务未测 | `tools/bench/web-budget.mjs` |
| G18 | 模块边界/架构测试 | 已接入 | `tools/arch-check.mjs` |
| G19 | ADR/变更单 | 已建立流程与模板 | `docs/adr/`、`docs/GOVERNANCE.md` |

## 2. 一键验证

```bash
./tools/verify.sh
```

步骤：

1. `node tools/generate.mjs --check`
2. `node tools/arch-check.mjs`
3. `cargo fmt --all -- --check`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `cargo test --workspace`
6. Web `npm run typecheck`
7. Web `npm test`
8. Web `npm run build`
9. `node tools/bench/web-budget.mjs`

## 3. M0 已验证结果

### 契约

```text
contract check: 13 commands, 10 tools, 18 schemas, 19 error codes
arch-check: OK
- workspace members: 12
- core crates: 8; app crates: 4
- legacy route gate: 8 routes unresolved by catch-all
- generated artifacts: 10
```

### Rust

```text
cargo fmt --all -- --check        OK
cargo clippy --workspace --all-targets -- -D warnings  OK
cargo test --workspace            OK
```

已观察测试（M1）：

- foundation：5 个
- render：1 个
- server：5 个
- security：49 个（crypto/vault/recovery/Shamir/backup/session/keyring）
- store：15 个（schema v9、SQLCipher、迁移、旧备份导入、检查点、审计、诊断）
- 其余 crate：0 个（尚未接领域的边界）

### M1 安全与数据底座

```text
core/security:
  crypto v1/v2/v3 KDF、AES-256-GCM、HKDF 三域、KEK/DEK、恢复码三代封印
  Shamir 2-of-N / 3-of-5、校验和、批次指纹、口令保护、恢复包 HTML
  FileKeyring 0600、认证备份清单（SHA-256 + HMAC）
core/store:
  schema v9 = 26 张表；schema_migrations；additive v8->v9
  SQLCipher raw key；单写 + 只读连接；integrity_check / quick_check
  legacy v4/v5 JSON 导入（源密码验证后写入）
  checkpoints（最多 12）+ restore verify
  audit hash chain + diagnostics JSON
```

M1 退出条件中的“加密、恢复、备份、迁移、审计全绿”已由上述单测覆盖；
`integrity_check` 在 store 测试中返回 `ok`。

### M2 真实领域与入口接线

```text
core/domains: assets 3 + plan 2 + overview 1 + runs 1 + policy 2 = 9 tests
core/application: 5 tests（读命令、CRUD+审计、签到、政策、交付）
apps/server: 6 tests（真实读接口、写 token 门禁、404/403）
core/render: 2 tests（纯文本转义、可信 body 页面壳）
core/store: 15 tests（schema v10 / 27 表 / 迁移 / 加密 / 诊断）
```

已实测：

- CLI 用真实 SQLite 创建资产，返回 id=1；`assets list --q 邮箱` 命中。
- HTTP `/api/v1/{overview,assets,plan,policies,security,agent/runs}` 返回真实行；`/assets` 返回 `items/nextCursor/total`。
- MCP `tools/call -> overview.get` 经 `_meta["digilegacy/command"]` 映射到 `app.overview`，返回真实 envelope，不再伪造成功。
- Web 在 `VITE_USE_MOCK=false` 下读取同一 API；资产搜索/状态筛选走服务端，分页用 `nextCursor` 追加。

### M2-B 政策库与交付物

```text
policies 表：152 条，content/policies/platforms.json，16 个分类
policy_provenance：152 条，全部 unverified，不伪造已核实
deliver.export：report/checklist/legal/toolkit 四类 HTML，jobs + deliverable_exports 落库
```

已实测：

- CLI `policies list --q 支付宝` 返回 16 个分类与命中条目；`policies get 1` 返回支付宝、`provenance.status=unverified`。
- HTTP `GET /api/v1/policies?q=支付宝` 返回 2 条命中；`GET /api/v1/policies/1` 返回完整 20 字段。
- HTTP `POST /api/v1/deliverables/export` 返回 `jobId/status=succeeded/fileName/downloadUrl`；`GET /api/v1/exports/{jobId}` 返回 `text/html` 附件。
- 报告含资产、继承人、政策材料/流程/风险；法律文本含 4 类模板；清单含平台材料；工具包含官方入口与申请表链接。
- Web 政策库并入资产二级标签：152 条、分类筛选、服务端搜索、游标加载更多、详情展开、溯源状态显示。
- Web 交付页生成后显示文件名与下载链接；资产详情可跳转对应平台政策。

未完成边界：

- Web 写入仍受 HTTP 写 token 和解锁流程阻塞；资产创建/更新目前走 CLI `--allow-write`。
- 政策溯源只读，没有“标记已核实/争议”的写命令与审核界面。
- 交付物同步生成，未接 SSE/轮询进度、取消与后台队列。
- vault 解锁未接 server/Web；元数据读链路当前可读，敏感字段展示仍需解锁。

### Web

```text
npm run typecheck   OK
npm test            OK（6 tests）
npm run build       OK
```

包体基线：

```text
总 gzip         87.29 KB / 110 KB
入口 raw        48.77 KB / 60 KB
vendor raw      98.88 KB / 130 KB
路由最大 raw    7.03 KB / 24 KB
```

### 运行态 smoke

| 入口 | 请求 | 结果 |
|---|---|---|
| Server | `GET /api/v1/health` | `200 OK`，13 commands |
| Server | `GET /api/v1/overview` | `501 UNSUPPORTED`（M0 预期） |
| Server | 未知命令 | `404 NOT_FOUND` |
| Server | `assets.create` | `401 AUTH_REQUIRED`（M0 写 token 未接） |
| Server | `connections.test` | `403 FORBIDDEN` |
| CLI | `commands` / `health` | OK |
| CLI | 未知命令 | `NOT_FOUND` |
| MCP | `initialize` / `tools/list` | OK |
| MCP | `tools/call assets.list` | `isError=true`，不伪造成功 |

### 性能基线

```text
bus_bench: 1,000,000 dispatches in 250.19ms
          ≈ 250.2 ns/op，约 4.0M ops/s
```

## 4. M0 退出条件

M0 可以判定“工程壳完成”的条件：

- [x] 新仓库结构可编译
- [x] 契约单一事实源可生成、可校验
- [x] 四入口空壳可启动/可调用
- [x] Web 可启动、可浏览 5 主入口
- [x] 旧路由 404
- [x] 架构门禁与 bundle 预算
- [x] CI 与 verify 脚本
- [x] 定位/架构/验收/DoR-DoD/ADR/RFC
- [ ] 真实领域 handler
- [ ] 安全内核与数据库
- [ ] E2E 与视觉回归
- [ ] 无 mock 生产分块

后四项属于 M1/M2，不应在 M0 里伪造为已完成。

## 5. 回归规则

- 性能回归超过 20%：必须给出原因、影响面与回滚方案。
- 契约变更导致 Web/CLI/MCP 任一消费者漂移：阻断合并。
- 新增写命令没有 `approval=required` 或没有审计字段：阻断合并。
- 把 `key/network/desktop` 暴露给 Agent/MCP：阻断合并。
- 旧路由重新出现：阻断合并。
- 前端新增页面没有加载/空/错误三态：阻断合并。
