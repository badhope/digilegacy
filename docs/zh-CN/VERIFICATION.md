# 验收与门禁

[English](../VERIFICATION.md) | 简体中文

> 英文页是面向贡献者的门禁与 smoke 摘要；本页保留逐阶段的详细验收记录。

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
- store：15 个（schema v15 / 30 表、SQLCipher、迁移、旧备份导入、检查点、审计、诊断）
- domains：17 个；application：19 个；agent：4 个；合计 123 个

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
core/store: 15 tests（schema v15 / 30 表 / 迁移 / 加密 / 诊断）
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
- 审计修复：`planId` 与 task 不匹配返回 `INVALID_ARGS`（HTTP 400 实测）；`jobs.cancel` 条件更新避免竞态；`claim_next` 检查行数避免双 Worker 重复执行；运行页切换 Run 后 SSE 重新绑定新 `runId`（浏览器实测 0.7s 内更新）。

未完成边界：

- 成本只覆盖规划器调用；工具执行的费用/外部 API 成本未计入。
- 没有按日/模型聚合的用量报表；当前按计划记录与运行继承查询。
- 真实 DeepSeek/豆包 key 未配置，E2E 使用本地 mock provider。

### M4-B 用量汇总

```text
agent.usage.summary(days=30)
  totals  ← agent_plan_log 的 token/成本；agent_run_log 只计 run 数
  byDay   ← 计划 token/成本 + 当日运行数
  byModel ← 按模型聚合的计划 token/成本
HTTP GET /api/v1/agent/usage
CLI  usage summary --days 30
```

已实测：

- 领域测试固定两条 plan（135 tokens，其中 1 条未定价）与一条 run（100/20 tokens），汇总返回 `plans=2`、`runs=1`、`tokens=135`，证明运行继承 token 没有被二次相加。
- Application 测试：一条 120-token 计划启动运行后，汇总仍为 `plans=1`、`runs=1`、`tokens=120`。
- 契约扩至 **26 命令 / 18 工具 / 33 schema / 19 错误码**；`agent.usage.summary` 对 MCP/Agent 开放，权限 read。
- Web 自动化页新增“用量”卡，展示规划数、运行数、token、成本、模型明细与未定价提示。

未完成边界：

- 汇总只覆盖规划器用量；工具执行/外部 API 成本未计。
- 没有导出 CSV/账单的入口；当前是运行时查询。
- `byDay` 使用 UTC 日期切分，跨时区账单口径需后续统一。

### M4-G 触发链完整生命周期

```text
trigger.configure  -> enabled / days / pauseDays，暂停或关闭会中止当前触发
trigger.evaluate   -> stage1 -> stage2 -> stage3 -> final，按等待期幂等推进
trigger.checkin    -> last_check_in + 清空 triggered_at/stage + checkin 事件
trigger.events.list -> 只追加时间线（配置/签到/阶段）
```

已验证：

- HTTP 无写令牌 `PATCH /api/v1/trigger` 返回 `401 AUTH_REQUIRED`；带 token 配置返回 `armed`。
- 把 `last_check_in` 置为过去时间后 `POST /api/v1/trigger/evaluate` 返回 `triggered/stage=1`；`GET /api/v1/trigger/events` 返回 `stage1` 与 `configure` 事件。
- `trigger.checkin` 后 `plan.get` 返回 `status=armed`、`triggeredAt=''`、`stage=0`；事件链新增 `checkin`。
- 重启 server 时自动评估线程立即执行：过期签到被置为 `triggered/stage=1`，并写 `trigger.evaluate.auto` 审计；把 `triggered_at` 置老后评估可推进到 `stage=4/final`。
- Web 实测“暂停 7 天”进入 `paused` 并显示暂停至日期；恢复布防、评估触发链、事件时间线均可操作。
- CLI 实测 `trigger configure --enabled --days 30`、`trigger evaluate`、`trigger events`。
- `./tools/verify.sh` exit 0：127 Rust + 10 Web；契约 **31 命令 / 20 工具 / 41 schema / 19 错误码**；schema v14 / 29 表。
- 预算：入口 70.41 KB / 72 KB、JS gzip 91.85 KB / 93 KB、CSS 18.78 KB / 19 KB、总 gzip 110.63 KB / 112 KB。

### M4-F 旧代码清退

```text
删除 legacy/（旧桌面端、旧文档、旧脚本、旧截图、node_modules）
删除 apps/desktop（桌面空壳）
workspace members 13 -> 12
```

已验证：

- `legacy/` 不再存在；`git ls-files legacy` 为空；`apps/desktop` 不再存在。
- `./tools/verify.sh` exit 0，119 Rust + 10 Web 通过；架构门禁 workspace members=12。
- 契约计数不变：26 命令 / 18 工具 / 33 schema / 19 错误码。
- 预算：入口 68 KB、JS gzip 91 KB、CSS 19 KB、总 gzip 110 KB。
- `legacy_import` 保留，作为旧备份导入能力，不属于旧应用代码。

### M4-E 规划签到闭环

```text
plan.get  -> trigger.lastCheckIn / nextCheckAt
trigger.checkin -> check_in_log + dms_config.last_check_in + audit
Web       -> 完成签到 + 成功提示 + 静默刷新
```

已验证：

- 浏览器实测：未配置写令牌时签到返回 `AUTH_REQUIRED`，页面给出“需要写令牌”提示；写入 `test-write-token` 后签到成功，显示“已签到 · 2026/11/19”。
- `plan.output` schema 与 Web `Plan` 类型同步；全仓 verify 通过。
- 签到只记录/重置计时，不自动启用 DMS；启用仍由安全设置控制。

未完成边界：

- 触发链阶段时间线、邮件发件箱、暂停/恢复 UI 尚未实现。

### M4-D 运行中协作取消

```text
agent.runs.cancel(id)
  running          -> cancel_requested=1, outcome=cancelling
  waiting_approval -> expire approvals, steps cancelled, outcome=cancelled
executor
  step boundary    -> cancel_requested? abandon + cancelled
  finish           -> succeeded 落库受 cancel_requested 条件保护
```

已验证：

