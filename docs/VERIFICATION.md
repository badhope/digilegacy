# 验证记录

本文件记录最终验证证据，避免"看起来绿"与"真的绿"混淆。历史轮次保留当时的数字，但不代表当前工作树或本轮门禁结果。

## 2026-11-03 设计与 harness 轮次（v0.3.0）：当前基线

本轮做了三件事：设计系统与关键页面视觉升级、harness 能力落地（工具注册表 + 权限分级 + 检查点回滚）、双语截图与文档同步。下表为本轮实跑结果。

| 命令 | 本轮基线结果 | 证据边界 |
|---|---|---|
| `npm run typecheck` | PASS | `vue-tsc --noEmit` |
| `npm run test:coverage` | **34 files / 468 tests passed**；83.05% statements、79.05% branches、75.81% functions、84.59% lines | 前端单测与覆盖率；覆盖率阈值全部达标 |
| `npm run build` | PASS；`ls dist/assets | grep '^mock-'` 无输出 | 生产前端构建 |
| `cargo fmt --check` | PASS | 仅格式检查 |
| `cargo clippy --all-targets -- -D warnings` | PASS | 无 warnings；唯一提示是上游 imap-proto 的 future-incompat |
| `cargo test --locked` | **274 passed / 1 ignored**（276 collected） | 通过 MSVC `cl.exe`；新增工具注册表 5 项 + 检查点 4 项 |
| `PW_CHANNEL=msedge npx playwright test` | **85 passed** | 全量 E2E；新增 `harness.spec.ts` 5 项 |
| `node scripts/check-doc-facts.mjs` | PASS | 276 Rust、468 前端、87 IPC、17 表、152 平台、85 E2E |

### 本轮新增能力

1. **命令面板（⌘K / Ctrl+K）**：键盘优先的全局导航，覆盖 9 个页面 + 主题/语言/锁定三个动作。头部有可见入口，不依赖用户读过文档。无障碍实现为 `dialog` + `combobox` + `aria-activedescendant`。
2. **检查点与一键回滚**：命名快照，粒度是「按下按钮那一刻」，与每日自动备份互补。回滚走既有认证式导入（源口令 + wrapped_dek + AEAD），且回滚前自动打 `pre-restore` 检查点，保证回滚本身可撤销。上限 12 个，自动检查点优先淘汰。
3. **工具注册表与权限分级（`agent_tools.rs`）**：工具定义、执行、权限绑定在同一条 `ToolSpec` 上，`tool_definitions()` 与 `execute()` 不可能漂移。权限是枚举（`Read` / `Draft`）并在执行入口统一拦截，5 个测试断言「每个工具都有权限标注」「没有档位能写库」。
4. **设计系统增强层（`design-upgrade.css`）**：模块化字号阶梯、4px 间距阶梯、五档高度阶梯（深色模式单独取值）、语义化表面令牌、动效时长令牌。新增环形指标、分段进度条、骨架屏、规范空态。

### 本轮修复的界面缺陷

- 环形指标里百分比与标签重叠（`place-items: center` 把两行叠在一起）。
- 命令面板列表溢出时页脚被挤出面板（面板未限高、列表未用 flex 填充）。
- 检查点在浏览器预览里不可见（`loadCheckpoints()` 误放进 `if (isTauri)` 守卫）。
- E2E 主题切换用 `header button` 位置选择器，头部新增按钮后点错元素（改为 `data-testid`）。

---

## 2026-11-02 仓库治理与双语展示轮次：历史基线

本轮做了三件事：清理仓库结构（分支、对象库、分支残留）、修复界面缺陷（图标尺寸、报告标题字距、DMS 与仪表板口径矛盾、送审字段中英混排）、把对外展示统一为英文主 + 中文可切换。下表是本轮实跑结果。

| 命令 | 本轮基线结果 | 证据边界 |
|---|---|---|
| `npm run typecheck` | PASS | `vue-tsc --noEmit` |
| `npm run test:coverage` | **32 files / 451 tests passed**；83.36% statements、78.31% branches、76.36% functions、84.84% lines | 该轮历史数字；不是 E2E，也不是 Rust |
| `npm run build` | PASS；`ls dist/assets | grep '^mock-'` 无输出 | 生产前端构建；不等于 Windows 安装包构建 |
| `npm run build:demo` | PASS | 浏览器 mock 数据构建；数据写入未加密 `localStorage`，不连接桌面 Rust 后端 |
| `node scripts/check-doc-facts.mjs` | PASS | 267 Rust 声明测试、451 前端源码用例、83 IPC、17 表、152 平台、80 E2E 与文档一致；含平台分类双语完整性检查 |
| `node scripts/check-policy-provenance.mjs` | PASS | 152 条中 147 条显式未核实、5 条有出处缺复审日期；棘轮未变差 |
| `node server/heartbeat-server.test.mjs` | PASS | 本地心跳服务器行为 |
| `cargo fmt --check` | PASS | 仅格式检查，不编译 |
| `cargo clippy --all-targets -- -D warnings` | PASS | 通过 MSVC `cl.exe`；唯一提示是 imap-proto 的 future-incompat 警告（上游依赖） |
| `cargo test --locked` | **265 passed / 1 ignored**（266 collected） | 通过 MSVC `cl.exe` |
| `PW_CHANNEL=msedge npx playwright test` | **80 passed** | 系统 Edge 通道；全量 E2E 实跑 |
| `npm run lint` | NOT CONFIGURED | `package.json` 没有 `lint` script；未把类型检查冒充 lint |

### 本轮修复的缺陷

1. **法律模板页被一个图标撑满整屏**：`@vicons` 的 SVG 只有 `viewBox`、没有宽高，在流式容器里按默认 300×150 渲染。加了全局内联图标兜底尺寸，并给该提示条单独的 flex 布局。
2. **报告标题单词粘连**：英文标题写成 `'D I G I T A L   L E G A C Y   P L A N'`，HTML 折叠连续空格后变成 `DIGITALL EGACYPLAN`。改为正常文案 + CSS `letter-spacing`。
3. **DMS 页与仪表板口径矛盾**：仪表板把「无邮件通道」列为 blocker，DMS 页却显示「一切安好」。现在 DMS 页在同一张状态卡里给出投递警告，判定与后端 `trigger_ready = dms_enabled && trigger_channel == "smtp"` 一致。
4. **送审字段中英混排**：Rust 侧 `payload_fields_for` 返回中文文案，英文界面原样打印。改为返回稳定标识 `payload.field.*`，由前端按当前语言渲染。
5. **报告继承人联系方式为 null 时渲染 `(null)`**：改为区分「有联系方式 / 未留联系方式」两种文案。
6. **平台分类未本地化**：16 个分类中 4 个缺译名，英文界面露出中文分组标题。补齐并加了门禁。
7. **演示端完成度恒不高**：mock 里 `triggerReady` 写死 `false`，导致四维完成度的触发链一维永远为 0，与后端契约不一致。
8. **测试固化了缺陷**：`HeirGuideView.test.ts` 断言分类筛选按钮文字是原始中文数据值，等于把「界面直接打印数据串」当成期望行为。

### 仓库结构变更

- 删除 5 个本地分支与 1 个远程分支，本地/远程统一为 `main`。
- 清理 `refs/original/*`（`git filter-branch` 遗留）与全部 reflog，`git gc` 后 `.git` 从 358 MB 降到 29 MB。
- 顶层临时产物（`e2e_run.log`、`out/`、文件名损坏的心跳日志）移出仓库树。
- 新增 `.github/CODEOWNERS`，把加密、备份、外连配置与门禁脚本标为必须维护者审阅。
- 截图脚本改为双语出图：中文留 `docs/screenshots/`，英文落 `docs/screenshots/en/`，并断言页面标题与目标语言一致（截错页直接失败）。

### 已知未验证项（与本轮无关，仍然成立）

- 外部政策链接的真实内容未联网核验；147 条未核实是如实标注，不是遗漏。
- 线上站点、GitHub Actions、Release 产物未在本轮验证（本轮只推送 GitCode 与 Gitee）。
- 真实 SMTP/IMAP/AI endpoint 未实测；Tauri 原生窗口的导入导出未实测。

