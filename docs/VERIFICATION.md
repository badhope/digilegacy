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
| G09 | 政策溯源棘轮 | 部分接入（M3-A 写四态 + 审计） | 审核工作台待 M3 |
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

已观察测试（M3）：

- foundation：5 个
- render：2 个
- server：12 个（真实读接口、写 token 门禁、正确/错误 token、溯源 draft 写与审计、审计链读取、审批列表/决定/冲突、Runtime 启动/详情）
- security：49 个（crypto/vault/recovery/Shamir/backup/session/keyring）
- store：15 个（schema v12 / 28 表、SQLCipher、迁移、旧备份导入、检查点、审计、诊断）
- domains：13 个；application：15 个；agent：4 个；合计 115 个

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
core/domains: assets 3 + plan 2 + overview 1 + runs 1 + policy 3 = 10 tests
core/application: 5 tests（读命令、CRUD+审计、签到、政策、交付）
apps/server: 6 tests（真实读接口、写 token 门禁、404/403）
core/render: 2 tests（纯文本转义、可信 body 页面壳）
core/store: 15 tests（schema v12 / 28 表 / 迁移 / 加密 / 诊断）
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
- Web 资产新增/编辑：连接设置保存 token 后，`assets.create/update` 成功；列表刷新、审计 `asset.create/asset.update` 可查。
- 审计页：`GET /api/v1/audit` 返回真实 14 条，`limit=2` 返回 `nextCursor`；`kind=asset.update` 过滤为 1 条；Web 设置 → 审计显示哈希链记录。
- Web 交付页生成后显示文件名与下载链接；资产详情可跳转对应平台政策。

### M3-D Agent 审批队列

```text
agent.approvals.list   GET  /api/v1/agent/approvals?status=pending
agent.approvals.decide POST /api/v1/agent/approvals/{id}/decide（write，需 Bearer）
CLI approvals list --status pending | approvals decide <id> --decision approve|reject
Web /automation/approvals：状态筛选、diff、理由、批准/拒绝
```

已实测：

- `approvals` 表种子 2 条 pending：`appr_seed_1`（assets.update）、`appr_seed_2`（policies.provenance.set）。
- HTTP 无 token 决定返回 401 `AUTH_REQUIRED`；带 token 批准返回 200，`status=approved`、`decidedAt` 非空。
- 二次决定返回 409 `CONFLICT`；CLI `approvals list --status pending` 从 2 条降为 1 条。
- 审计 `GET /api/v1/audit?kind=agent.approval.decide` 返回 2 条，subject 分别为两个审批 id。
- 浏览器审批页批准后待批从 1 变 0，空态出现；自动化页侧栏读取真实待批工具。
- `core/domains` 审批测试 1 项、server 审批测试 1 项覆盖状态只能转移一次。

未完成边界：

- 目前没有生产创建审批记录的入口；Agent Runtime 尚未写入 `agent_approvals`。
- 审批通过后由 Runtime 执行当前步骤；取消、预算、重试与 LLM 规划已在 M3-F 补齐。
- 审批页批量操作、影响条数、可回滚点未做。

### M3-E 确定性 Agent Runtime

```text
agent.runs.start  POST /api/v1/agent/runs（write，需 Bearer）
agent.runs.get    GET  /api/v1/agent/runs/{id}
CLI runs start --task --steps；runs get <id>；runs list
Web /automation/runs/new、/automation/runs/:id、/automation/tools
```

已实测：

- 只读运行 `overview.get + assets.list` 返回 `succeeded`，步骤均 `done`。
- 写运行 `overview.get + assets.update` 返回 `waiting_approval`，生成 `appr_2_1`；批准后运行变 `succeeded`，资产 value 从 4200 更新到 5000。
- 从 Web 新建运行 `assets.update` 后跳转审批页；批准后同一运行完成，资产更新到 5200。
- CLI `runs get 2` 返回任务、步骤与 `succeeded`；HTTP 无 token 启动返回 401。
- 运行步骤落 `agent_run_log.steps_json`，审批通过后从当前步骤继续，拒绝则运行 `rejected`。
- Tool Registry 名称与命令名双解析：`overview.get` 映射到 `app.overview`。

### M3-F 规划、取消、预算与重试

```text
agent.runs.plan    POST /api/v1/agent/plan（draft）
agent.runs.cancel  POST /api/v1/agent/runs/{id}/cancel（write，需 Bearer）
agent.runs.retry   POST /api/v1/agent/runs/{id}/retry（write，需 Bearer）
CLI runs plan --task --max-steps；runs cancel <id>；runs retry <id>
Web /automation/runs/new 的“AI 生成步骤”；运行详情的“取消运行/重试运行”
```

已实测：