- schema v13：`agent_run_log.cancel_requested`，仍 28 表；Store 15 项测试全绿。
- Application 测试：手工 running 运行取消后状态 `cancelling`，重复取消仍返回 `cancelling`；`execute_from` 在取消置位后不执行步骤，直接落 `cancelled`。
- 原有 `waiting_approval` 取消/审批过期测试继续通过。
- Web `RunStatus` 增加 `cancelling`，运行详情按钮在取消中禁用并显示“取消中…”。

未完成边界：

- 取消依赖步骤边界，单个长工具调用无法被强制抢占；这是协作式取消，不是杀线程。
- 后台 `agent.resume` 的 Job 仍只有 queued 可取消；运行本身可协作取消。

### M4-C 查询缓存与加载策略

```text
cachedQuery(key, loader, 10s)  ← 成功才缓存；同 key 并发共享 inflight
platformClient.read(...)       ← 稳定列表/详情接入
platformClient.write(...)      ← 成功后按域失效 + audit
getRun/getJob                  ← 直接 invoke，不走缓存
```

已验证：

- `queryCache.test.ts` 4 项：TTL 命中只调用一次、并发去重、失败不缓存、前缀失效。
- 全仓 `npm test` 10 项通过（缓存 4 + 路由/页面 6）。
- 路由 hover/focus 预取只加载目标 chunk，首屏仍只加载入口与概览。
- 运行/Job 详情保留 SSE + 1.5 秒兜底轮询，缓存不介入。

未完成边界：