---

## 2026-11-01 文档与真实功能审计：历史基线

本轮目标是把项目简介、README 与实现边界对齐，并记录当前工作树的可复现实证据；不把历史 Rust 或发布结果冒充本轮证据。前端单测、浏览器 E2E、覆盖率收集和 Rust 门禁已在本轮实跑；网络来源真实性核验未运行，因为本轮不联网。

| 命令 | 本轮基线结果 | 证据边界 |
|---|---|---|
| `npm run test:coverage` | **32 files / 451 tests passed**；83.36% statements、78.31% branches、76.36% functions、84.84% lines | 该轮历史数字；不是 E2E，也不是 Rust |
| `npm run typecheck` | PASS | `vue-tsc --noEmit` |
| `npm run build` | PASS；产物无 `mock-*` 分块 | 生产前端构建；不等于 Windows 安装包构建 |
| `npm run build:demo` | PASS | 浏览器 mock 数据构建；数据写入未加密 `localStorage`，不连接桌面 Rust 后端 |
| `node server/heartbeat-server.test.mjs` | PASS | 本地心跳服务器行为；不代表用户自配外部端点可达 |
| `node scripts/check-doc-facts.mjs` | PASS | 源码推导的 267 Rust 声明测试、451 前端源码用例、83 IPC、17 表、152 平台、80 E2E 清单与文档一致；清单数字不等于 Rust 测试已在本机执行 |
| `node scripts/check-policy-provenance.mjs` | PASS | 政策条目溯源棘轮；不等于所有外部链接内容已人工核实 |
| `cargo fmt --check` | PASS | 正确 crate 根目录 `digilegacy/src-tauri` 执行；仅格式检查，不编译 Rust |
| `cargo clippy --all-targets -- -D warnings` | PASS | 通过 MSVC `cl.exe`；无 warnings |
| `cargo test --locked` | **265 passed / 1 ignored**（266 tests collected） | 通过 MSVC `cl.exe`；新增 authenticated backup 与锁定态回归均通过 |
| `PW_CHANNEL=msedge npx playwright test` | **80 passed** | 系统 Edge 通道；当前工作树普通 E2E 已实跑 |
| `PW_CHANNEL=msedge E2E_COVERAGE=1 npx playwright test` + `node e2e/collect-coverage.mjs` | **80 passed**；coverage 31 files | 覆盖率收集器与 fixture 失败路径已单独验证；不是 Rust/Tauri 原生窗口 |
| `npm run lint` | NOT CONFIGURED | `digilegacy/package.json` 没有 `lint` script；本轮未把类型检查冒充 lint |

### 当前实现边界核对

- 数据库 schema 当前为 `SCHEMA_VERSION = 7`，`ai_chat_log` 属于 v7 schema；这不是备份格式。
- 导出备份当前为 **format v5**，包含安全配置、恢复封印/包元数据、继承人、审核记录、SMTP、保管位置、助手配置、心跳与政策补丁等；导入默认合并，覆盖模式显式选择，并由命令层在导入前创建快照。
- AI 助手与 Agent 没有内置服务；只有用户配置 OpenAI-compatible endpoint 后才会建立外部连接。工具循环有界、无 shell/任意 URL/发信/凭据写入工具，浏览器演示不启动 Agent。
- 心跳、SMTP、IMAP 也是用户配置的可选连接；默认不产生这些外部请求。零内容心跳只发送用户配置的令牌到用户提供的端点，不是 DigiLegacy 托管服务。
- 浏览器 demo 使用真实 Vue 组件与 mock backend，但 mock 数据未加密存储在 `localStorage`，只用于展示，不承载真实数据；生产构建的分支检查确认不包含 mock 分块。
- 本轮没有访问远程仓库、Release、Actions 或任何托管平台；远程发布状态与线上产物未验证。

## 历史验证：2026-10-31 第四/五轮：内置 Agent + 前端信息架构重构

### 内置 Agent（受限本地工具循环）

| 项 | 实现 | 安全边界 |
|---|---|---|
| 工具协议 | OpenAI-compatible `tool_calls`；arguments 支持字符串与对象；外层 `id` 缺失时本地生成并同步回 assistant 消息 | 未知工具默认拒绝；每轮最多 6 个调用 |
| 工具集 | `query_assets` / `query_platform_policy` / `get_planning_stats` / `run_readiness_drill` / `get_trigger_status` / `draft_heir_note` | 无写资产/档案/凭据/恢复码工具；无 shell / 任意 URL / 发信 |
| 资源上限 | 8 次模型调用（最后一轮只收敛）、1 MiB 响应、8k 工具参数、6k 工具结果、20k 最终文本 | 达到上限返回 `truncated`，不静默丢弃已完成步骤 |
| 数据边界 | `query_assets` 只返回平台/类别/处置/继承人状态；工具结果回传前加 `UNTRUSTED_TOOL_RESULT` 标记 | 账号标识、姓名、联系方式、备注、凭据不出境 |
| 执行线程 | Tauri `spawn_blocking`，事件经 `Channel<AgentEvent>` 推送 | 网络循环不占用 UI 线程 |
| 前端 | ReportView Agent 面板：任务输入、步骤时间线、草稿结果、桌面版边界提示 | 浏览器演示不启动 Agent |

**安全审计发现并修复**：`review_heir` 的首版把继承人姓名、关系与紧急联系人写进 AI payload，违反既有 `FORBIDDEN_FIELDS` 设计。已改为「已指定/未指定 + 计数」，并加 `payload_never_contains_credentials` 的 PII 断言。

### 前端信息架构重构

| 变更 | 之前 | 现在 |
|---|---|---|
| 侧栏导航 | 9 个功能平铺 | 5 组按动线：开始 / 盘点资产 / 安排身后事 / 交给家人 / 发出前核对 |
| 分组依据 | 功能模块 | 读者与阶段：继承人向导归「交给家人」，法律模板归「安排身后事」，审核归「发出前核对」 |
| 首页首屏 | 完成度百分比与统计卡 | 先给「下一步该做什么」（待办可点击直达），统计与完成度后置 |
| 正文可读性 | 14px / 默认行高 | 15px / 1.7 行高；卡片标题 17px、副标题 14px |
| 标题竞态 | 用 path 反查 navRoutes，首帧回退 appName | 优先 `route.meta.titleKey` |
| E2E 并发 | 本机 4 worker 导致 Chromium OOM（code=134） | 本机固定 2 worker；CI 仍 1 |

### 本轮真实执行

| 命令 | 结果 |
|---|---|
| `cargo test --locked` | **243 passed / 1 ignored**（当时源码声明 245；历史快照） |
| `cargo fmt --check` / `clippy -D warnings` | PASS / 0 |
| `npm run test:coverage` | **441 passed / 32 files**，83.5/78.31/76.29/85.06（阈值 78/78/71/79） |
| `npx vue-tsc --noEmit` | PASS |
| `PW_CHANNEL=msedge npx playwright test` | **80 passed** |
| `npm run build` | PASS，无 `mock-*` 分块 |
| `check-doc-facts` | exit 0 |
| `check-policy-provenance` / `heartbeat-server.test` | PASS / PASS |

### 明确未做

- Agent 任务取消（AbortController 语义）未实现；当前以 8 轮与超时兜底。
- Agent 完整步骤未持久化到长期审计表；只在本次响应与 UI 时间线可见。
- 外部网页检索未开放（需域名白名单、SSRF 防护、来源快照、引用校验，单独设计）。

## 2026-10-30 第三轮：AI 结合深化

原有 AI 能力只有「政策问答」一点：一个 ask 命令、一次成型返回、不留痕。本轮把 AI 从「单次问答」升级为「贯穿产品承诺的三条线」，全部遵守既有安全边界（白名单出境 / AI 永不放行 / 无端点优雅降级 / 不内置服务）。

### 变更摘要