- 本地 OpenAI 兼容 mock 规划器返回 `model=mock-planner` 与 `overview.get + assets.list`；Web 点击“AI 生成步骤”后步骤 JSON 被填回，并显示规划器模型。
- `agent.runs.cancel` 将运行置 `cancelled`、步骤置 `cancelled`，该运行 pending 审批变 `expired`；对过期审批再次 decide 返回 `409 CONFLICT`。
- `agent.runs.retry` 对 `rejected` 运行创建新运行并进入 `waiting_approval`；旧运行仍保持 `rejected`。
- `maxWriteSteps=1` 的 2 个写步骤运行在启动前返回 `400 INVALID_ARGS`，不创建运行。
- CLI `runs plan`、`runs retry 10`、`runs cancel 11` 均返回预期结果。
- 运行详情监听路由参数变化，重试跳转新运行后会重新加载，不再残留旧运行内容。

未完成边界：

- 规划器需要外部 OpenAI 兼容端点与 key；未配置时明确返回 `UNSUPPORTED`，不会伪造计划。
- 取消是协作式：已同步进入执行的单个工具不会被中断；HTTP 审批已后台执行，CLI 默认同步。
- 预算只在启动前静态校验，不落库、不做运行时动态消耗。
- 运行详情未展示 token/成本；审批影响条数与回滚点未做。

### M3-G 审批后台 Job、运行进度与耗时

```text
jobs.get            GET /api/v1/jobs/{id}（read）
HTTP 审批            POST /api/v1/agent/approvals/{id}/decide（强制 background=true）
CLI approvals decide --background；jobs get <id>
Worker              单进程 250ms 轮询；回收 >120s 的 running 僵尸任务
Web                 审批页后台提示 + jobId；运行页 1.5s 静默轮询，页面隐藏暂停
```

已实测：

- 写运行 `id=1` 审批 `appr_1_0`：HTTP 批准 **1.329 ms** 返回 `jobId`；Worker 约 **45 ms** 完成 `agent.resume`；运行 1 变 `succeeded`，`durationMs=4330`（含等待审批），步骤 `done`。
- `GET /api/v1/jobs/{jobId}` 返回 `succeeded`、`progress=1`、`result={approvalId,runId,runStatus}`、`finishedAt` 比入队晚 45 ms。
- Web 第二次审批（运行 4）批准后显示“已批准，后台正在执行”、`job_...` 与“查看运行”；运行详情显示 **耗时 8.4 s**。
- 运行 5 在页面显示“等待审批”时从外部批准，2.5 秒后页面无刷新自动变为“已完成”，确认静默轮询生效。
- CLI `approvals decide --background` 返回 `jobId`；`jobs get` 返回 job `succeeded`，运行 6 `succeeded`、`durationMs=6945`。
- schema v11 为 `agent_run_log` 增加 `duration_ms`，仍 27 表；迁移 ledger `v11_agent_run_duration`。
- 修复：`jobs.finish` 的 `?2` 复用导致 3 参数对 2 占位符（`Got 3, needed 2`）；`useResource.reload` 被模板事件传入 `PointerEvent`，拆分 `reload` / `refresh`。

未完成边界：

- Web 当前是 1.5 秒静默轮询，不是 SSE；SSE 留作后续优化。
- token/成本未落库，只记录 `duration_ms`；`Run.tokens` 仍为 0。
- CLI 审批默认仍同步（可显式 `--background`）；HTTP 已全量后台化。
- 规划器只对本地 mock provider 验证，真实 DeepSeek/豆包 key 未配置。

### M3-H 交付物后台导出

```text
deliver.export  wait=false → jobs（kind=deliver.export）→ jobId
Worker          deliver::run_job → HTML + deliverable_exports → jobs::finish
Web             DeliverablesView 轮询 jobs.get，完成后显示下载
CLI             deliver export 默认同步；--background 入队
```

已实测：

- HTTP `POST /api/v1/deliverables/export` **1.462 ms** 返回 `status=queued`、`jobId`，不阻塞请求。
- `jobs.get` 轮询到 `succeeded`、`progress=1`、`result.fileName=job_..._deliver_export.html`；`GET /api/v1/exports/{jobId}` 返回 `200`、1912 bytes HTML。
- Web 交付页点击“生成”：0.5 秒显示 jobId 与“生成中”，2 秒后自动变为“已生成 + 下载 HTML”。
- CLI `deliver export --kind report --background` 返回 `queued`；`jobs get` 返回 `succeeded` 与文件名。
- 失败路径由 `jobs::fail` 记录 `error_code`；`requeue_stale` 覆盖 `agent.resume` 与 `deliver.export`。

未完成边界：

