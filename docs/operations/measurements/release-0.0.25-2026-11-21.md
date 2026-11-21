# DigiLegacy 0.0.25 发布验收报告

日期：2026-11-21  
版本：0.0.25  
代码提交：`e943821`  
分支：`main`

## 结论

0.0.24 的本地模型链路已经完整真测，但“全站全部正常功能”没有逐条跑完。本轮补测全站正常路径时发现一个真实缺口：契约里声明了 `POST /api/v1/diagnostics/export`，应用层也实现了方法，但服务端没注册路由，`dispatch` 也没有 `diagnostics.export` 分支，导致 HTTP 端点返回 404，文档里的 `invoke diagnostics.export` CLI 命令也不可用。0.0.25 修复该缺口，并完成全站正常路径、保险库生命周期、真实浏览器与发布包复验。

## 修复

- `apps/server/src/main.rs`：注册 `POST /api/v1/diagnostics/export`，新增带鉴权的处理器。
- `core/application/src/lib.rs`：`dispatch` 增加 `"diagnostics.export" => self.diagnostics_export()`。
- 新增 HTTP 路由回归测试，真实调用端点并校验 `audit.valid` 与 schema 版本；应用层单测改为走 `dispatch`。
- 该修复同时恢复文档承诺的 `digilegacy-cli invoke diagnostics.export --allow-write`。

## 全站正常路径覆盖

| 模块 | 覆盖内容 | 结果 |
| --- | --- | --- |
| 契约路由 | 44 条 HTTP 命令全部有注册路由；静态比对无缺失 | PASS |
| 基础 | `/health`、`/metrics`、`/commands`、`/overview`、`/audit` | PASS |
| 资产 | 创建、列表、详情、PATCH、批量新增（created=1/skipped=1）、发现分类、关键字搜索 | PASS |
| 规划 | 读取空态、设置持有人、设置 2 位继承人（首位 executor）、复核 | PASS |
| 触发链 | 启用、签到、评估、事件列表 | PASS |
| 政策 | 列表 16 分类、详情、provenance 置 verified、候选列表 | PASS |
| 政策同步 | 本地 HTTP feed 拉取 1 条 → 入队 1 个候选 → 人工批准 → 政策库出现新分类 | PASS |
| 交付 | report/checklist/legal/toolkit 四种导出；job 轮询到 succeeded；下载 HTML 839–1496 B | PASS |
| 诊断 | `POST /api/v1/diagnostics/export` 200；`audit.valid=true`；schema 16；CLI `invoke` 同命令 200 | PASS |
| 连接 | 保存加密连接、列表、本地 webhook 实测 HTTP 200/2 ms、删除 | PASS |
| 通知 | 列表、空配置 send（configured=false/pending=0）；SMTP 实际投递由单测覆盖 | PASS |
| Agent 读 | `overview.get` 运行、详情、列表、usage | PASS |
| Agent 写 | `assets.create` 进入 pending approval；批准后 job 恢复、运行成功、资产落库 | PASS |
| Agent 控制 | 待审批运行取消、已取消运行重试、再次取消 | PASS |
| 任务 | 导出 job 排队后立即取消，状态 `CANCELLED` | PASS |
| 调用与事件 | `/invoke` 的 `app.overview`、`assets.list`；SSE `/events?jobId=` 首个 snapshot | PASS |
| 保险库 | 初始化、启动锁定、锁定态读取 423、错误口令拒绝、正确解锁、写入、锁定、恢复码解锁、再次锁定/解锁 | PASS |
| 检查点 | CLI 创建 311,296 B 检查点、列表、校验 `verified=true` | PASS |
| 本地模型 | 0.0.24 已完成 2B 真实下载、启用、规划、聊天、错误路径与 Web 按钮；0.0.25 未改该链路 | PASS（沿用 0.0.24） |

## 门禁与浏览器

| 项目 | 结果 |
| --- | --- |
| Rust workspace | 191 passed / 0 failed |
| Web | 79 passed / 0 failed（12 个测试文件） |
| `./tools/verify.sh` | exit 0；契约、架构、Rust、Web、web-budget 全部 OK |
| 真实 Chromium | 16/16 PASS；0 failures；无横向溢出；axe 0 违规 |
| API 版本 | `/health` 返回 0.0.25；schema 16；31 张表 |

## 测试边界

- 真实外部 SMTP/Webhook/飞书/企微账号仍未接入；通知实际投递由本地假 SMTP 单测覆盖，外部时延仍待账号。
- 政策同步使用本地 HTTP feed，未接真实外部政策提供方；签名校验路径由现有集成测试覆盖。
- 触发链只验证了启用、签到、评估和 stage 0 事件；跨 30/60/90 天的 stage 1→4 推进未在真实时间等待中重跑，阶段逻辑由领域单测覆盖。
- 1B 未真实下载：按用户要求只安装和使用 2B。
- 双模型热切换与超过 8K 上下文未在 0.0.25 重测；0.0.25 未改本地模型代码。
- 浏览器巡检覆盖 16 条主路由的渲染、溢出与 axe，不代替逐表单交互测试。

## 发布物

- 包：`packages/digilegacy-0.0.25-linux-x86_64.tar.gz`，18,445,456 B（约 18 MB）
- SHA256：`41d0896ffb85e4e6fd657395c681f38e4adb74caf17495c5972cdfb819e867d6`
- 解包启动：`/health` 返回 0.0.25；schema 16；31 张表
- 包内本地模型：1B/2B 均 `missing`，`active` 为空；无模型文件、无 `active.json`
- 包内诊断导出：`POST /api/v1/diagnostics/export` 200，`audit.valid=true`

## 与 0.0.24 的关系

0.0.24 的发布包在本地模型链路完整，但全站功能审计发现诊断导出 HTTP 路由缺失。0.0.24 包不应作为全站验收版本；0.0.25 修复后替代 0.0.24 包。