| 变更 | 内容 | 证据 |
|---|---|---|
| **新审核场景「继承人读得懂」**（heir-comprehension） | 审「系统生成的交付物」：家人执行包正文、继承人分配说明、开头说明。Strict 模式 6 检项（可执行下一步/无术语/步骤顺序/无时间承诺/无凭据明文/无替代公证暗示）。注册进 review_profiles 注册表，前端自动出现 | `review_heir.rs` 5 组测试；`review_profiles` 路由 + payload_fields；mock 同步 |
| **SSE 流式助手** | `call_chat_api_streaming`：逐行读 `data:` 增量，`[DONE]` 终止，注释行/解析失败跳过；命令层 `ask_assistant_streaming` 用 Tauri Channel 逐字推送；前端流式优先 + 非流式回退（非 Tauri 环境直接走旧命令） | `sse_delta_extraction_from_lines`（[DONE] 后不得消费）；ReportView 24 测试全绿 |
| **问答本地留痕**（ai_chat_log，schema v7） | 问/答/模型/时间落库；`list_ai_chat` 倒序、`clear_ai_chat` 需确认框；历史卡片独立于助手开关（助手未启用也能回看清除）；长度上限 2k/20k | `ai_chat_log_roundtrip` + `rejects_oversized`；前端「回答后刷新历史」「清空需确认」两组测试 |
| **白名单出境回归补强** | 流式与一次成型共用同一个 `build_safe_payload`；FORBIDDEN_FIELDS 全量反向断言（字段名 + 值双重检查） | `streaming_payload_never_contains_forbidden_fields` |

### 安全边界不变式（逐条对应实现）

1. AI 永不独自放行：流式只用于问答，不触碰审核管线终态。
2. 白名单出境：流式 payload 走 `preview_payload` → 同一白名单函数。
3. 不内置服务：无端点时 streaming 命令与旧命令同样 fail-fast 并提示配置。
4. 本地留痕：问答只存文本，不含资产快照；用户可见可清除。
5. 占位符拦截：锁定态下流式与一次成型同样拒绝提问。

### 本轮真实执行

| 命令 | 结果 |
|---|---|
| `cargo test --locked` | **229 passed / 1 ignored**（源码声明 231） |
| `cargo fmt --check` / `clippy -D warnings` | PASS / 0 warnings |
| `npm run test:coverage` | **434 passed / 32 files**，83.48/78.26/76.18/84.97（阈值 78/78/71/79） |
| `npx vue-tsc --noEmit` | PASS |
| `npm run build` | PASS，无 mock-* 分块 |
| `PW_CHANNEL=msedge npx playwright test` | **80 passed** |
| `check-doc-facts` | exit 0（Rust 231 / 前端 434 / E2E 80 / IPC 81 / 表 17 / 平台 152） |
| `check-policy-provenance` / `heartbeat-server.test` | PASS / PASS |

### 已知波动与明确未做

- `heartbeat::reachable_local_server_records_ok` 在并行高负载下曾出现过一次偶发超时（本机资源竞争，重跑即绿；该测试自带 3 次有界重试）。CI 隔离环境下无此现象。
- SSE 真实端到端联调需用户自配端点（模块测试覆盖解析/边界；网络层 ureq 与 call_chat_api 共用，后者已有真实验证路径）。
- 流式命令未做取消（中断）：当前超时 60s 兜底；可中断属下一轮候选。

## 2026-10-30 第二轮：能力收口（E2E 实跑 / 打包实跑 / 两层密钥 / KDBX / IMAP）

上一轮「明确未做」在本轮全部落地或给出实测结论。

### 变更摘要

| 变更 | 类型 | 证据 |
|---|---|---|
| **两层密钥**（随机 DEK + 密码派生 KEK 包裹，`wrapped_dek` 列） | 架构 | `change_password_does_not_touch_ciphertext`：改密前后密文逐字节相同，O(n)→O(1) |
| legacy 库一次性迁移到 v2（HKDF 域分离），`reencrypt_all_to_v2` 仅此调用 | 兼容 | `test_change_password_and_backup_roundtrip` 全绿 |
| 恢复重设改为 O(1)（同一 DEK 换 KEK 包裹） | 性能 | `reset_security_reencrypts_assets_and_profile` 重写为 DEK 语义 |
| **HKDF 域分离基础** | 安全 | `field` 域已用于字段加密；`hkdf_domains_are_separated` 锁定 field/backup/kit 标签互斥，但 backup/kit 生产路径仍是后续切换项 |
| 备份升 **v5**：携带 `wrapped_dek` 与恢复包元数据 | 换机 | `cross_device_recovery_from_shares_and_backup` 全链路（生成→备份→导入→3 份额+口令解锁→同一 DEK） |
| **KDBX 4 长期归档导出**（`keepass` crate save_kdbx4） | 能力 | 魔数/版本断言 + 同口令重开 + 错误口令拒绝，家人用 KeePassXC 可直接打开 |
| **IMAP 入站签到**（回邮件=我还安好；白名单发件人 + uid 去重 + lookback 窗口） | 能力 | `imap_checkin` 3 组测试 + 后台 30 分钟轮询接线 |
| 遗留死代码清理（`reencrypt_heirs`、`invalidate_on_rekey`、`set_security_config_db` 薄包装） | 整洁 | clippy -D warnings 0 |

### SQLCipher（整库加密）实测结论：本机不可行，列 CI 路径

| 尝试 | 结果 |
|---|---|
| `bundled-sqlcipher` + `OPENSSL_DIR=C:\msys64\ucrt64` | cl.exe 拉入 MSYS GCC 风格头，`corecrt_wstdio.h` 语法错误 100+ |
| `bundled-sqlcipher-vendored-openssl` | openssl-src 需 native Windows perl（本机仅 msys perl），`Configure VC-WIN64A` exit 2 |
| include 顺序调整 | 仍要求 MSYS 的 `openssl/crypto.h`，两套头无法同时满足 |

结论与缓解已写入 `docs/BENCHMARK.md` P2-1：CI（ubuntu）可直接编译；当前泄露面仅剩行数与表结构。

### 打包

| 产物 | 结果 |
|---|---|
| `digilegacy.exe`（release） | 18 MB，`cargo build --release` 成功 |
| **NSIS 安装包** | `DigiLegacy_0.2.2_x64-setup.exe` **4.68 MiB**（NSIS 3.13 手动缓存到 `%LOCALAPPDATA%\tauri\NSIS` + nsis_tauri_utils.dll 插件后成功） |
| MSI | WiX 3.14 下载超时（GitHub 网络阻断），CI 的 release-build job 可产出 |

### E2E（本机首次实跑）

| 命令 | 结果 |
|---|---|
| `PW_CHANNEL=msedge npx playwright test` | **80 passed**（系统 Edge 通道；playwright 官方 CDN 下载在本机网络反复被 dirlock 卡死，已把通道开关写进 playwright.config.ts） |

### 本轮真实执行（全部门禁）

| 命令 | 结果 |
|---|---|
| `cargo test --locked` | **220 passed / 1 ignored** |
| `cargo fmt --check` / `clippy --all-targets -D warnings` | PASS / 0 warnings |
| `npm run test:coverage` | 432 passed / 32 files，83.79/78.72/76.19/85.23（阈值 78/78/71/79） |
| `npx vue-tsc --noEmit` | PASS |
| `npm run build` / `build:demo` | PASS，无 `mock-*` 分块 |
| `check-doc-facts` | exit 0（Rust 222 / 前端 432 / E2E 80 / IPC 78 / 表 16 / 平台 152） |
| `check-policy-provenance` | exit 0（棘轮未变差） |
| `heartbeat-server.test.mjs` | all assertions passed |

### 明确未做

- MSI 安装包（WiX 下载被网络阻断；CI 可产出）。
- IMAP 真实服务器联调（模块测试覆盖解析/白名单/去重/配置往返；真实 Gmail/QQ 需账号，属用户侧验证）。
- 多设备 Shamir 恢复由集成测试覆盖，未做第二台物理机人工验证。

## 2026-10-29 能力补齐轮（v0.2.2 之后）

本轮从「单机无法让家人解密」这一结构性缺口出发，补齐了恢复路径与传输层，并对性能/模块化做了实测取舍。

### 变更摘要

