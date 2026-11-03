# AI / Agent 集成边界与开发指南

## 目标

DigiLegacy 的 AI 不是内置云服务，也不是允许模型替用户操作电脑的通用 Agent。当前分成两条能力路径：

| 路径 | 用途 | 交付 |
|---|---|---|
| AI 助手 | 单轮政策/规划问答 | 文本答案，落入本地 `ai_chat_log` |
| 内置 Agent | 多轮本地资料检索与草稿编排 | 工具步骤时间线 + 人工核对的草稿 |

两条路径共用用户自配的 OpenAI-compatible endpoint、model、API Key；没有 endpoint 时都必须优雅降级，不伪装成可用。

## Agent 循环

`src-tauri/src/agent.rs` 实现固定边界的工具循环：

1. 命令层检查 Agent 配置、API Key、任务长度、数据是否处于占位符锁定态。
2. Rust 发送 `messages + tools` 到用户配置的 `/chat/completions`。
3. 模型返回 `tool_calls` 时，Rust 在本地执行工具，把结果作为 `tool` 消息回传。
4. 没有工具调用时，响应视为最终回答；最多 8 次模型调用，单轮最多 6 个工具调用。
5. `run_agent_task_streaming` 通过 Tauri `Channel<AgentEvent>` 推送 `started`、`tool_start`、`tool_end`、`final`、`truncated` 事件；最终结果仍通过命令响应返回。

### 当前工具

| 工具 | 数据 | 写入 |
|---|---|---|
| `query_assets` | 平台、类别、处置方式、继承人是否已指定 | 否 |
| `query_platform_policy` | 内置 152 平台政策 | 否 |
| `get_planning_stats` | 规划统计与待办键 | 否 |
| `run_readiness_drill` | 家人执行演练的元数据结果 | 否 |
| `get_trigger_status` | DMS 当前阶段与签到状态 | 否 |
| `draft_heir_note` | 给继承人的纯文本草稿 | 否；只返回文本 |

Agent 工具集中没有 `get_asset_credential`、密码、恢复码、API Key、身份联系方式或任意文件/shell 工具。

## 数据边界

### 出境前

- AI 助手使用 `ai_assist::build_safe_payload` 白名单构造 payload。
- Agent 的资产查询只返回规划状态，不返回账号标识、继承人姓名/联系方式、备注、凭据。
- Tool result 回传模型前加 `[UNTRUSTED_TOOL_RESULT]` 标记。工具返回内容是资料，不是系统指令，不能改变安全规则。
- AI/Agent endpoint 可按兼容性使用 `http://` / `https://`；ureq 与 Agent 请求都禁用重定向，避免用户自配端点把令牌转发到第二个主机。零内容 heartbeat 是独立的产品契约：用户配置的 heartbeat endpoint **仅允许 HTTPS**，参考 server 产品部署也必须使用 TLS；仓库行为测试才通过 `HEARTBEAT_TEST_HTTP=1` 启用本机 HTTP fixture，不能复制到用户配置或生产部署。
- 模型响应设 1 MiB 上限，工具参数设 8,000 字符上限，工具结果设 6,000 字符上限，最终答案设 20,000 字符上限。

### 写入边界

Agent 没有写资产、写档案、写凭据、改密码、发邮件、执行 shell、打开外部链接的工具。`draft_heir_note` 仅返回草稿，并明确提示用户人工核对后自行保存。

审核管线继续遵循更严格的不变式：AI 只能产生 `Pass` / `Flag` / `Unknown`，只有人工 verdict 能生成 `Approved`。

## 前端接线

ReportView 在助手已启用时展示 Agent 面板：

- 浏览器演示环境不启动 Agent，显示桌面版边界提示。
- Tauri 环境使用 `Channel` 接收步骤事件。
- 结果明确标为 Agent 草稿，不自动写入任何数据。
- 单轮助手仍保留，Agent 不可用时不影响已有问答功能。

## 新审核场景

`review_heir.rs` 注册 `heir-comprehension` 场景，审查系统生成的继承人交付物，而不是替 AI 审核法律有效性。Strict 模式包含六项检查：

1. 每条资产有可执行下一步；
2. 无内部术语或占位符；
3. 有依赖的步骤标明顺序；
4. 无无法兑现的时间承诺；
5. 无凭据/密码明文；
6. 未暗示可以替代律师、公证处或平台人工审核。

## 接入新工具的要求

新增工具必须同时满足：

1. 只读或草稿；不得直接写业务数据；
2. 在 `tool_definitions()` 和 `execute_tool()` 同时登记；未知工具默认拒绝；
3. 对 arguments 做字段、长度、控制字符校验；
4. 对结果做截断，且以不可信资料标记回传；
5. 不返回 `FORBIDDEN_FIELDS` 或其值；
6. 加一个行为测试：证明敏感数据不出境、异常输入被拒绝、工具结果可见；
7. 更新 mock、i18n、ReportView 测试与文档。

## 模型兼容性

当前协议依赖 OpenAI-compatible `chat/completions` 和 `tools` / `tool_calls`。模型不支持 tools 时，Agent 会把不带工具调用的响应视为最终回答；不会擅自把模型当作可执行 Agent。

支持本地模型时，优先使用用户本机提供的 OpenAI-compatible endpoint（例如用户自行运行的本地服务），不把模型文件或推理服务打进 DigiLegacy 安装包。这样保持安装包大小、无服务器和无遥测契约。

## 已知边界与下一步

- 当前 Agent 任务没有持久化完整步骤日志；步骤在本次任务的响应与 UI 时间线中可见，最终文本进入已有 AI 问答历史的路径仍是单轮助手路径。若要长期审计，应新增 `agent_run_log`，但需设计保留期限与清除策略。
- 流式助手支持 SSE；Agent 工具循环目前以每轮完整 JSON 响应为单位。下一步可为 Agent 增加可取消的任务句柄，而不是直接复用 SSE。
- 目前只查本地政策和本地资产。若未来加入外部网页检索，必须单独设计域名白名单、缓存、来源快照、SSRF 防护、隐私确认和引用校验，不能直接把任意 URL 工具暴露给模型。