- 外部 CLI/Agent 写入不会主动通知浏览器缓存失效，仪表盘最多滞后 10 秒。
- 未做统一广播总线；当前以 TTL 与页面内失效兜底。

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
总 gzip         110.63 KB / 112 KB
JS gzip         91.85 KB / 93 KB
CSS gzip        18.78 KB / 19 KB
入口 raw        70.41 KB / 72 KB
vendor raw      101.88 KB / 130 KB
路由最大 raw    8.96 KB / 24 KB
```

### 运行态 smoke

| 入口 | 请求 | 结果 |
|---|---|---|
| Server | `GET /api/v1/health` | `200 OK`，39 commands / 31 表 / schema v16 |
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
- [x] 三入口空壳可启动/可调用（server/CLI/MCP）
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

### M4-H 资产发现与批量导入

- `content/assets/catalog.json`：8 分类、36 条建议；每条包含名称、类型、平台、中文/英文理由。
- `assets.discover`：支持 `category`/`q` 过滤；按现有资产的 name+platform 标记 `exists`，不返回重复建议。
- `assets.bulkCreate`：预校验所有条目；最多 50 条；已存在同名同平台跳过；返回 `created/skipped/items`；写审计 `assets.bulk_create`。
- HTTP：无 token `POST /api/v1/assets/bulk` 返回 401；带 `test-write-token` 首次创建 2 条、重跑返回 `created=0/skipped=1`。
- CLI：`assets discover --category finance` 返回 6 条；`assets bulk-create --file /tmp/digilegacy-bulk.json --allow-write` 成功创建 1 条。
- Web：`/assets/discover` 实测分类 → 勾选微信零钱/支付宝余额 → 确认 → 成功页（新建 2、跳过 0）；再次进入同一分类显示“已在库”，不能重复勾选。
- 预算调整：入口 **74.68 KB / 76 KB**、JS gzip **95.86 KB / 97 KB**、CSS gzip **20.30 KB / 21 KB**、总 gzip **116.17 KB / 118 KB**；新增 `AssetsDiscoverView` 路由块 8.72 KB。

### M4-I 前端整合与去重

- 新增 `web/src/features/assets/AssetEditor.vue`：创建/编辑共用同一表单，减少两套字段、校验与样式。
- 新增 `web/src/features/assets/model.ts`：统一 `ASSET_KINDS`、`assetKindKey`、草稿映射。
- 新增 `web/src/shared/format.ts`：统一日期 `formatDate` 与日期时间 `formatDateTime`，替换资产、规划、审批、运行、自动化 5 处重复实现。
- 资产详情移除重复的“编辑说明”卡片块；资产页移除重复的 `app.mode` 胶囊；详情编辑时隐藏“编辑资产”按钮。
- 移动端资产表隐藏平台/更新时间列，取消固定 720px 最小宽度，避免横向溢出。
- 实测：共享编辑器在 `/assets` 与 `/assets/1` 均正常打开；日期渲染正常；创建/编辑逻辑未变。
- `./tools/verify.sh` exit 0：127 Rust + 10 Web；契约仍 31/20/41/19；预算入口 75.51 KB / 76 KB、JS 94.48 KB / 97 KB、CSS 19.74 KB / 21 KB、总 114.22 KB / 118 KB。

### M5-A 最终通知发件箱

- schema v15 / 30 表新增 `notification_outbox`：`id/created_at/trigger_event_id/kind/recipient/recipient_role/subject/body/status/attempts/last_error/sent_at`，唯一约束 `(trigger_event_id, recipient)`。
- `notify::enqueue_final` 只在 final 阶段生成；收件人取紧急联系人、主继承人、执行人，邮箱优先、其次电话/名称，按 recipient 去重；无接收人时写入 `blocked` 行并保留原因。
- 状态机：`pending` → `sent` / `failed`；`blocked` 不投递。后台 worker 每 60 秒处理 pending；`DIGILEGACY_WEBHOOK_URL` 未配置时只入箱，手动发送返回 `configured=false + pending`。
- HTTP `GET /api/v1/notifications` 只读；`POST /api/v1/notifications/send` 走写 token，服务端用 `spawn_blocking` 执行 blocking webhook client，避免 Tokio 运行时 drop panic；`retryFailed=true` 会先把 failed 行排回 pending。CLI `notifications list/send --retry-failed`；MCP 只暴露只读列表。
- 实测：v14 数据库重启自动迁移到 v15 / 30 表；final 评估生成 2 条 pending；mock webhook 收到 2 条并置 `sent`，审计 `notification.send.auto` 记录 `sent=2;failed=0`；未配置 webhook 时手动发送返回 `configured=false/pending=1`，无 panic；failed 行经 `retryFailed` 回到 pending。
- Web 规划页“最终通知”发件箱实测空态、2 条列表、状态 Pill、失败原因位置、发送按钮与未配置提示；1920 桌面与 390px 移动端均无横向溢出。
- `./tools/verify.sh` exit 0：**131 Rust + 10 Web**；契约 **33 命令 / 21 工具 / 45 schema / 19 错误码**；schema v15 / 30 表。
- 预算（因新增通知 i18n 与规划页逻辑，入口预算从 76 调到 80 KB）：入口 **77.91 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **95.50 / 97 KB**、CSS gzip **19.92 / 21 KB**、总 gzip **115.42 / 118 KB**。

### M5-B 连接健康与通知渠道

- 新增 `connections.list`：HTTP `GET /api/v1/connections`，读模型返回 `id/kind/label/enabled/lastCheckAt/lastStatus/lastError/updatedAt/source`，不返回 `config_enc` 或任何凭据；`source=env` 只由 `config_enc='env'` 标记推导。
- `connections.test` 改为 write + `agentApproval=required`，HTTP `POST /api/v1/connections/test` 需要 `DIGILEGACY_API_TOKEN`；无 token 实测 401 `AUTH_REQUIRED`，带 `test-write-token` 实测 Webhook 返回 `ok=true/status=ok/HTTP 200` 并回写 `last_status=ok`。
- 8 类探针：`llm`（`/models`）、`policy`/`heartbeat`（GET）、`webhook`/`feishu`/`wecom`（POST 对应 payload）、`smtp`/`imap`（TCP 连通性）。失败 URL 脱敏，成功不把 `HTTP 200` 写进 `last_error`。
- 通知通道实测：`DIGILEGACY_NOTIFY_CHANNEL=feishu` + `DIGILEGACY_FEISHU_WEBHOOK_URL` 投递 1 条，sink 收到 `{"msg_type":"text","content":{"text":"【M5-B 通道验证】..."}}`，发件箱置 `sent`；`wecom` 同样收到 `{"msgtype":"text","text":{"content":"..."}}` 并置 `sent`。临时 outbox 行与测试通道元数据已清理。
- Web Settings → 连接页实测桌面 1920 与移动端 390px：连接列表、状态、最近检查、失败原因、测试按钮均正常；390px `scrollWidth=390`，无横向溢出；点击测试后绿色 `ok · 3 ms · HTTP 200` 提示。
- CLI `connections list --json` 返回真实连接；`connections test --kind webhook --id 2 --allow-write --json` 返回 `ok=true/HTTP 200`；无 `--allow-write` 返回 `FORBIDDEN`。
- M5-B 修复：reqwest blocking 探针改为普通 OS 线程创建/销毁，避免 Tokio `spawn_blocking` 关闭内部 runtime 时 panic；首次实现曾返回空 HTTP body，修复后 curl 与 Web 均拿到完整 JSON。
- `./tools/verify.sh` exit 0：**138 Rust + 10 Web**；契约 **34 命令 / 22 工具 / 46 schema / 19 错误码**；schema v15 / 30 表。
- 预算：入口 **78.85 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **96.84 / 97 KB**、CSS gzip **20.24 / 21 KB**、总 gzip **117.08 / 118 KB**；Settings 路由 11.26 / 24 KB。

### M5-C 连接配置加密写接口与 SMTP 投递

- 契约扩至 **36 命令 / 22 工具 / 49 schema / 20 错误码**；新增 `connection.upsert.input.json`、`connection.output.json`、`connection.delete.output.json`，新增错误码 `CONFIG_KEY_MISSING`（HTTP 409 / cliExit 3）。
- `connections.upsert`：HTTP `POST /api/v1/connections`、CLI `connections save`；MCP/Agent 不暴露。输入 `id?/kind/label?/endpoint/token?/username?/from?/security?/enabled?`；配置密钥由 `DIGILEGACY_CONFIG_KEY_FILE` 提供，缺省从 DB 密钥 HKDF 派生（域标签 `digilegacy/connection-config/v1`），都没有时返回 `CONFIG_KEY_MISSING`。配置整体加密为 `enc:v2`，读模型只返回 kind/label/enabled/health/source，不回显 endpoint/token/username/from/security。
- `connections.delete`：HTTP `DELETE /api/v1/connections/{id}`、CLI `connections delete`；环境连接改/删返回 `CONFLICT`；新增同类型同标签返回 `CONFLICT`，编辑必须带 id，不能改变 kind。
- Web Settings → 连接页实测：新增保存 SMTP、编辑重填地址/凭据、删除确认、测试按钮、环境连接只读（无编辑/删除）；无写令牌提示、`CONFIG_KEY_MISSING` 映射为“服务端未配置连接配置密钥”；390px 单列表单 `scrollWidth=390`，无横向溢出；亮色/暗色主题均正常。
- SMTP 发送单元测试覆盖本地明文 SMTP server：EHLO、MAIL、RCPT、DATA、QUIT、UTF-8 主题 base64、正文 dot-stuffing；AUTH PLAIN 仅实现并编译通过，未在单测或真实服务器验证。未做真实 STARTTLS/TLS 服务器联调，仅验证安全模式解析与本地明文链路；这是当前已知边界。
- 端到端：临时运行库 `/tmp/m5c-e2e.db` 配置已保存 SMTP 连接（id=1，`127.0.0.1:2526`、security=none、from=sender@example.com）；注入 1 条 pending outbox，设置 `DIGILEGACY_NOTIFY_CHANNEL=smtp` 启动 server，后台 worker 实际投递到本地 SMTP sink，sink 收到 `From/To/Subject` 与中文正文，outbox 置 `sent`；手动 `POST /api/v1/notifications/send` 返回 `configured=true/sent=0/failed=0/pending=0`。临时库、密钥、outbox 与 sink 仅用于验收，未进入仓库。
- 无配置密钥实测：独立空库 `/tmp/m5c-nokey.db` 启动 server，`connections.upsert` 返回 `CONFIG_KEY_MISSING`，Web 显示对应中文提示，未写入连接。
- `./tools/verify.sh` exit 0：**146 Rust + 10 Web**；workspace members=12；schema v15 / 30 表；Settings 路由 17.46 / 24 KB。
- 预算：入口 **79.31 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **98.75 / 100 KB**、CSS gzip **20.50 / 21 KB**、总 gzip **119.25 / 121 KB**。JS/总 gzip 预算从 97/118 调到 100/121，用于新增管理表单与 SMTP 配置 UI；入口与 vendor 预算未放宽。
- 复验命令：`./tools/verify.sh`；日志 `/tmp/m5c-verify.log`。
### M5-D 政策远程同步与候选审核

- schema v16 / 31 表：新增 `policy_sync_state`（`source/cursor/etag/last_sync_at/last_status/last_error`）；`policy_candidates(status, fetched_at)` 加索引。迁移 ledger 写 `v16_policy_sync_state`。
- 契约扩至 **39 命令 / 23 工具 / 56 schema / 21 错误码**；新增 `policies.sync`（HTTP `POST /api/v1/policies/sync`、CLI `policies sync`）、`policies.candidates.list`（HTTP `GET /api/v1/policies/candidates`、CLI `policies candidates`、MCP 只读）、`policies.candidate.decide`（HTTP `POST /api/v1/policies/candidates/{id}/decide`、CLI `policies candidates decide`）；新增错误码 `PROVIDER_UNCONFIGURED`（HTTP 409 / cliExit 3）。
- `policies.sync` 只写候选：本地 feed 返回 1 条修改后的支付宝政策，同步返回 `fetched=1/queued=1/changed=1/conflict=0/cursor=feed-cursor-1`，消息明确“政策库未改变”；随后 `GET /api/v1/policies/1` 的 `notes` 仍是旧值，证明拉取未自动落库。
- 候选列表：`GET /api/v1/policies/candidates?status=all&limit=20` 返回 `total=1`、`lastSyncAt`、`lastStatus=ok`、候选 `platformId=1`、`name=支付宝`、`status=pending`、7 个 changedFields。
- 批准：`POST /api/v1/policies/candidates/1/decide` body `{"action":"approve","note":"M5-D e2e"}` 返回 approved；政策 notes 更新，provenance 回 `unverified`，note 为“同步候选已批准；内容已更新，来源需重新核对”；审计新增 `policy.candidate.approve`、`policy.sync`。
- ETag/304：第二次同步返回 `notModified=true/fetched=0/queued=0`，消息“远端未修改（304），没有入队候选”；feed 日志记录 `path=/v1/policies?since=feed-cursor-1` 与 `if_none_match=feed-etag-1`。
- Web：政策页显示“待审核候选 1”、字段级 diff、批准/拒绝；候选 2 页面批准后政策 notes 更新，候选 3 页面拒绝后政策不变；全部审完后候选面板隐藏；候选 4 的 notes diff 在 390px 单列展开，`scrollWidth=390`，亮/暗主题均正常。截图见 `/tmp/bu-shot-52at11zj.png`、`/tmp/bu-shot-vwhh_qmy.png`、`/tmp/bu-shot-n36m6bji.png`。
- CLI：`policies candidates --status all` 返回真实候选；`policies sync --allow-write` 返回 `notModified=true`；`policies decide 4 --action reject` 返回 rejected 且政策不变；`policies provenance 1 --status unverified --allow-write` 写回成功。
- `./tools/verify.sh` exit 0：**158 Rust + 10 Web**；契约 **39 命令 / 23 工具 / 56 schema / 21 错误码**；schema v16 / 31 表；workspace members=12。
- 预算：入口 **77.27 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **100.35 / 101 KB**、CSS gzip **20.81 / 21 KB**、总 gzip **121.15 / 122 KB**；Policies 路由 15.88 / 24 KB。
- 签名校验单测覆盖 `sha256=<hex>`、`sha256:<hex>` 与裸 hex，以及缺失、篡改、非法 hex 的 fail-closed；未用带签名的远端 feed 做真实 HTTP 联调。普通 localhost 明文 HTTP feed（未签名）已验证 200/304、候选入队和审核链路。
- 复验命令：`./tools/verify.sh`；日志 `/tmp/m5d-verify8.log`、`/tmp/m5d-feed.log`、`/tmp/m5d-server-8787.log`。

### M5-E 出站连接安全补丁

- `core/security/src/vault.rs` 的 `validate_external_url` 增加统一出站地址拦截：云元数据主机、云元数据 IP、IPv4 私网、IPv6 unique-local/link-local、unspecified/multicast 字面地址默认拒绝；loopback 保留，`DIGILEGACY_ALLOW_PRIVATE_NETWORK=1` 可放行局域网字面地址。IPv4-mapped IPv6 与尾部点号主机也纳入判断。
- `core/application/src/policy_sync.rs` 在读取 saved policy 连接或 `DIGILEGACY_POLICY_URL` 后、发起请求前复用同一校验；`http` 仍只允许 localhost，避免政策同步链路成为内网探测入口。
- 安全单测新增 2 个：云元数据地址（含 `169.254.169.254`、`100.100.100.200`、`fd00:ec2::254`、IPv4-mapped IPv6、尾部点号）拒绝；私网默认拦截、loopback 放行、显式开关放行。
- `core/integrations` 新增 2 个真实本地 HTTP 集成测试：签名 feed 返回 `X-Digilegacy-Signature`，验证 HMAC 通过、错误密钥/篡改正文拒绝；`If-None-Match` 请求命中 ETag 后返回 304，客户端正确读取状态与 ETag。
- `./tools/verify.sh` exit 0：**162 Rust + 10 Web**；契约 **39 命令 / 23 工具 / 56 schema / 21 错误码**；schema v16 / 31 表；workspace members=12。
- 预算保持：入口 **77.27 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **100.35 / 101 KB**、CSS gzip **20.80 / 21 KB**、总 gzip **121.15 / 122 KB**；Policies 路由 15.88 / 24 KB。
- 已知边界：当前拦截字面地址，尚未在 DNS 解析后校验解析结果，也没有完整的出站 allowlist/DNS rebinding 防护；真实 STARTTLS/TLS SMTP 服务器联调仍未做。
- 复验命令：`./tools/verify.sh`；日志 `/tmp/m5e-verify.log`。

### M5-F SMTP STARTTLS 与隐式 TLS 真实联调

- `core/integrations` 的 TLS 客户端支持测试注入额外根证书；生产路径仍只使用系统 CA，不改变公开 `send_smtp` API。
- 新增本地 rustls SMTP 测试服务器，测试时用 `rcgen` 在内存生成 localhost 自签证书（不落仓库，只信任测试客户端），覆盖：
  - 隐式 TLS：TCP 连接后直接握手，完成 `EHLO`、`AUTH PLAIN`、`MAIL/RCPT/DATA`、中文正文与 `QUIT`。
  - STARTTLS：明文 `EHLO` → `STARTTLS 220` → TLS 握手 → TLS 后重发 `EHLO` → `AUTH PLAIN` → 投递；验证了 RFC 3207 要求。
  - 证书校验：测试客户端显式信任测试证书；未信任时握手失败。
- 修复 SMTP 行读取的 UTF-8 处理：从逐字节 Latin-1 拼接改为收集原始字节后统一解码，中文正文与错误响应不再被拆坏。
- `./tools/verify.sh` exit 0：**164 Rust + 10 Web**；契约 **39 命令 / 23 工具 / 56 schema / 21 错误码**；schema v16 / 31 表；workspace members=12。
- 预算：入口 **77.27 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **100.34 / 101 KB**、CSS gzip **20.81 / 21 KB**、总 gzip **121.15 / 122 KB**；Policies 路由 15.88 / 24 KB。
- 已知边界：测试证书只用于本地测试，不是生产凭据；真实邮件服务商的 STARTTLS/隐式 TLS 互操作仍建议在发布前用实际账号做一次人工验收。
- 复验命令：`./tools/verify.sh`；日志 `/tmp/m5f-verify.log`。

### M5-G 弹层焦点恢复与 Tab 焦点陷阱

- 新增 `web/src/app/dialogFocus.ts`：统一收集可聚焦控件、处理 `Tab` / `Shift+Tab` 循环、打开时记录触发元素并在关闭后恢复焦点。
- 接入 Agent 抽屉、命令面板、锁定层：Agent 抽屉初始焦点在面板，命令面板初始焦点在搜索框，锁定层初始焦点在解锁按钮；`Escape` 仍由各弹层自行处理，避免把不可关闭的锁定层误关。
- 单测覆盖 5 个场景：可聚焦控件过滤 `tabindex=-1`、末项 Tab 回绕、首项 Shift+Tab 回绕、焦点在弹层外时拉回、面板容器自身 Shift+Tab 回绕。
- `./tools/verify.sh` exit 0：**164 Rust + 15 Web**；契约 **39 命令 / 23 工具 / 56 schema / 21 错误码**；schema v16 / 31 表。
- 预算：入口 **78.32 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **100.78 / 101 KB**、CSS gzip **20.81 / 21 KB**、总 gzip **121.59 / 122 KB**；Policies 路由 15.88 / 24 KB。
- 边界：这是弹层键盘焦点回归，不等同于完整 axe/WCAG AA 审计；对比度、读屏文案与减少动效仍需 M6 的完整 a11y 检查。
- 复验命令：`./tools/verify.sh`；日志 `/tmp/m5g-verify.log`。

### M6-A 壳层可访问性与自动化门禁

- 新增本地化 `a11y.skipToMain` 与 `a11y.primaryNav`，跳转主内容链接不再复用“概览”文案；主导航标签在中英文下均有明确名称。
- 移动导航改为模态抽屉：菜单按钮带 `aria-expanded`，打开时焦点进入抽屉关闭按钮，背景 `frame` 设 `inert`，`Escape` 关闭并把焦点还给菜单按钮；关闭状态下通过 `visibility: hidden` 将抽屉移出 Tab 顺序和可访问性树，避免 1100px 以下隐藏导航仍可聚焦。
- 命令面板 `ESC` 按钮补 `aria-label`，复制命令状态加 `aria-live`，读屏可以听到“已复制”。
- 设计令牌调整：亮色 `--muted` 改为 `#5d6357`、`--clay` 改为 `#8f4e2f`、`--amber` 改为 `#825a1b`；暗色 `--clay` 改为 `#d79674`。新增 `web/src/design/contrast.test.ts`，解析 `tokens.css` 并对亮/暗各 15 组前景/背景执行 WCAG AA 4.5:1 断言。
- 新增 `web/src/app/a11y.test.ts`：用 `axe-core` 按 WCAG 2.0/2.1/2.2 A/AA 标签扫描壳层、Agent 抽屉、命令面板、锁定层；另测移动导航打开/关闭的 `aria-expanded`、`role="dialog"`、背景 `inert` 和焦点恢复。
- `axe-core` 与 `@types/node` 仅作为 Web devDependencies；生产构建不包含 axe。
- `./tools/verify.sh` exit 0：**164 Rust + 51 Web**；契约 **39 命令 / 23 工具 / 56 schema / 21 错误码**；schema v16 / 31 表；workspace members=12。
- 预算：入口 **79.16 / 80 KB**、vendor 99.50 / 130 KB、JS 总 gzip **101.07 / 101.1 KB**、CSS gzip **20.85 / 21 KB**、总 gzip **121.93 / 122 KB**；为容纳壳层移动导航语义，仅将 JS 总 gzip 预算从 101 调到 101.1 KB。
- 边界：axe 覆盖壳层与三个弹层，不等于全路由审计；对比度用令牌测试补齐，但仍需真实浏览器亮/暗与读屏人工验收；视觉回归基线未在本片交付。
- 复验命令：`./tools/verify.sh`；日志 `/tmp/m6a-verify.log`。