| 变更 | 类型 | 依据 |
|---|---|---|
| KDF：PBKDF2-600k → **Argon2id**（m=19MiB, t=2, p=1），盐串 `v3.<m>.<t>.<p>.` 自描述；v1/v2 读取路径保留 | 安全 | OWASP 2024 建议；解锁耗时从 300–600ms 降到 20–60ms |
| 恢复封印 `RS2.` → `RS3.`（Argon2id + 自描述盐串），旧 RS2 / 无前缀 v1 封印继续可解 | 安全 | 换机恢复链路不能因升级断掉 |
| **Shamir 继承人恢复包**（`blahaj` crate，3-of-5，Base32 份额 + SHA-256 校验和 + 批次指纹） | 能力 | 单机应用唯一的「家人可解密」路径 |
| **恢复演练**（随机密钥自检 + 份额只读校验） | 能力 | 生成过 ≠ 可用 |
| SMTP：curl 子进程 → `lettre`（rustls） | 依赖 | 不依赖系统 curl 是否存在；TLS 策略由代码控制 |
| 心跳 / AI 助手的 HTTP：curl → `ureq`（rustls） | 依赖 | 同上 |
| 单实例锁（`tauri-plugin-single-instance`） | 正确性 | 双开会重复投递 DMS 邮件并争 SQLite 锁 |
| SQLite：`synchronous=NORMAL`、`cache_size=64MB`、`temp_store=MEMORY`、`mmap_size=256MB`、启动 `quick_check` | 性能/可靠性 | 写入吞吐提升；损坏库启动即暴露 |
| 索引：`idx_assets_created` / `idx_assets_identifier` / `idx_checkin_checked_at` / `idx_heirs_order` | 性能 | 资产列表默认按 created_at 排序，原先全表扫描 + 临时排序 |
| `db.rs` → 拆出 `db_assets.rs`（167 行）、`db_profile.rs`（196 行） | 可维护性 | 主文件从 2898 行降到 2606 行 |
| 移除 `naive-ui`（只用了两个 wrapper，却引入整个组件库） | 体积 | 入口 chunk 201KB → 98KB（-51.5%），总 gzip -6.8% |
| 仪表板统计保持 12 条查询（**实测否决合并**） | 性能 | 见下方基准，个人数据量级下合并更慢 |

### 仪表板查询基准（Python sqlite3, WAL + synchronous=NORMAL, 300 次均值）

| 行数 | 12 条独立查询 | 1 条聚合查询 | 结论 |
|---|---|---|---|
| 20 | 0.077 ms | 0.030 ms | 合并更快，绝对值可忽略 |
| 500 | 0.378 ms | 0.403 ms | 合并已无优势 |
| 2,000 | 1.365 ms | 1.663 ms | 合并慢 22% |
| 20,000 | 28.6 ms | 18.5 ms | 只有万级才反超 |

结论：个人数字资产量级（几十到几百条）下，覆盖索引让 12 条 COUNT 只读索引；单条 SUM(CASE…) 必须整表扫描。**保留原实现并在代码注释里写明否决理由与数据**，避免后人重复「优化」。

### 本轮真实执行

| 命令 | 结果 |
|---|---|
| `cargo test --locked` | **209 passed / 1 ignored**（源码声明 211，含 2 个 Unix-only 用例） |
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | 0 warnings |
| `npm run test:coverage` | **432 passed / 32 files**，statements 83.79 / branches 78.72 / functions 76.19 / lines 85.23（阈值 78/78/71/79 通过） |
| `npx vue-tsc --noEmit` | PASS |
| `npm run build` | PASS，`dist/assets` 无 `mock-*` 分块 |
| `npm run build:demo` | PASS |
| `node scripts/check-doc-facts.mjs` | exit 0（Rust 211 / 前端 432 / E2E 80 / IPC 74 / 表 14 / 平台 152） |
| `node scripts/check-policy-provenance.mjs` | exit 0（152 条 / 147 未核实显式标注 / 棘轮未变差） |
| `node server/heartbeat-server.test.mjs` | all assertions passed |

### 明确未做 / 未验证

- **E2E（Playwright）未在本机执行**：需先 `npx playwright install chromium`（本机未安装浏览器二进制）。E2E 用例数仍为 80（静态推导），未改动。
- **未执行 `tauri:build`**：本机无 WiX/NSIS 打包链，安装包构建由 CI 的 `release-build` job 覆盖。
- **未做真机多设备验证**：Shamir 份额跨设备恢复路径由单元测试覆盖（含混合批次拒绝），未在第二台机器上人工验证。
- **整库加密（SQLCipher）未实施**：字段级加密不保护表结构与元数据；这是已知缺口，列入下一轮（见 `docs/BENCHMARK.md` P2-1）。

## 2026-10-28 发版核对轮（v0.2.2）

用户要求严格核对五个远端的上传/CI/官网/Release/分支五项事实。核对结论与修复如下。

### 核对时发现的真实问题（均已修复）

| 问题 | 证据 | 修复 |
|---|---|---|
| Gitee 落后 51 个提交 | `git ls-remote` gitee main = `49a78a9`，本地 `331c650` | 推送至 `b0d2cfd` |
| 三个 GitHub 镜像落后 51 个提交 | API `repos/*/digilegacy` `pushed_at` 停在 2026-10-26 | 全部推送至 `b0d2cfd` |
| 官网线上内容是 2026-10-24 的旧版 | 线上 `docs/index.html` sha256 == `49a78a9:docs/index.html`，仍写「86 Rust / 265 frontend」「AI 接口已预留」 | 更新中英双站并随本次提交部署 |
| 在线演示缺审核页等 4 轮功能 | `docs/demo` 停在 `a27f70f`（10-26 19:32），`grep ReviewView` 零命中 | `npm run build:demo` 重建并覆盖 |
| x33834 与 Morningstar 的 Pages 部署失败 | x33834 `deploy` job 报 `in progress deployment` 冲突；Pages 状态曾为 `errored` | 本次推送触发重部署，两个站均已 `built` 并返回 200 |
| Gitee / GitCode 只有 v0.1.0 Release | API `releases` 长度 1 | 见「未能修复」 |
| README 中英测试数与覆盖率过期 | 写 118 passing / 85.4%，实测 202 passed / 85.23% | 改为 202 / 85.2% |
| README 写「Signed build pipeline」但无签名配置 | `grep signtool\|certificate` 全空 | 改为「发布 MSI+NSIS + SHA-256 校验，未做代码签名」 |
| 分支不只有 main | GitHub 镜像有 `backup-before-v0.2.0` + 6 个 dependabot 分支，Gitee 有 1 个 | 备份分支先打 `archive-backup-before-v0.2.0` tag 再删；dependabot 分支随 PR 关闭清理 |

### 本轮真实执行

| 命令 | 结果 |
|---|---|
| `bash scripts/verify.sh --with-e2e` | **ALL GREEN**（12 项 PASS） |
| `npm run test:coverage` | 424 passed / 32 files；statements 85.23 / branches 79.34 / functions 77.73 / lines 86.73 |
| `npx playwright test` | **80 passed** |
| `cargo test --locked` | **202 passed / 1 ignored**（源码声明 204） |
| `node scripts/check-doc-facts.mjs` | exit 0（Rust 204 / 前端 424 / E2E 80 / IPC 69 / 表 13 / 平台 152） |
| 站点内部链接 | 中英各 59 条，缺失 0 |

### 发布产物（v0.2.2，badhope/digilegacy）

| 检查 | 结果 |
|---|---|
| Release workflow | build + verify-release 两个 job 全 success |
| 资产 | `DigiLegacy_0.2.2_x64-setup.exe` (3,915,816 B)、`DigiLegacy_0.2.2_x64_zh-CN.msi` (5,525,504 B)、`digest-sha256.txt` |
| 下载 + 校验 | 三个文件全部 200 下载成功，`sha256sum -c` **两个 OK** |
| CI on `b0d2cfd` | 5 个 job 全 success |

### 明确未能修复 / 未验证

