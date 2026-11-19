# 技术栈审计：轮子检查与选型依据

> 2026-11 全量审计结论。原则：**能用成熟生态就不自研；自研必须给出「为什么现成方案不满足」的理由。**
> 本文件随依赖变化维护，新增依赖时同步补充。

## 结论速览

| 层 | 选型 | 判定 | 说明 |
|---|---|---|---|
| 桌面框架 | Tauri 2 | ✅ 成熟生态 | 比 Electron 小一个数量级的安装包；Rust 侧复用系统 WebView |
| 前端框架 | Vue 3 + TypeScript + Vite | ✅ 成熟生态 | 组合式 API + `vue-tsc` 全量类型门禁 |
| 状态 | Pinia 4 | ✅ 生态标准 | Vue 官方推荐，取代手写全局 store |
| 图表 | ECharts 6 | ✅ 成熟生态 | 仪表板环形图/构成条 |
| 打包 | JSZip | ✅ 成熟生态 | 工具包 ZIP 导出 |
| 数据库 | SQLite（rusqlite 0.40） | ✅ 事实标准 | 单文件、可备份、可离线 |
| 密码哈希 | Argon2id（`argon2` 0.6） | ✅ OWASP 建议 | 显式上限防参数注入；旧 PBKDF2 库只读兼容 |
| 字段加密 | AES-256-GCM | ✅ 事实标准 | 随机数据密钥 + 口令包裹密钥，改密码不重加密全库 |
| 秘密分片 | **blahaj 0.6**（GF(256) Shamir） | ✅ 2026-11 迁移 | 原 `sharks 0.5.0` 有 RUSTSEC-2024-0398 且无上游修复；blahaj 是其官方认可的修复分叉，API 兼容 |
| 邮件 | `lettre` 0.11（rustls） | ✅ 成熟生态 | 取代早期 curl 子进程方案：不依赖系统 curl、TLS 策略可控 |
| HTTP | `ureq` 3（rustls） | ✅ 轻量成熟 | 仅心跳/更新检查；阻塞式足够（无高并发场景） |
| 邮件签到 | `imap` + `imap-proto` | ✅ 成熟生态 | 零内容签到：只看有没有新邮件，不读正文 |
| Base32/编码 | `data-encoding` | ✅ 成熟生态 | 恢复份额纸面编码；不自研 Base32 实现 |
| 敏感数据擦除 | `zeroize` | ✅ 行业标准 | 主密码用完即清内存 |
| Agent 协议 | OpenAI-compatible + **MCP stdio** | ✅ 双通道 | 应用内助手走 function-calling；`digilegacy-mcp` 二进制把同一套只读工具暴露给任意 MCP 客户端（2026-11 落地） |

## 曾经过自研、后来被替换的轮子

| 曾自研 | 替换为 | 原因 |
|---|---|---|
| curl 子进程发邮件 | lettre（2026-0x） | 依赖系统 curl 是否存在，打包后不可控 |
| 手写「工具定义 + 执行 match」双份维护 | `ToolSpec` 注册表（2026-11） | 两边人肉同步必然漂移；现在定义/执行/权限绑定在一条数据上 |
| 固有 `from_str` 解析（models.rs） | 标准 `std::str::FromStr` trait（2026-11） | clippy::should_implement_trait 指出的惯用性问题；调用方改用标准 `parse::<T>()` |

## 有意保留的自研（附理由）

| 模块 | 为什么不用现成库 |
|---|---|
| 政策库（152 平台，JSON 内嵌） | 没有现成的「中国平台数字遗产政策」数据集；这是本项目的核心内容资产，不是轮子 |
| 四维完成度模型 | 表达产品判断（继承必须有继承人），通用任务库给不了 |
| 家人执行清单/FAQ 渲染器 | 交付格式（自包含可打印 HTML）是产品差异化，不是通用 HTML 框架能替的 |
| MCP 服务器（~300 行手写 JSON-RPC） | 官方 Rust SDK（rmcp）尚在快速演进且会引入异步运行时依赖；本项目 stdio 只读场景极小，手写 + 9 个协议契约测试更可控。SDK 稳定后可评估迁移 |

## 已知技术债（如实列出）

1. `imap-proto 0.10` 有 future-incompat 警告（上游未发新版），等待上游升级。
2. **RustSec 7 条 unmaintained/unsound 告警全部来自 Tauri 的 Linux 传递依赖**
   （zbus_names/zvariant/zvariant_derive/glib/async-executor/lexical-core/sharks 时代的
   余量已清）。Windows 构建不打 Linux GTK/DBus 依赖树，这些告警对发布的安装包
   无影响；修复责任在 Tauri 上游切换 Linux 后端时，应用侧无法提前升级。
   CI 扫描（vulnerabilities → fail，unmaintained/unsound → warn）已把
   「真漏洞出现时必须红」的机制固化。2026-11-04 CI 实测：**0 vulnerabilities**。
3. ~~整库加密（SQLCipher）~~ **已实施（2026-11-04，v0.4.0）**：`rusqlite/bundled-sqlcipher-vendored-openssl`；
   库加密 key 随机 32 字节 + Windows DPAPI（CurrentUser + 应用熵）保护，独立于主密码体系
   （锁定/解锁/改密码语义零改动）；迁移走官方 `sqlcipher_export`，带快照与独立连接验证，
   失败自动回滚；明文模式 = 无 key pragma（SQLCipher 无 key 即普通 SQLite）。
   280+9 测试在 SQLCipher 构建下全过，含加密↔明文往返与错误 key fail-closed 两个规格测试。
2. ~~MCP 服务器读数据库走读写连接~~ **已解决（2026-11-04）**：`init_database_readonly_at` 以 `SQLITE_OPEN_READ_ONLY` 打开，不迁移不建表；WAL 残留时明确报错而不是降级读写。
3. ~~Agent 任务循环无持久化日志~~ **已解决（2026-11-04）**：`agent_run_log` 表（任务/结果摘要/轮数/工具名清单，90 天保留 + 手动清空，参数与结果全文不落库）。