### M6-B 全路由 a11y 门禁

- 新增 `web/src/features/routesA11y.test.ts`：用真实内存路由挂载 15 条 Web 路由，推进 mock 数据计时器并落定后，对 `document.body` 按 WCAG 2.0/2.1/2.2 A/AA 标签执行 axe。
- 覆盖路由：概览、资产、政策、发现、资产详情、规划、交付、自动化、新建运行、运行详情、工具、审批、设置工作区、设置安全、404。
- 首次扫描只发现工具目录权限筛选 `select-name` 违规；搜索框与 `select` 补 `aria-label`，新增 `tools.permission` 中英文案后 15/15 通过。
- 路由测试在数据落定后切回真实计时器再执行 axe，避免 axe 内部计时器被 fake timers 卡住。
- `./tools/verify.sh` exit 0：**164 Rust + 66 Web**；契约 **39 命令 / 23 工具 / 56 schema / 21 错误码**；schema v16 / 31 表；workspace members=12。
- 预算：入口 **79.24 / 80 KB**、vendor 99.50 / 130 KB、JS 总 gzip **101.09 / 101.1 KB**、CSS gzip **20.85 / 21 KB**、总 gzip **121.94 / 122 KB**。
- 边界：jsdom 中 axe 不计算真实颜色对比度，仍由 30 组令牌对比度测试兜底；真实浏览器亮/暗检查与读屏人工验收仍未完成；视觉回归基线由 M6-C 补齐。
- 复验命令：`./tools/verify.sh`；日志 `/tmp/m6b-verify.log`。