- **Gitee 与 GitCode 的 Release 仍只有 v0.1.0 及其两个 EXE**：`v0.2.0/0.2.1/0.2.2` 的构建产物由 GitHub Actions 产出，Gitee/GitCode 无等价 CI；把 5.5MB MSI 与 3.9MB EXE 上传到这两个平台需要手工建 Release 并传附件（接口已确认可用，但本轮以「官网下载指向 GitHub」为准）。官网所有下载按钮指向 `github.com/badhope/digilegacy/releases`，因此用户下载路径不受影响。
- **代码签名**：无证书，安装包未签名，Windows SmartScreen 会提示未知发布者。README 已改为如实表述。
- **GitHub HTTPS 主 IP 不可达**：`github.com:443`（20.205.243.166）超时，`api.github.com` 间歇超时；git 推送改用 `http.curloptResolve` 指向 `140.82.121.4` 或 SSH（`ssh.github.com:443`）。这是本机网络环境，不是仓库问题。

## 2026-10-26 扩展升级轮（历史快照 · 数字为当时状态，最终数字见第三轮门禁）

| 命令 | 结果 |
|---|---|
| `npx vue-tsc --noEmit` | exit 0 |
| `npm run test:coverage` | **349 tests / 31 files**，statements 85.40%（branches 78.74 / functions 76.88 / lines 86.88，阈值 78/78/71/79） |
| `npm run build` | PASS；`dist/assets` 无 `mock-*.js` 分块 |
| `npx playwright test` | **76 passed**（52.2s，含场景矩阵 39 个） |
| `cargo test --locked` | **103 passed, 0 failed, 1 ignored** |
| `cargo clippy --all-targets -- -D warnings` | 0 warnings |
| `cargo fmt --check` | PASS |
| `node server/heartbeat-server.test.mjs` | all assertions passed |

本轮修复与新增（每条都有回归测试）：

| 项目 | 级别 | 回归测试 |
|---|---|---|
| 打开仪表板即调用 `evaluate_dms`，会真的执行一轮 SMTP 投递（读页面产生副作用） | P0 | `DMSView.test.ts`「打开页面只读状态，绝不触发投递命令」、`dashboard.test.ts`「evaluateDms 走只读命令」 |
| 明文备份导入已加密库时绕过加密、直接写明文列 | P0 | `db.rs::plaintext_backup_never_lands_as_plaintext_in_encrypted_vault` |
| 备份不含恢复封印，换机恢复后恢复码失效 | P1 | `db.rs::import_backup_preserves_v3_seal_and_clears_legacy_stale_seal` |
| 剩余天数用 `num_days()` 截断，到期前不足一天被误判逾期 | P1 | `db.rs::days_until_due_rounds_up_instead_of_truncating` |
| 政策补丁导入后 `platformsExtended` 仍是旧缓存，继承人向导显示旧政策 | P1 | `platforms.test.ts::refreshPlatforms` |
| ZIP 工具包里的报告是页内片段，缺 DOCTYPE 与打印样式 | P1 | `ReportView.test.ts::ZIP 里的报告必须是独立 HTML` |
| `set_disposal_wish` 绕过全部输入校验（任意长度文本/非法枚举） | P2 | `commands.rs::disposal_wish_validates_enum_and_size` |
| `import_backup` / `export_report_to_file` 无体积上限 | P2 | `commands.rs::export_commands_reject_oversized_payloads` |
| 心跳地址经环境变量可绕过 http(s) 校验 | P2 | `heartbeat.rs::post_heartbeat_rejects_non_http_schemes` |
| 备份与 SMTP netrc 在 Unix 上按 0644 落盘 | P2 | `fsutil.rs::unix_file_is_owner_only` |
| 英文界面金额仍按「万」显示 | P2 | `format.test.ts::英文界面按 k/M/B 缩写` |
| 词典存在中英不对称、界面残留 emoji | P3 | `i18n/index.test.ts` 键集对齐 + emoji 门禁 |
| 自动备份是静默功能，界面不可见 | P3 | `ProfileView.test.ts::自动备份状态可见`、`backup.rs::latest_summary_reports_newest_file_and_size` |
| 通知历史只看本轮评估结果，刷新即消失 | P2 | `DMSView.test.ts::历史投递持久可见`、`notify.rs::outbox_history_reads_newest_first_and_skips_bad_lines` |
| 签到日志只写不读，用户无法自查上次确认 | P3 | `DMSView.test.ts::显示最近几次确认记录`、`db.rs::check_in_history_is_newest_first_and_capped` |
| `get_dms_status` 是界面唯一 DMS 数据源，若混入投递逻辑会重演 P0 | P1 | `commands.rs::get_dms_status_is_read_only_and_reports_stage`（含 dms_config 不被改动） |

## 2026-10-26 加固收尾（第三轮）

按上一轮评审的推荐逐项落地。

| 项 | 做法 | 验证 |
|---|---|---|
| CI 假绿灯 | `cargo-audit` 去 `|| true`、钉 0.22.1、打印 `--version` | `ci.yml:154-165`；本地 `cargo audit` 0 漏洞 |
| 文档数字漂移 | `doc-facts-lib.mjs` 单一推导 + `check-doc-facts.mjs` 门禁 + `sync-doc-facts.mjs` 修正 | `node scripts/check-doc-facts.mjs` exit 0（Rust 声明 120 / 前端 352 / E2E 76 / IPC 46） |
| 报告抓 DOM | 新增 `report.rs` 后端渲染，单文件与 ZIP 同源；含占位符拒绝导出 | `report.rs` 5 组测试 + `ReportView.test.ts` 2 例 + `commands.rs::render_report_response_refuses_placeholder_and_returns_full_document` |
| 「已解锁」不可信 | `security_status` 用验证器校验密钥；写路径同样拒绝错配密钥 | `commands.rs::security_status_is_false_when_key_does_not_match_verifier` |
| DMS 边界未披露 | 安心传承页新增边界区块 + 实际通道显示 | `DMSView.test.ts` 3 例 |
| `db.rs` 3.9k 行 | 拆 `db_platforms.rs` / `db_tests.rs`，生产代码 1882 行 | `cargo test --locked` 117 passed（纯移动；该数字为拆分当时快照，此后新增测试使其增长） |
| 导入层缺鲁棒性测试 | 513 个确定性畸形输入 | `scenarios.rs::backup_import_survives_500_malformed_inputs` |
| 死配置 | `stage3_wait_days` 补充说明 | `notify.rs` 注释 |
| 遗留截图 | 删除根目录 14 张（2.9MB） | `git ls-files screenshots` 为空 |

### 本轮新发现并修复

| 缺陷 | 级别 | 复现 | 回归 |
|---|---|---|---|
| `security_status` 只看「内存有 key」，恢复中途退出后密钥错配仍宣称已解锁 → 界面展示占位符并允许写回 | P0 入口 | 设正确密钥→换错密钥→`security_status().unlocked == true` | `security_status_is_false_when_key_does_not_match_verifier` |
| 备份 `user_profile` / `check_in_log` 非数组时被静默当空数组 → 导入"成功"却清空档案 | P1 | `{"user_profile":"x"}` 返回 Ok | `backup_import_survives_500_malformed_inputs` |

### 本轮验证

| 命令 | 结果 |
|---|---|
| `cargo test --locked` | **118 passed, 0 failed, 1 ignored**（源码声明 120，含 1 条 `#[cfg(unix)]` 在 Windows 不编译） |
| `cargo clippy --all-targets -- -D warnings` | 0 warning |
| `cargo fmt --check` | PASS |
| `npx vue-tsc --noEmit` | exit 0 |
| `npx vitest run` | **352 passed / 31 files** |
| `node scripts/check-doc-facts.mjs` | exit 0（文档与源码一致；反向验证：手改一个数字后 exit 1） |

## 2026-10-26 对抗式 debug（第二轮）

用「先复现、再修复、留回归」的方式逐条验证，不复述实现。