- 交付物进度仍是 1.5 秒轮询，不是 SSE。
- 导出任务未支持取消；重复生成会创建多个 job。
- 后台导出只记录 job 状态，未把 `duration_ms` 写入交付物表。

### M3-I SSE 与排队任务取消

```text
GET /api/v1/events?runId=5 | ?jobId=job_... → text/event-stream
server: 500ms 读取当前 RunDetail / JobDetail；变化时 event: snapshot + JSON；15s keepalive
web:    RunDetailView / DeliverablesView 优先 EventSource；连接失败回退 1.5s 轮询
jobs.cancel: queued → cancelled / CANCELLED + finishedAt；其余状态 CONFLICT
```

已实测：

- 运行页打开等待审批的 run #2 后，外部 HTTP 批准，**0.6 秒内**页面变为“已完成”，早于 1.5s 轮询；浏览器网络记录存在 `GET /api/v1/events?runId=2`。
- `curl -N /api/v1/events?runId=1` 依次输出 `waiting_approval`、`pending_approval`、`succeeded`、`done` 的 snapshot，状态变化由 SSE 推送。
- `curl -N /api/v1/events?jobId=...` 返回 queued job 的完整 `JobDetail`，无变化时 `: keepalive`。
- CLI：`deliver export --background` 入队后，`jobs cancel <id>` 返回 `cancelled`、`errorCode=CANCELLED`；再次 `jobs get` 仍为 `cancelled`。
- 单元测试 `cancels_queued_job_once` 覆盖 queued 取消与二次取消 `CONFLICT`；application 测试 14 项。
- 契约增至 **25 命令 / 17 工具 / 31 schema / 19 错误码**；`jobs.cancel` 仅 CLI/HTTP/IPC，mcp/agent 为 false。

未完成边界：

- SSE 是服务端 DB 轮询（每连接 500ms），不是跨进程广播总线；单工作区可接受，多实例需改事件总线。
- `jobs.cancel` 只取消 `queued`；`running` 的导出任务无法中断，返回 `CONFLICT`。
- token/成本仍未落库；`Run.tokens` 仍为 0，下一步需接真实 LLM usage。

### M4-A Agent 用量账本

```text
agent.runs.plan  → provider usage → agent_plan_log（tokens/成本/定价状态）
agent.runs.start → planId → agent_run_log（tokens_in/out/cost_micros/plan_id）
agent.runs.list/get → tokens / costMicros / usagePriced / planId
```

已实测：

- Mock OpenAI 兼容 provider 返回 `prompt_tokens=321`、`completion_tokens=87`；`agent.runs.plan` 返回 `planId`、`usage.costMicros=495`、`priced=true`。
- HTTP 用该 `planId` 启动运行 #3，返回 `tokens=408`、`tokensIn=321`、`tokensOut=87`、`costMicros=495`、`usagePriced=true`、`planId` 正确。
- Web 新建运行页显示“输入 321 / 输出 87 tokens · ¥0.000495”；点击开始后运行 #4 详情显示“用量 408 tokens · ¥0.000495”。
- schema v12：`agent_plan_log` 新表，`agent_run_log` 增加 `tokens_in/tokens_out/cost_micros/cost_priced/plan_id`；共 28 表。
- 单价由 `DIGILEGACY_LLM_INPUT_COST_MICROS_PER_MILLION` / `DIGILEGACY_LLM_OUTPUT_COST_MICROS_PER_MILLION` 配置；未配置时仍记 token，`priced=false`，UI 显示“成本未配置”。

未完成边界：

- 成本只覆盖规划器调用；工具执行的费用/外部 API 成本未计入。
- 没有按日/模型聚合的用量报表；当前按计划记录与运行继承查询。
- 真实 DeepSeek/豆包 key 未配置，E2E 使用本地 mock provider。

### 未完成边界

- 政策审核工作台与批量核对尚未做；政策溯源写已支持四态与审计。
- 交付物同步生成；后台 Job 已用于 Agent 审批恢复，交付导出仍为同步。
- vault 解锁未接 server/Web；元数据读链路当前可读，敏感字段展示仍需解锁。

### Web

```text
npm run typecheck   OK
npm test            OK（6 tests）
npm run build       OK
```

包体基线：

```text
总 gzip         106.35 KB / 110 KB
JS gzip         88.14 KB / 89 KB
CSS gzip        18.21 KB / 19 KB
入口 raw        64.41 KB / 65 KB
vendor raw      98.89 KB / 130 KB
路由最大 raw    8.96 KB / 24 KB
```

### 运行态 smoke

| 入口 | 请求 | 结果 |
|---|---|---|
| Server | `GET /api/v1/health` | `200 OK`，25 commands / 28 表 / schema v12 |
| Server | `GET /api/v1/overview` | `200 OK`，真实资产/规划/活动 |
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