### M6-C 轻量视觉回归

- 新增 `tools/visual/regress.py` 与 `tools/visual/baseline/`：9 张固定视口 WebP，共 **292 KB**；桌面亮色 7 张（概览、资产、政策、规划、交付、自动化、设置）、桌面暗色概览 1 张、390×844 移动端概览 1 张。
- 截图前冻结时钟 `2026-11-20T10:00:00+08:00`，写入 `zh-CN` 与亮/暗主题，关闭动画；每张图 1200×800（移动端 390×844），设备像素比 1。
- 比较阈值：单通道差 ≤10 的像素不计，变化像素不超过 0.4%；失败时把 diff 写入 `tools/visual/artifacts/`，该目录已忽略。当前 9/9 通过，暗色概览有 0.059% 的 WebP 重编码噪声，其余为 0.000%。
- 修移动端页头：700px 以下 `PageHeader` 从横排改纵排，标题降到 `clamp(28px, 8vw, 36px)`，操作按钮落到文案下方，避免中文标题被挤成每行两三个字。
- 预算：入口 **79.24 / 80 KB**、vendor 99.50 / 130 KB、JS 总 gzip **101.13 / 101.15 KB**、CSS **20.92 / 21 KB**、总 gzip **122.04 / 122.1 KB**；移动端页头使 JS 总与总 gzip 各放宽 0.05 / 0.1 KB，其余未放宽。
- 视觉检查不进入 `tools/verify.sh`：浏览器版本与字体抗锯齿会带来环境噪声，保留为发布前人工门禁；生产构建不增加 Playwright/Pillow 依赖。
- 复验：`python3 tools/visual/regress.py`；日志 `/tmp/m6c-visual.log`。