| 缺陷 | 级别 | 复现证据 | 回归测试 |
|---|---|---|---|
| 解密占位符被写回，真实密文被永久覆盖（资产详情 + 我的档案） | **P0** | 用错钥匙读出 `【加密数据 · 需解锁】` 后保存 → 用正确钥匙再读仍是占位符 | `db.rs::asset_placeholder_cannot_be_written_back`、`db.rs::profile_placeholder_cannot_be_written_back`、`AssetDetailView.test.ts`、`ProfileView.test.ts` |
| 锁定态可删除资产（锁定只防读不防破坏） | **P0** | `delete_asset_db` 在 `vault::clear_key()` 后仍返回 `Ok(())` | `db.rs::delete_asset_rejected_while_locked` |
| 政策补丁新增平台无法登记资产 | **P1** | `add_asset(platform_id=9001)` → `FOREIGN KEY constraint failed` | `db.rs::patch_only_platform_can_hold_assets` |
| 补丁平台上有资产时清除补丁直接失败 | **P1** | `clear_platform_overrides_db()` → `FOREIGN KEY constraint failed` | `scenarios.rs::clearing_patch_keeps_assets_on_patch_only_platforms` |
| 启用 DMS 但从未签到 → 永不触发 | **P1** | `enabled=true, last_check_in=NULL` → `overdue=false, stage=0` 恒成立 | `notify.rs::enabled_dms_anchors_on_first_enable_instead_of_never_firing` |
| 外部链接 userinfo 欺骗 | **P2** | `https://trusted.example.com@evil.example.com/steal` 通过校验 | `vault.rs::url_validation_rejects_userinfo_and_control_chars` |
| 发件箱按默认权限落盘（含收件人联系方式） | **P2** | 用 `OpenOptions` 直接创建，无非 Unix 0600 | `fsutil.rs` + `notify.rs` 写入路径 |
| 仪表板/报告总价值未过滤负值与超大值 | **P2** | `SUM(estimated_value)` 直接累加 | `db.rs::totals_ignore_negative_and_absurd_values` |
| 继承人向导「可继承/难继承」硬编码中文 | **P3** | 英文界面仍显示中文 | i18n 键已存在，改为 `t()` |

本轮验证：`cargo test --locked` **110 passed / 1 ignored**；`cargo clippy --all-targets -- -D warnings` 0 warning；`cargo fmt --check` PASS；`npx vitest run` **347 passed / 31 files**；`npm run test:coverage` exit 0（statements 85.40 / branches 78.74 / functions 76.88 / lines 86.88）；`npx playwright test` **76 passed**。

## 2026-10-26 真实使用场景模拟（大规模）

除回归单测外，本轮新增两套「按真实使用路径做矩阵遍历」的场景模拟，逐检查点断言不变量。

### 后端（真实 SQLite + 真实 AES，`src-tauri/src/scenarios.rs`）

| 场景族 | 规模 | 检查点 | 覆盖内容 |
|---|---|---|---|
| 资产生命周期矩阵 | 3 类别 × 7 处置 × 3 金额形态 × 加密/明文 = 126 | **1386** | 新增→读回→更新→分类查询→统计→删除，每步校验字段未被改写、统计自洽 |
| 备份往返矩阵 | 30 轮 × 5 资产组合 | **692** | v3 导出→清空→导入逐字段比对；v1 旧格式兼容；4 类损坏输入 fail-closed 且数据不变 |
| 凭据保险库矩阵 | 20 资产 × 加密/明文 | **180** | 锁定态拒绝、密文以 `enc:v1:` 落库、`has_credential` 同步、清除后不可读 |
| DMS 时间线矩阵 | 7 间隔 × 10 偏移 = 70 | **350** | 逾期判定、三级分级、签到后归零，与界面判定口径一致 |
| 政策补丁矩阵 | 30 轮 | **216** | 新增/覆盖判定、导出、清除回落到内置 152 条 |
| **合计** | **五个矩阵族** | **2824** | |

> 口径说明：组合数按「轮次 × 资产数」等不同定义，各矩阵族的「组合」不可加总，因此合计只报可执行的检查点数。

### 前端（真实 Pinia store + 真实 mock 后端，`src/scenarios.test.ts`）

| 场景族 | 规模 | 检查点 |
|---|---|---|
| 资产 store × 类别 × 处置 × 金额 | 3×7×8 = 168 | **1008** |
| 筛选/排序/搜索 | 4 类别 × 5 关键词 × 4 排序 = 80 | **324** |
| 格式化 × 语言 × 金额 | 2×12 | **88** |
| 枚举文案 × 语言 | 2×(3+7+2) | **24** |
| 锁定态 × 写路径 | 5 条命令 | **12** |
| 锁定态批量读取 | 12 资产 | **36** |
| mock 资产生命周期 | 20 轮 | **160** |
| 占位符识别（资产 5 字段） | 5 字段 × 正常/占位 | **10** |
| 档案占位符（12 字段） | 12 字段 | **12** |
| 平台缓存失效 | 10 轮 × 3 态 | **40** |
| **合计** | **十个矩阵族** | **1714** |

### 浏览器（真实 Chromium，`e2e/scenarios.spec.ts`）

| 场景族 | 规模 | 断言要点 |
|---|---|---|
| 路由 × 语言 × 主题 | 8 × 2 × 2 = **32** | 标题随语言、`data-theme` 落地、`html[lang]` 同步、无控制台错误、无横向溢出、主体非空 |
| 平台搜索关键词 | **20** | 每个关键词结果都包含关键词，不出现不相关平台 |
| 资产筛选 × 排序 | 4 × 4 = **16** | 组合后行数合法、无重复行 |
| 法律模板分类 | **6** | 每类都有模板且预览可开可关 |
| 继承人向导 Tab × 难度 | **2×4 + 折叠** | Tab 选中态、难度筛选、展开/收起确实改变条目数 |
| 安心传承幂等 | **10** | 连续签到 5 次 + 保存设置 5 次无错误 |
| 继承报告 | **6** | 正文完整、导出/工具包按钮可用、打印真实触发 |
| 语言/主题往返 | 8 × 2 = **16** | 每页切语言标题更新、主题切换生效 |
| **合计** | **39 个 E2E 场景** | 全绿（48.9s） |

### 本轮场景模拟抓出的真实缺陷

| 缺陷 | 级别 | 修复 | 回归 |
|---|---|---|---|
| 备份导入完全不校验 `category` / `disposal_method` / `estimated_value`，手改 JSON 可把非法枚举与负金额写进统计与报告 | P1 | `db.rs` 导入循环加枚举与金额校验，事务内拒绝且不留半截数据 | `backup_import_rejects_invalid_asset_fields` |
| `assets.totalValue` 用 `|| 0` 累加，负金额会原样相加，总价值可显示为负数 | P2 | 只累加有限且非负的估值，与后端统计口径一致 | `scenarios.test.ts::资产 store 矩阵` |
| Web 演示 mock 的 `add_asset` 不校验类别/处置/金额，与桌面版行为不一致 | P2 | mock 补同等边界；合法值仍放行 | `mock.test.ts::非法类别/处置/负金额…被拒绝` |

### 明确未覆盖

- 真实 SMTP 投递（需外部邮件服务器）与真实 Tauri WebView 自动化（需桌面窗口）不在场景模拟内；DMS 的分级与发件箱落盘已由 Rust 场景与单测覆盖。
- 场景模拟使用确定性伪随机序列（可复现），不是全量状态空间穷举。

## 本地门禁（Windows，`bash scripts/verify.sh`；历史轮 · 2026-10-24 数字）

| 步骤 | 命令 | 结果 |
|---|---|---|
| 依赖就绪 | `npm install`（本地；CI 用 `npm ci`） | PASS |
| 类型检查 | `npx vue-tsc --noEmit` | PASS (exit 0) |
| 测试 + 覆盖率门禁 | `npm run test:coverage` | 265 tests / 26 files，statements 79.41%（branches 79.00 / functions 73.17 / lines 80.78，阈值 78/78/71/79） |
| 生产构建 | `npm run build` | PASS |
| E2E（Playwright） | `npx playwright test` | **37 passed**（7 视图 + 发现链路 + 全站 UI 健全性 + 深色模式 + 视觉验收） |
| 格式 | `cargo fmt --check` | PASS |
| Lint | `cargo clippy --all-targets -- -D warnings` | 0 warnings |
| Rust 测试 | `cargo test --locked` | 86 passed, 0 failed, 1 ignored |
| 心跳服务器行为测试 | `node server/heartbeat-server.test.mjs` | 9 assertions PASS |

## 跨平台验证（Ubuntu 24.04 容器，真实 Linux）

CI 的 Rust job 在 linux 上跑，本地是 Windows。为避免"本地绿、CI 红"，在
Ubuntu 24.04 容器内用 Rust 1.99.1 完整复跑：

| 步骤 | 结果 |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS，0 warnings |
| `cargo test --locked` | 59 passed, 0 failed, 1 ignored（68.04s）— **历史记录**，当时套件规模；此后用例增至 86，容器未重跑 |

## E2E 跨平台验证（Ubuntu 24.04 容器）

E2E 首次在 Linux 容器内运行时 18 个用例全失败，排查后确认**不是测试代码问题**：
容器内 Playwright 浏览器二进制版本（1243）与 lockfile 要求（1248）不匹配，
`npx playwright install` 因 191MB 包在容器网络下超时未完成。

改用容器内已存在的 chromium 显式指定 `executablePath` 后复跑：

| 结果 | 耗时 |
|---|---|
| **18 passed / 0 failed** | 27.5s |

结论：E2E 用例本身跨平台可跑通；CI 上浏览器由 `npx playwright install --with-deps chromium`
安装，job 已设 `timeout-minutes: 15` 避免大包下载被截断。该记录为当时 18 例规模，当前套件为 37 例。

## E2E 变异验证

E2E 的价值在于能抓住视图层变异（单测覆盖不到）：

| 变异 | 结果 |
|---|---|
| DashboardView 统计卡片 class 改名 | 失败（被抓住） |
| ReportView 报告正文容器改名 | 失败（被抓住） |
| LegalTemplates 预览按钮去掉 click 绑定 | 失败（被抓住） |

## 心跳服务器 job

与 CI 中完全相同的脚本序列本地执行：

| 断言 | 结果 |
|---|---|
| `node --check heartbeat-server.mjs` | PASS |
| `POST /heartbeat` 返回 `{"ok":true}` | PASS |
| 非法请求（空 body）返回 400 | PASS |
| `GET /check` 返回 alerts/checked_at | PASS |

## Node 版本

依赖链 engines 要求 `^22.22.2 || ^24.15.0 || >=26`（jsdom 及其传递依赖）。本机 Node 24.14.0
**不满足**该范围（`npm ci` 会打印 EBADENGINE，但可运行），CI 固定 `22.22.2`，并在
`digilegacy/package.json` 的 `engines` 字段与本文件同步声明，`.nvmrc` 固定版本。

## 未能在本机验证的部分

- **GitHub Actions 运行结果**：`api.github.com` 在本机 TLS 不可达（curl exit 35），
  代理 `ghfast.top` 仅支持 git 协议。首次成功运行已由独立评审通过 GitHub API
  核实：run `39558924027`（sha `3fc3b68`）**completed / success**，当时三个 job 全绿。
  CI 现为五个 job（前端 / E2E / Rust / 心跳服务器 / 发布构建），本次改动后的运行结果
  仍需在 Actions 页面确认：
  https://github.com/badhope/digilegacy/actions

## 视觉与品牌验证

| 项目 | 验证方式 | 结果 |
|---|---|---|
| 深色模式 | Playwright 设 `data-theme=dark` 后截图检查标题/背景/卡片层次 | 通过（3 页截图） |
| 主题切换 | 点按钮后断言 `data-theme` 由 light 变 dark | 通过 |
| Toast / 确认框 | 单元测试 12 例（自动消失、错误样式、Promise 语义、alertdialog） | 通过 |
| 空态插画 | 资产页空数据渲染 SVG | 通过 |
| 报告文书版式 | 页眉/编号/页脚/签名区渲染 | 通过 |

## 可靠性与供应链

| 项目 | 验证 | 结果 |
|---|---|---|
| 后台任务 panic 隔离 | 单轮 panic 不终止循环（2 例） | 通过 |
| 每日自动备份 | 当日幂等、保留 7 份、裁剪顺序、无临时残留（4 例） | 通过 |
| Rust 依赖扫描 | `cargo audit --deny vulnerabilities` | 0 漏洞（5 条 unmaintained/unsound 警告来自 Tauri 上游） |
| npm 依赖扫描 | `npm audit --audit-level=high` | 0 漏洞 |
| E2E 覆盖率 | `npm run test:e2e:coverage` | 语句 58.9% / 行 60.1%（26 文件，与单测口径互补） |

## 发布构建验证（Windows）

此前从未验证过安装包能否产出。实测发现并修复了一个发布阻塞缺陷：
WiX 默认 codepage 1252 无法编码中文产品名「归零」，MSI 打包直接失败
（代码全绿但用户装不上）。

修复：`tauri.conf.json` 指定 `bundle.windows.wix.language = ["zh-CN"]`
+ `nsis.languages = ["SimpChinese"]`。

| 产物 | 大小 | 校验 |
|---|---|---|
| `归零_0.1.0_x64_zh-CN.msi` | 4.79 MB | ProductName=归零, ProductVersion=0.1.0, ProductLanguage=2052 |
| `归零_0.1.0_x64-setup.exe` (NSIS) | 3.34 MB | 生成成功 |

CI 新增 `release-build` job（windows-latest）断言两个产物都存在并上传 artifact。

## 2026-10-23 审计修复验证（历史轮 · 数字为当时状态）

本轮审计修复的每一项都带真实命令与结果。凡是未重跑的历史结论，均标注为「历史」。

### 修复项与回归网

| 缺陷 | 级别 | 回归测试 | 结果 |
|---|---|---|---|
| `encrypt_existing_data_db` 档案分支索引越界 panic（先填档案再设主密码必崩） | P0 | `cargo test --lib encrypt_existing_data_covers_all_profile_fields` | 修复前 `index out of bounds: the len is 10 but the index is 10`；修复后 PASS |
| 恢复成功后新恢复密钥 6 秒自动消失（旧码已失效） | P0 | `npx vitest run src/App.test.ts -t "新恢复密钥在用户确认抄写前不得自动消失"` | 修复前 FAIL；修复后 20 例 PASS |
| 托盘「锁定」事件无监听，界面仍显示已解密数据 | P1 | `npx vitest run src/App.test.ts` | 20 例 PASS（`bindTrayEvents` 在非 Tauri 环境为 no-op） |
| 深色模式三处硬编码白底（`.category-card` / `.modal-content` / `.confirm-card`） | P1 | `npx playwright test`（theme.spec + visual.spec） | 37 passed |
| `get_all_platforms_extended` O(n²) 重建平台表 | P2 | `cargo test --locked` | 81 passed |
| 报告加载失败静默（只有 console.error） | P2 | `npx vitest run src/views/ReportView.test.ts` | 8 例 PASS |

### 全量命令

| 命令 | 结果 |
|---|---|
| `npx vue-tsc --noEmit` | exit 0 |
| `npm test` | 22 files / **241 passed** |
| `npm run test:coverage` | statements 79.43 / branches 78.75 / functions 73.64 / lines 80.63（阈值 78/78/71/79） |
| `npx playwright test` | **37 passed**（31.4s） |
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS，0 warning |
| `cargo test --locked` | **81 passed, 0 failed, 1 ignored**（92.93s） |

### 文档与治理补齐

新增 `CONTRIBUTING.md`、`SECURITY.md`、`CODE_OF_CONDUCT.md`、`CHANGELOG.md`、`docs/ARCHITECTURE.md`、
`.gitattributes`、`.editorconfig`、`.nvmrc`、`.github/dependabot.yml`、
`.github/ISSUE_TEMPLATE/{bug_report,policy_correction,config}.yml`、`.github/PULL_REQUEST_TEMPLATE.md`、
`.github/workflows/release.yml`。