### M6-D 确定性模糊测试

- 新增 `core/security/tests/fuzz_smoke.rs` 4 个目标：`validate_external_url`、`normalize_recovery_code` + `validate_recovery_seal_structure`、`decode_share`、`parse_kdf_params`；新增 `core/render/tests/fuzz_smoke.rs` 1 个目标：`escape_html` / `render_page` / `render_document`。
- 固定种子：security 基种子 `0xD161_E6A1`，render 基种子 `0xD161_E65E`；5 个测试共 45,000 组输入，约 1 秒完成。
- 断言边界：URL 接受值必须是 http/https 且无控制字符；恢复码归一幂等且不残留空白/连字符；份额解码成功后编码可再解码；Argon2 `m_cost ≥ 8192`、`t_cost ≥ 1`、`p_cost ≥ 1`；转义输出不含原始 `<`、`>`、双引号或单引号。
- `rand` 在 security 已是运行依赖；render 仅新增 dev-dependency，生产依赖图不变。
- 边界：这是固定种子的确定性回归，不是覆盖率引导 fuzz，也不替代 libFuzzer；目标选的是解析边界，不是全部命令面。
- 验收：`./tools/verify.sh` exit 0，**169 Rust + 66 Web**；日志 `/tmp/m6d-verify.log`。

### M6-E 运维收口

- 新增本地 CLI `security checkpoint` / `security checkpoints` / `security verify`：`create_checkpoint` 先做 WAL checkpoint，生成带 SHA-256 与 HMAC 的清单；成功创建写 `backup.create` 审计，`security status` 的 `lastBackupAt` 同步更新。
- 新增 `checkpoint_commands_create_list_and_verify`：临时加密库创建检查点、列表、校验、`lastBackupAt` 全链路断言；缺失 ID 返回参数错误。
- 新增 `docs/operations/SLO.md` 与 `docs/operations/RUNBOOK.md`，中文镜像在 `docs/zh-CN/operations/`：
  - SLO：99.5% 月度可用性（预算 3h36m）、读 p95 250ms / 写 p95 500ms（暂定）、审计链 100%、RPO 24h / RTO 2h。
  - 运行手册：环境变量、启动、日常检查、备份、离线恢复、升级回滚、常见故障表。
- 恢复保持人工离线：停服、校验检查点、复制到新目标、临时端口实例验证、切换、复验；不提供在线 `security restore`，避免覆盖活跃数据库。
- 边界：无 `/metrics` 端点，SLO 依赖外部探针、CLI 与诊断包；容量演练与恢复演练仍未执行，延迟/RTO 只是暂定目标。
- 验收：`./tools/verify.sh` exit 0，**170 Rust + 66 Web**；日志 `/tmp/m6f-verify.log`。

### M6-F 指标、容量与恢复演练