数字核对（以代码为准，逐条 grep 验证）：
- Tauri 注册命令 **42**（`commands.rs` 41 个 `#[command]` + `notify::evaluate_dms`）
- SQLite 表 **8**（`grep -c "CREATE TABLE" src-tauri/src/db.rs`）
- 内置平台 **152** 条 / **16** 分类（`PlatformEntry::{new,with_source}` 计数与 id 去重）
- 前端 mock 与 Rust 平台库 id 全集一致（152 vs 152，diff 为空）

### 明确未验证

- GitHub Actions 本轮改动后的运行结果：本机 `api.github.com` TLS 不可达，需在 Actions 页面确认。
- Windows 安装包未在本轮重新构建（`cargo test` 与前端构建产物已验证，`tauri:build` 未重跑）。
- 深色模式三处修复的**像素级**观感：由 `theme.spec.ts` 截图产出，未逐张人工比对。

## 2026-10-24 交付轮（v0.2.0）

### 修复与新增能力

| 项目 | 级别 | 回归网 | 结果 |
|---|---|---|---|
| CI Rust job 连续两次失败：`cargo audit --deny vulnerabilities` 是 0.22 已移除的取值 | 阻断 | 本地 `cargo audit`（默认遇漏洞 exit 1） | 0 漏洞、5 条上游 unmaintained/unsound 警告 |
| 资产列表加载失败被渲染成「还没有登记任何数字资产」 | P1 | `AssetsView.test.ts`（mock reject） | 12 例 PASS |
| 资产详情直接改 store，取消不撤销、保存失败无提示 | P1 | `AssetDetailView.test.ts`（失败留在页面） | 16 例 PASS |
| 危险确认框默认焦点落在「删除」上 | P1 | `FeedbackLayer.test.ts` | PASS |
| 导入备份后 store 全是旧数据 | P1 | `ProfileView.test.ts`（断言重取资产/统计） | 19 例 PASS |
| DMS 配置双事实源（保存档案会覆盖 DMS 页的间隔） | P1 | `ProfileView.test.ts`（断言不调用 `set_dead_mans_switch`） | PASS |
| 暗色三处浮层 `--gray-0` 未定义、侧栏 1.72:1、实心按钮 2.26–3.49:1、继承人提示卡硬编码浅黄 | P1 | Playwright theme/visual spec + 手工对比度计算 | 5.88–9.22:1 |
| 报告页加载中为空白壳、导出按钮可点但静默 | P2 | `ReportView.test.ts` | 8 例 PASS |
| 备份 v1 缺 heartbeat_config / platform_overrides | P2 | `backup_v2_covers_heartbeat_and_platform_overrides`、`legacy_v1_backup_imports_without_wiping_v2_config` | PASS |
| 命令层无输入上限（8KB 文本 / 估值有限性 / 平台 id / DMS 1–365 天 / 档案 500 字符） | P2 | `asset_commands_reject_oversized_and_invalid_payloads`、`dms_interval_must_be_within_range` | PASS |
| 心跳服务器 `/check` 无鉴权、任意令牌可写状态、`_alerted` 只在内存 | P2 | `server/heartbeat-server.test.mjs` | 9 断言 PASS |

### 本轮全量命令

| 命令 | 结果 |
|---|---|
| `npx vue-tsc --noEmit` | exit 0 |
| `npm test` | 26 files / **265 passed**，stderr 无未预期错误 |
| `npm run test:coverage` | 79.41 / 79.00 / 73.17 / 80.78（阈值 78/78/71/79） |
| `npm run build` | PASS；**产物中已无 mock 分块**（`dist/assets` 不含 `mock-*.js`） |
| `npm run build:demo` | PASS；`docs/demo/` 0.91 MB，hash 路由，浏览器实测无控制台错误 |
| `npx playwright test` | **37 passed** |
| `node server/heartbeat-server.test.mjs` | 9 assertions PASS |
| `cargo test --locked` | **86 passed, 0 failed, 1 ignored** |
| `cargo audit` | 0 漏洞（5 条 unmaintained/unsound 警告，来自 Tauri 上游） |

### 明确未验证

- macOS / Linux 安装包构建：仅 Windows 目标在 CI 中产出。
- 演示视频为 32 秒 1200×800 mp4，未做多语言字幕。

CI 已确认：run `40072253561`（sha `9a55a80`）5 个 job 全绿；Release run `40075221850` 的
build 与 verify 两个 job 全绿，Release `v0.2.0` 的 MSI/NSIS 已独立复核 SHA-256 一致。

### 收尾轮补充（路由与 IPC 入口的回归网）

`src/router.ts` 与 `src/api/invoke.ts` 是导航与 IPC 的唯一入口，此前 router 为 0% 覆盖。
本轮补 `src/router.test.ts`（5 例：404 兜底、深层未知路径、`document.title` 同步、
根路径重定向、scrollBehavior 恢复）与 `invoke.test.ts` 扩展（+3 例：纯前端生产构建报错、
`IS_WEB_PREVIEW` 在 dev/Tauri 两种环境下的取值）。

| 命令 | 结果 |
|---|---|
| `npx vitest run src/router.test.ts` | 5 passed |
| `npx vitest run src/api/invoke.test.ts` | 8 passed，invoke.ts 覆盖率 100% |
| `npm test` | 26 files / **265 passed** |
| `npm run test:coverage` | 79.41 / 79.00 / 73.17 / 80.78（阈值 78/78/71/79） |
| `node scripts/demo-media.mjs`（SKIP_VIDEO=1） | 10 张截图重出，标题断言全部匹配 |

`scripts/demo-media.mjs` 的 `title` 字段此前未被断言（只检查标题非空），现在会在标题与
路由预期不符时直接失败，避免交付错图。

### 双语官网与截图交付（2026-10-24 追加）

| 检查项 | 命令 / 方法 | 结果 |
|---|---|---|
| 内部链接完整性 | 解析 `docs/index.html` 与 `docs/zh/index.html` 的全部 href/src/poster，逐个 stat | 两页各 62 条，缺失 0 |
| HTML 结构 | 自写 HTMLParser 栈式校验 | 两页均无未闭合/错配标签 |
| 语言切换 | Playwright 点击 `.lang a`，双向验证 | `zh/#screenshots → en/#screenshots`、`en/#security → zh/#security` |
| 首屏截图 | `img.naturalWidth` 断言 | 2400（真实加载，非占位） |
| 截图区 | 逐段滚动触发 lazy load 后统计 | 10/10 加载成功 |
| 演示视频 | `video.readyState` + 元数据 | 32.28s / 1200×800，可播放 |
| 双语内容纯度 | 去标签后统计可见文本 | 英文页残留中文仅品牌名与「中文官网」入口；中文页仅代码块含英文 |
| README 锚点 | 标题 slug 与 `](#…)` 比对 | 两个 README 各 0 失效 |
| 线上部署 | `curl -o /dev/null -w %{http_code}` | `/`、`/zh/`、`/demo/` 均 200；三套镜像的 `/zh/` 均 200 |
| CI | GitHub Actions run | `2cf8aa4` 5 job 全绿 |

新增文件：`docs/site.css`（两站共用）、`docs/zh/index.html`（完整中文站）、`README.zh-CN.md`。

### 2026-10-31 追加：release 构建稳定性根因与修复

`tauri build` 首次失败于 `rustc` 自身崩溃，不是代码错误：

| 现象 | 证据 |
|---|---|
| release 链接 `digilegacy_lib` 时 rustc 退出 `0xc0000409` | `STATUS_STACK_BUFFER_OVERRUN` |
| 定位到 `windows 0.62.2` 编译阶段 | 加 `RUST_MIN_STACK=256MB` + `CARGO_BUILD_JOBS=1` 仍崩 |
| 降 `opt-level=3 → 1`、`codegen-units=16`、`debug=0` | **release 构建成功，16m38s** |

已固化到 `src-tauri/Cargo.toml` 的 `[profile.release]`，避免每次打包重复踩坑。这是本地工具链限制，CI（ubuntu / 官方 runner）不受影响。

另修：`tauri-plugin-notification` Rust 2.8.0 与 NPM 2.7.0 版本错配导致 bundler 直接拒绝打包，已对齐到 2.8.0。