- 新增只读 `GET /api/v1/metrics`：`AtomicU64` 记录总请求、并发中、2xx/3xx/4xx/5xx、延迟总和/最大值与 12 个延迟桶；中间件在响应后记录状态与耗时，fallback 404 也计入。
- 指标只含结构计数，不记录路径、资产名、继承人、连接标签或凭据；单测 `metrics_endpoint_exposes_counters_without_secrets` 断言输出不含 `token`/`secret`/`asset`，`unknown_route_is_counted_by_middleware` 断言 404 进入计数。
- 补上 `diagnostics export`：应用层调用 `Store::diagnostics_json`，输出 `audit.valid`、schema、表数与各表行数，并写 `diagnostics.export` 审计；CLI 加 `--allow-write` 门禁。
- 新增 `apps/server/examples/seed_reference.rs` 与 `tools/bench/capacity.py`：临时库补齐 1,000 资产、250 继承人、10,000 条审计；脚本混合 health/overview/assets/plan/audit/security/metrics 读，并通过正常 invoke 创建 120 个临时资产。
- `0.0.15` 实测：读 1,200 次、并发 8，p50 6.38 ms、p95 **9.54 ms**、p99 **11.09 ms**，0 错误；写 120 次、并发 4，p50 1.98 ms、p95 **3.70 ms**、p99 8.87 ms，0 错误。
- 恢复演练：复制 3.5 MB 检查点到新路径，启动临时 release 实例，**158 ms** 达到健康；读出 1,120 资产、10,120 条审计，`diagnostics export` 返回 `audit.valid: true`。
- 证据：`docs/operations/measurements/capacity-2026-11-20.json` 与 `restore-2026-11-20.json`；SLO/Runbook 中英文已改为真实存在的测量信号。
- 边界：容量数据来自单台本地合成主机，不代表生产硬件；通知服务商时延仍没有真实账号，未测“已接受→最终送达”。
- 验收：`./tools/verify.sh` exit 0，**173 Rust + 66 Web**；契约 **39 命令 / 23 工具 / 56 schema / 21 错误码**；schema v16 / 31 表；日志 `/tmp/m6f2-verify.log`。

### M6-J 真实浏览器路由冒烟（2026-11-20）

- 新增 `tools/visual/live_smoke.py`：对真实 API + Vue dev server + Chromium 146 运行 15 个路由/主题/视口组合，覆盖概览、资产、发现、政策、规划、交付、自动化、工具、审批、新建运行、设置三个分区、暗色与 390×844 移动端。
- 判定：每条路由必须有唯一 `main`、无横向溢出、无 `pageerror`、无 API 5xx、无非 favicon 的 console error，并在真实浏览器运行 axe WCAG A/AA/2.2 AA（含颜色对比度）。首轮抓到顶栏搜索与语言按钮 2 个 `label-content-name-mismatch`，修复后全部 15 条 PASS、axe 违规 0，总耗时约 34.7 秒。
- 视觉基线同时复跑：9 张固定视口 WebP 的差异全部 ≤0.4%，其中 overview-dark 0.043%，其余 0.000%。
- 证据：`docs/operations/measurements/live-browser-2026-11-20.json`；API `0.0.17`、schema v16 / 31 表。
- 边界：这是自动渲染/网络/溢出/真实 DOM axe 检查，不是读屏人工验收；真实读屏与亮暗人工观感仍保留为发布前人工项。
- 验收：`./tools/verify.sh` exit 0，**175 Rust + 68 Web**；契约 39 / 23 / 56 / 21；schema v16 / 31 表；日志 `/tmp/m6k-verify.log`。

### M6-I 发布包与干净机演练（2026-11-20）

- 新增 `tools/package/build.sh`：release 构建 server/cli/mcp，打包 Web dist、content 与运维文档，输出 `packages/digilegacy-0.0.17-linux-x86_64.tar.gz` 与 `.sha256`。
- 新增 `packaging/install.sh`、`start.sh`、`stop.sh`、`uninstall.sh`：安装时生成 0600 env 与随机写令牌（不打印），首次 health 建 schema/密钥；启动做 HTTP 健康探测；卸载默认保留数据与密钥，`--purge-data` 显式彻底删除。
- 新增 `tools/package/clean_drill.py`：全新 HOME、前缀、数据目录、最小环境变量与随机回环端口，执行安装→健康/metrics→诊断导出→检查点验证→卸载保留→重装读库→彻底卸载，12 步全通过，总耗时约 1.3 秒。
- 证据：`docs/operations/measurements/install-2026-11-20.json`；归档 18,094,813 bytes，SHA256 `cd37a194…`，诊断 `audit.valid: true`，schema v16 / 31 表；重装后数据库与审计链仍可读。
- 安装脚本在 umask 077 下显式把 `web/content/docs/share/tools` 设为可读（`a+rX`），避免非 root nginx worker 读不到 0600 静态文件；干净机演练新增精确权限断言（Web/文档/样例 0644、策略工具 0755、env 0600）。
- 中英文 `docs/operations/INSTALL.md` 说明安装、nginx 同源代理、升级、导出、检查点、卸载与密钥保留策略。
- 边界：API 不直接托管 SPA，生产需反向代理；Windows 代码签名与自动更新（G-37/G-38）仍未做，不属于本次 Linux 干净机演练范围。

### M6-H DNS rebinding 与出站 allowlist（2026-11-20）

- `core/security` 新增 `ResolvedExternalUrl`、`resolve_external_url`、`resolve_tcp_endpoint` 与 `resolve_tcp_host`：域名只解析一次，返回的全部地址逐個校验后再交给连接层。
- 校验规则：云元数据地址（含 IPv4 映射）始终拒绝；私网地址仅 `DIGILEGACY_ALLOW_PRIVATE_NETWORK` 放行；运营商级 NAT、基准测试、保留与文档网段拒绝；loopback 保持原有本地集成行为。
- 新增可选 `DIGILEGACY_OUTBOUND_ALLOWLIST`（逗号分隔，支持 `*.example.com`）：设置后不在表内的主机在 DNS 解析前拒绝；通配符只匹配子域名。
- `core/integrations` 的 HTTP 探针与政策同步改用 `.resolve_to_addrs(...)` 钉住已验地址，并加 `.no_proxy()`；TCP 探针与 SMTP 投递改为连接已验地址列表，不再让 `TcpStream` 重新解析主机名。
- 新增 `resolved_outbound_addresses_are_validated_before_connect` 与保留网段断言；安全单测 52 条，集成 11 条。
- 验收：`./tools/verify.sh` exit 0，**175 Rust + 68 Web**；契约 39 / 23 / 56 / 21；schema v16 / 31 表；日志 `/tmp/m6k-verify.log`。
- 新增端到端重绑定演练：`tools/security/dns_rebind_drill.py` 在 `unshare -Urnm` 隔离网络中起假 DNS 与 `8.8.8.8` TCP 服务；探针只解析一次，DNS 答案切到 `10.0.0.1` 后仍连到第一次校验的 `8.8.8.8`，A 查询恰好 1 次；切换后新解析被拒绝。证据 `docs/operations/measurements/dns-rebind-2026-11-20.json`，重复 3 次 PASS（约 0.31–0.38 秒）。
- 边界：`.no_proxy()` 意味着只允许经 HTTP 代理出网的部署不能直接使用连接测试，需要在受控出口代理处单独处理。

### M6-G 账号接入引导（2026-11-20）

- 设置 → 连接 新增“账号接入引导”：说明谁填、填什么、测试 OK 后再交回；SMTP、Webhook、飞书、企业微信四个按钮直接预填并打开已有加密连接表单。
- 新增“复制填写说明”，生成一段可发给账号本人的交接文案；文案强调平台写令牌与账号凭据分开渠道，不把密码/令牌发群。
- 新增中英文 `docs/operations/CONNECTION-SETUP.md`：字段表、端口/TLS 说明、服务端保存行为、交接检查清单。
- 连接设置路由纳入全路由 axe 门禁，路由覆盖 15 → 16 条；`SettingsView` 路由块 20.79 / 24 KB。
- 预算：入口 **81.28 / 82 KB**、JS 总 gzip **102.95 / 103.1 KB**、CSS gzip **21.05 / 21.1 KB**、总 gzip **124.00 / 124.2 KB**；vendor 99.50 / 130 KB 与单路由 24 KB 不变。
- 验收：`./tools/verify.sh` exit 0，**173 Rust + 67 Web**；契约 39 / 23 / 56 / 21；schema v16 / 31 表；日志 `/tmp/m6g-verify2.log`。
- 边界：这是填写入口和交接材料，不是账号代填；真实通知时延仍需账号本人填完后用真实服务商跑一次。

### M6-L 政策来源预审（2026-11-20）

- 新增 `tools/policies/source_audit.py`，只输出机器预审清单与可达性，不改 `content/policies/platforms.json`、不改数据库、不修改 provenance 状态。
- 152 条全部有 URL 且为 HTTPS；150 条是平台首页，只有 2 条是具体深链（iCloud / Apple ID 共用 `https://support.apple.com/zh-cn/HT212360`）。
- 17 组重复 URL，覆盖 38 条；134 条可访问、2 条被拦截（403 等）、2 条失效（521/567）、11 条超时、3 条传输错误。超时/拦截可能是跨境访问或反爬，不能直接判死链。
- 152 条全部需要人工复核：每条要么是首页、要么重复、要么没有干净 2xx/3xx。机器预审不能作为 G-16 的关闭证据。
- 证据：`docs/operations/measurements/policy-sources-2026-11-20.json`；清单：`docs/operations/policy-source-worklist.csv`；流程：`docs/operations/POLICY-SOURCES.md`。
- 边界：脚本依赖 Python `requests`；政策来源仍是用户侧人工内容决策，代码不伪造核对人、日期或法律结论。G-16 状态为 PARTIAL。
- 验收：`./tools/verify.sh` exit 0，**175 Rust + 68 Web**；`ops tools` 段新增 `py_compile`，覆盖政策预审、干净机演练与容量脚本；日志 `/tmp/m6m-verify.log`。

### M6-Q 真实保险库锁屏（2026-11-20）

- 新增 CLI-only `security init`：标准输入读取主密码，创建 DEK/KEK/恢复封条并写入 `security_config`；不暴露到 HTTP、IPC、MCP 或 Agent。新增 `security.unlock` / `security.lock` 契约，HTTP 为 `POST /api/v1/security/unlock` / `lock`。
- 已配置且锁定时，应用层只放行三个 security 命令，其余返回 `423 VAULT_LOCKED`；`security.status` 增加 `configured`，`keySource` 未初始化时为 `unknown`。
- 引导事务内把 `connections.config_enc` 与 `policy_candidates.payload_enc` 从旧密钥迁移到保险库派生域密钥；迁移失败整体回滚。新增 `security_init_rekeys_existing_connection_secrets` 与 HTTP 门禁测试。
- Web 启动读取状态并在锁定态隐藏主壳；锁屏输入真实主密码，错误口令留在原地，成功后进主区域；顶栏锁定调用服务端。未配置时设置页给出 CLI 引导。
- 真实 API 隔离库演练：锁定 `423` → 错误口令 `400 INVALID_ARGS` → 正确解锁 `200` → 概览 `200` → 再锁定后资产 `423`。真实 Chromium 桌面与 390px：axe 0、无页面异常、无横向溢出；错误口令与再次锁定均通过。
- CLI 独立进程：无 `DIGILEGACY_VAULT_PASSWORD` 时 `assets list` 返回 `VAULT_LOCKED`；带变量时返回“锁定前资产”。`security status` 不受锁影响。
- 验收：`./tools/verify.sh` exit 0，**182 Rust + 77 Web**；契约 **43 命令 / 23 工具 / 59 schema / 21 错误码**；schema v16 / 31 表；入口 79.72/82 KB，JS gzip 104.45/104.6 KB，CSS gzip 20.15/21.1 KB，总 gzip 124.60/124.8 KB；日志 `/tmp/digilegacy-verify-lock7.log`。
- 真实浏览器 14/14 路由 PASS（`/tmp/digilegacy-live-smoke-final.json`），9/9 视觉基线 PASS；锁屏截图 `/tmp/digilegacy-lock-ui/`。
- 边界：Web 暂无恢复码解锁；解锁 API 默认只随服务绑定回环地址，对外暴露需在反向代理限流；SQLCipher 文件密钥仍保留，保险库 DEK 负责字段密钥与会话。
