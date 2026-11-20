# Changelog

[English](../../CHANGELOG.md) | 简体中文

## [0.0.21] - 2026-11-21

### 修复

- 同一模型的并发启用由互斥锁串行化；两个并发请求都返回 200，指向同一 PID，只留下一个 sidecar。
- 本地规划器超时不再被悄悄压到 120 秒；本地模型默认 180 秒，可用 `DIGILEGACY_LLM_TIMEOUT_SECS` 调到 300 秒。
- 服务启动时若 `active.json` 的 PID 与端口都已失效，会删除这条过期记录，规划器不会再指向已停止的 sidecar；仍在监听的 sidecar 会保留。
- 英文设置页补齐“推荐”徽标、上下文标签、参数字符串和三款模型描述，不再在中英文之间混排。
- 平台 API 或本地模型路由不可达时，面板显示“暂不可用”的可读提示，不再直接暴露 `Unexpected end of JSON input`。

### 新增

- `DIGILEGACY_LOCAL_MODEL_CTX` 可配置 sidecar 上下文，范围 2,048–131,072，默认 8,192。9,028-token prompt 在 8K 下失败，调到 16K 后成功。

### 测试

- Rust workspace：**187 passed / 0 failed**；Web：**79 passed / 0 failed**；`./tools/verify.sh` OK。
- 已实测并发启用、损坏 `active.json`、幽灵 PID 清理、端口耗尽、只读模型目录、文件写配额、路径穿越和完整中英文界面，记录见 `docs/operations/measurements/local-models-functional-2026-11-20.md`。

## [0.0.20] - 2026-11-20

### 新增

- “设置 → 连接”加入本地模型目录。默认 **MiniCPM5-1B**：1.08B 参数、131,072 上下文、Apache-2.0、标准 Llama 架构，Q4_K_M 文件 688 MB。
- 每个模型都有真实下载按钮、进度轮询、`.part` 临时文件、流式 SHA256 校验和校验标记。服务端用 `curl` 从 ModelScope 下载，因为部分 TLS 指纹会被 CDN 拒绝。
- “启用”会启动 `tools/local_model_server.py` 这个最小 OpenAI 兼容 sidecar，并写入 `data/models/active.json`。未设置 LLM 环境变量时，规划器读取该文件，因此 Web、CLI、HTTP 都会自动使用已启用模型，不需要 Ollama，也不需要手工改环境变量。
- MiniCPM5-2B 作为质量档列出，并明确标注“实际 >2B”。Spark-X2.5-1.7B 作为严格 <2B 的长上下文实验项列出；本机 `llama-cpp-python 0.3.37` 已能加载并返回合法计划，但长上下文未压测，仍保留“实验运行时”标注。

### 修复

- 本地模型规划请求加入 JSON 语法约束，避免 1B 模型少写计划末尾的右括号。
- 服务重启后能通过 `active.json` 识别仍在运行的 sidecar；重复启用同一模型是幂等的，切换到另一个模型会按 PID 终止旧 sidecar。
- 启用失败不再一律返回 502：未知模型返回 `NOT_FOUND`，未完成文件返回 `CONFLICT`，SHA256 不匹配返回 `CORRUPT_DATA`。
- 校验增加文件大小检查，并在加载前重新计算 SHA256；伪造标记不能再让损坏文件显示为可用。
- sidecar 启动即退出时在下一次轮询就报错，不再空等 45 秒。
- Web 面板在下载结束后清除“下载中”提示；保存写令牌后按钮即时解锁。

### 测试

- 功能矩阵共 33 项，覆盖目录、鉴权、下载成功/重复/失败、SHA256 损坏、启用、切换、停止、重启恢复、PID 安全与规划器接入，全部通过；详见 `docs/operations/measurements/local-models-functional-2026-11-20.md`。
- 规划实测：MiniCPM5-1B 55 秒，MiniCPM5-2B 116 秒，Spark-X2.5-1.7B 83 秒；三者均返回合法 `assets.list` 计划。
- Rust workspace：**185 passed / 0 failed**；Web：**79 passed / 0 failed**。
- 真实 Chromium live smoke：14/14 路由通过，axe 0，页面异常 0，包含新的本地模型设置区。

## [0.0.19] - 2026-11-20

### 新增

- 恢复码解锁接入四条链路：`security.recover` 契约、`POST /api/v1/security/recover`、CLI `security recover` 与 Web 锁屏“使用恢复码”。恢复码从初始化输出中离线保存，解锁成功后只装载当前进程会话，不落盘。
- Web 锁屏保留主密码表单，增加恢复码模式切换；错误恢复码、空输入和限速分别显示本地化提示，成功后仍把焦点移到主区域。

### 安全

- `security.unlock` 与 `security.recover` 共享进程内失败限速：60 秒窗口内累计 5 次失败后锁定 30 秒，返回 `429 RATE_LIMITED`；锁定期间即使输入正确主密码也不放行。成功解锁会清零计数。
- 全局锁门禁的放行名单加入 `security.recover`，锁定时恢复码入口不会被 `VAULT_LOCKED` 误拦；其余命令仍返回 `423`。
- CLI 的 `security recover` 从标准输入读取恢复码，和 `security unlock` 一样只在当前进程生效；脚本仍可用 `DIGILEGACY_VAULT_PASSWORD` 解锁普通命令。

### 测试

- `tools/verify.sh` exit 0：**184 Rust + 79 Web**；契约 **44 命令 / 23 工具 / 60 schema / 21 错误码**；schema v16 / 31 表。入口 80.75/82 KB，JS gzip 104.95/105.2 KB，CSS gzip 20.22/21.1 KB，总 gzip 125.17/125.5 KB。
- 隔离保险库真实 API：锁定概览 `423` → 错误口令 `400` → 正确解锁 `200` → 概览 `200` → 再锁定后恢复码 `200` → 资产 `200`（1 条）→ 再锁定；连续 5 次错误口令为 `400/400/400/400/429`，限速期间正确口令仍为 `429`。证据 `security-recovery-0.0.19-2026-11-20.json`。
- CLI：锁定状态可读；无口令 `assets list` 返回 `VAULT_LOCKED`；带 `DIGILEGACY_VAULT_PASSWORD` 读到 1 条；`security recover` 成功，但进程退出后下一条命令仍被锁。真实 Chromium 覆盖密码、恢复码、再次锁定与 390px，axe 0、页面异常 0；14/14 真实路由和 9/9 视觉基线通过。

## [0.0.18] - 2026-11-20

### 新增

- 资产生命周期显式化：资产支持 `active` / `archived`，详情页可原地归档与恢复，CLI 增加 `assets update --status`。归档只从默认列表隐藏，不删除数据。
- 规划页可编辑：`plan.profile.set` 更新持有人姓名，`plan.heirs.set` 整表替换继承人，支持新增、编辑、排序、指定执行人与删除；两者返回最新的 `plan.get` 快照，并同时暴露到 CLI、HTTP 与 Web。
- 空态补齐下一步：资产真空态直接打开编辑器，筛选空结果清除筛选，自动化无运行记录时新建运行。
- 审批拒绝在 Web 表单与领域命令两侧都强制填写理由；控制台补上保存成功提示。

### 修复
- 离线 mock 模式下资产创建/更新会回显请求的名称与状态，无需真实 API 也能看到归档/恢复结果。
- 写令牌缺失提示统一指向 **设置 → 安全与数据**，与实际填写 `DIGILEGACY_API_TOKEN` 的位置一致。
- 服务端补挂契约声明的资产写路由：`POST /api/v1/assets` 与 `PATCH /api/v1/assets/{id}`。

### 安全

- 控制台锁定改为真实保险库会话，不再是本地布尔遮罩。`security.init` 由本地 CLI 引导 `security_config`，`security.unlock` 校验主密码并装载进程 DEK，`security.lock` 清除它；Web 锁屏只有在本地 API 接受口令后才放行。
- 已配置保险库且锁定时，应用层除 `security.status`、`security.unlock`、`security.lock` 外拒绝所有命令，读取返回 `423 VAULT_LOCKED`；Web 主壳不挂载，锁屏背后不会残留旧数据。
- 引导时把已有连接密钥与政策候选密文从旧文件/配置密钥迁移到保险库派生的域密钥。`security_config` 存在后，字段加密只从已解锁会话 DEK 派生，不再使用数据库密钥文件。
- Web 启动读取 `security.status`，锁定态隐藏控制台；解锁后把焦点移到主区域且不滚过顶栏；未聚焦的跳转链接完全隐藏。

### 测试

- `tools/verify.sh` exit 0：**182 Rust + 77 Web**；契约 **43 命令 / 23 工具 / 59 schema / 21 错误码**；schema v16 / 31 表。入口 79.72/82 KB，JS gzip 104.46/104.6 KB，CSS gzip 20.15/21.1 KB，总 gzip 124.61/124.8 KB。
- 隔离保险库库真实 API 演练：锁定状态、`423 VAULT_LOCKED`、错误口令 `400 INVALID_ARGS`、正确解锁 `200`、概览 `200`、再次锁定后资产 `423`。真实 Chromium 覆盖错误口令、成功解锁、再次锁定、390px 布局，axe 0 违规且无页面异常；14/14 真实路由与 9/9 视觉基线通过。

## [0.0.17] - 2026-11-20

### Security

- 可选 `DIGILEGACY_OUTBOUND_ALLOWLIST` 现在在连接 URL 校验时即生效，不再只等到探测时拒绝；不在表内的主机在 DNS 解析前就被拒绝。
- 出站 HTTP(S)、政策 feed 与 SMTP/IMAP 探针改为“解析一次、校验全部返回地址、连接钉住已验地址”。主机名不能第一次解析为公网通过检查、第二次解析为私网再连接。
- 即使打开 `DIGILEGACY_ALLOW_PRIVATE_NETWORK`，云元数据地址仍拒绝；运营商级 NAT、基准测试、保留与文档网段也一并拒绝。
- 新增可选 `DIGILEGACY_OUTBOUND_ALLOWLIST`：逗号分隔的主机 allowlist（`example.com`、`*.example.com`）；设置后不在表内的主机在 DNS 解析前即拒绝。
- 新增带版本号的 Linux 发布归档与 `install.sh`、`start.sh`、`stop.sh`、`uninstall.sh`。安装脚本升级时保留环境文件、不打印写令牌，保留数据时一并保留数据库密钥；干净机演练覆盖安装、诊断导出、检查点验证、卸载与重装读库。
- 出站客户端绕过环境 HTTP 代理。代理会绕开地址钉住、重新解析目标，因此这些连接探针明确不走代理。

### Operations

- 发布包新增同源 nginx server 块 `share/nginx/digilegacy.conf.example`：SPA 回退、`/api` 代理、哈希资源 immutable 缓存、`index.html` no-store 与安全头；已在 nginx 1.18.0 实测。
- 安装脚本在 `umask 077` 下显式把静态 Web、文档、nginx 样例与运维工具设为可读；否则非 root nginx worker 读不到 SPA。干净机演练新增精确权限断言。
- 新增政策来源预审 `tools/policies/source_audit.py` 与 152 行人工清单：报告 URL 是否存在、HTTPS、首页/深链、重复组与可达性，不改政策数据或 provenance。2026-11-20 首轮结果：150 条首页、2 条深链、17 组重复，152/152 仍需人工复核。

### Testing

- 新增真实浏览器路由冒烟：真实 API + Vue dev server + Chromium 146 覆盖 15 个路由/主题/视口组合，并在真实浏览器运行 axe WCAG A/AA/2.2 AA（含对比度），无页面异常、API 5xx 或横向溢出。首轮抓到并修复顶栏 2 个 `label-content-name-mismatch`，最终 axe 违规为 0；9 张视觉基线也复跑通过。

## [0.0.16] - 2026-11-20

### Added

- **设置 → 连接**新增可交接的账号接入引导：SMTP、Webhook、飞书、企业微信按钮会预填已有的加密连接表单，另有“复制填写说明”可直接发给账号本人。
- 新增 `docs/operations/CONNECTION-SETUP.md` 及中文镜像：列出各服务商需要的字段，明确平台写令牌与账号凭据分开交接，并给出测试后交回的检查清单。

### Accessibility

- 连接设置路由纳入全路由 axe 门禁，路由覆盖从 15 条增加到 16 条。

### Changed

- 账号引导使体积预算调整为：入口 82 KB、JS 总 gzip 103.1 KB、CSS gzip 21.1 KB、总 gzip 124.2 KB；单路由 24 KB 与 vendor 130 KB 不变，设置页路由 20.79 KB。

## [0.0.15] - 2026-11-20

### Added

- 新增只读 `GET /api/v1/metrics` JSON 快照：请求总数、状态码分类、并发中请求、延迟 p50/p95/p99/max 与延迟桶。不记录路径、姓名、标签或凭据；未知路由也计入。
- `digilegacy-cli diagnostics export --allow-write --json` 现在输出脱敏结构快照，包含 `audit.valid` 与各表行数。该契约命令此前只在清单里，没有接上实现。
- 新增可重复的 SLO 参考数据集（`seed_reference`）与严格容量脚本（`tools/bench/capacity.py`），在临时库上测真实 HTTP 读写。
- `docs/operations/measurements/` 记录容量与恢复演练证据：读 p95 9.54 ms / p99 11.09 ms，写 p95 3.70 ms，恢复到健康 158 ms，10,120 条审计有效。

### Changed

- SLO 与运行手册改用 `/api/v1/metrics`、`diagnostics export` 与 `security status` 测量，不再引用不存在的 metrics 端点。

## [0.0.14] - 2026-11-20

### Added

- 本地 CLI 新增 `security checkpoint`、`security checkpoints`、`security verify`：先做 WAL checkpoint，生成认证清单并写入 `backup.create` 审计；`security status` 现在能报告真实最近备份时间。
- 新增运维文档：暂定 SLO、月度可用性预算，以及启动、日常检查、备份、离线恢复、升级与常见故障的运行手册。
- 运行手册明确不提供在线 `security restore` 命令：覆盖在线数据库必须人工、离线执行。

## [0.0.13] - 2026-11-20

### Added

- 新增 5 个确定性模糊测试目标，随 `cargo test` 每次执行：外部 URL 校验、恢复码归一与封印解析、纸质份额解码、KDF 参数解析、HTML 转义。
- 固定种子共 45,000 组输入，失败可复现；安全 crate 原本已依赖 `rand`，render crate 仅新增 dev-dependency，生产依赖图不增加。

## [0.0.12] - 2026-11-20

### Added

- 新增轻量视觉回归基线：9 张固定视口 WebP，覆盖桌面亮色、桌面暗色与 390px 移动端概览；整套约 **292 KB**，不进入 `tools/verify.sh`，作为发布前人工门禁。
- `tools/visual/regress.py` 在加载前冻结时钟与语言，失败时只写单张 diff；允许 0.4% 像素差异，避免有损 WebP 重编码产生噪声。

### Fixed

- 窄屏页面标题不再被挤成每行两三个字：700px 以下页头改为纵向堆叠，操作按钮落到文案下方，展示标题降到 `clamp(28px, 8vw, 36px)`。

- 修移动端页头后预算小幅上升：入口 79.24 / 80 KB、vendor 99.50 / 130 KB、JS 总 gzip 101.13 / 101.15 KB、CSS 20.92 / 21 KB、总 gzip 122.04 / 122.1 KB；JS 总与总 gzip 各放宽 0.05 / 0.1 KB，入口、vendor、CSS 未放宽。

## [0.0.11] - 2026-11-20

### Accessibility

- 新增全路由 `axe-core` 门禁：15 条 Web 路由在数据落定后按 WCAG 2.0/2.1/2.2 A/AA 标签扫描，覆盖概览、资产、政策、发现、资产详情、规划、交付、自动化、新建运行、运行详情、工具、审批、两个设置分区和 404。
- 工具目录的权限筛选下拉框与搜索框补齐可访问名称；扫描只发现这一处 `select-name` 违规，修复后 15/15 通过。
- Web 测试增至 **66**；契约不变 **39 命令 / 23 工具 / 56 schema / 21 错误码**。
- 预算：入口 79.24 / 80 KB、vendor 99.50 / 130 KB、JS 总 gzip 101.09 / 101.1 KB、CSS 20.85 / 21 KB、总 gzip 121.94 / 122 KB。
- 边界：jsdom 中的 axe 不计算真实颜色对比度，对比度仍由令牌测试兜底；真实浏览器亮暗检查、读屏人工验收和视觉回归基线仍未完成。

## [0.0.10] - 2026-11-20

### Changed

- 版本号统一为 `0.0.x` 三段式；此前 `0.9.0-m5x` 标签保留为历史记录，不再用于新构建。

### Accessibility

- 壳层新增本地化“跳到主要内容”链接和“主导航”标签；移动导航改为模态抽屉：打开时焦点进入关闭按钮，背景 `inert`，`Escape` 关闭，关闭后焦点回到菜单按钮；关闭状态下用 `visibility: hidden` 移出 Tab 顺序。
- 亮色主题的 `muted`、`clay`、`amber` 与暗色主题的 `clay` 调整到 WCAG AA 对比度；新增设计令牌测试，亮暗各 15 组前景/背景组合均要求 **≥ 4.5:1**。
- 新增 `axe-core` 开发期门禁：壳层、Agent 抽屉、命令面板、锁定层按 WCAG 2.0/2.1/2.2 A/AA 标签检查；移动导航另有键盘回归测试。
- Web 测试增至 **51**；契约不变 **39 命令 / 23 工具 / 56 schema / 21 错误码**。
- 预算：入口 79.16 / 80 KB、vendor 99.50 / 130 KB、JS 总 gzip 101.07 / 101.1 KB、CSS 20.85 / 21 KB、总 gzip 121.93 / 122 KB；JS 总预算从 101 调到 101.1 KB，其他未放宽。
- 边界：axe 只覆盖壳层和三个弹层，未覆盖全部业务路由；完整全路由 axe、视觉回归基线和读屏人工验收仍在 M6 待办。

## [0.0.9] - 2026-11-20

### Accessibility

- 新增共享 `useDialogFocus`：Agent 抽屉、命令面板和锁定层打开时把焦点移入弹层，`Tab` / `Shift+Tab` 在弹层内循环，关闭后焦点回到触发按钮。
- 锁定层初始焦点落在解锁按钮；命令面板初始焦点落在搜索框；Agent 抽屉初始焦点落在面板本身。

## [0.9.0-m5f] - 2026-11-20

### Fixed

- 新增本地 rustls SMTP 服务器联调：STARTTLS 与隐式 TLS 均完成真实握手、证书校验、TLS 后 `EHLO`、`AUTH PLAIN`、`MAIL/RCPT/DATA` 和中文正文投递。
- SMTP 行读取从逐字节 Latin-1 字符拼接改为按原始字节解码 UTF-8，避免中文正文在测试服务器与错误响应中被拆坏。

## [0.9.0-m5e] - 2026-11-20

### Security

- 出站连接与政策端点增加统一地址校验：拒绝云元数据、链路本地和私网字面地址；回环地址保留给本地开发，局域网部署需显式设置 `DIGILEGACY_ALLOW_PRIVATE_NETWORK=1`。
- `policies.sync` 在拉取远端 feed 前复用同一校验，避免候选审核链路被用来访问内网服务。
- 签名政策 feed 增加真实本地 HTTP 测试：HMAC 校验、篡改拒绝、`If-None-Match`/304 链路均有覆盖。

## [0.9.0-m5d] - 2026-11-20

### Added

- `policies.sync`：HTTP `POST /api/v1/policies/sync`、CLI `policies sync`；write + approval required + audit，MCP/Agent 不暴露。远端 feed 优先取已启用 saved `policy` 连接，回退 `DIGILEGACY_POLICY_URL/TOKEN`；支持 cursor、`If-None-Match/304`、Bearer；拉取只写候选，不自动修改政策库。
- `policies.candidates.list`：HTTP `GET /api/v1/policies/candidates`、CLI `policies candidates`；只读分页，返回待审/冲突候选、字段级 diff、内容哈希、最近同步时间/状态/错误；新增 MCP 只读工具。
- `policies.candidate.decide`：HTTP `POST /api/v1/policies/candidates/{id}/decide`、CLI `policies candidates decide`；write + approval required + audit。批准才解密 payload 落库，保留 `created_at`、更新 `updated_at`，provenance 强制回 `unverified` 并写入“同步候选已批准；内容已更新，来源需重新核对”；拒绝只改候选状态。
- schema v16 / 31 表：新增 `policy_sync_state` 持久化 source/cursor/etag/lastSyncAt/lastStatus/lastError；`policy_candidates(status, fetched_at)` 加索引。领域层新增 19 字段级 diff、平台 + SHA-256 内容去重、provenance 已验证时的冲突候选。
- 集成层 `fetch_policy_sync`：HTTPS 或仅 localhost HTTP、禁用重定向、20s 超时、`If-None-Match`、Bearer、`x-digilegacy-signature` HMAC-SHA256 校验（`sha256=<hex>`、`sha256:<hex>` 或裸 hex），签名缺失/无效 fail closed；新增 3 个签名/端点单测。
- Web 政策页：同步条、上次同步本地化时间、待审核/冲突计数、候选卡片、字段级 diff、批准/拒绝、busy 态；无候选隐藏面板；移动端 390px 与亮/暗主题适配。

### Changed

- Web 版本 `0.9.0-m5d`；政策文案拆到 `content/i18n/policies/` 命名空间，Policies 路由仍懒加载；`tools/arch-check.mjs` 更新为 39 命令 / 23 工具 / 56 schema / 21 错误码。
- 新增错误码 `PROVIDER_UNCONFIGURED`（HTTP 409 / cliExit 3 / `error.providerUnconfigured`）；401/403 映射 `PROVIDER_AUTH`，429 `RATE_LIMITED`，5xx `PROVIDER_ERROR`，超时/网络分别 `TIMEOUT/NETWORK`。
- JS gzip 预算 100 → 101 KB、总 gzip 121 → 122 KB；entry 80、vendor 130、route 24、CSS 21 未放宽。
- CLI `policies provenance --allow-write` 恢复真实写入路径；CLI 实测 `policies candidates`、`policies sync`（304）、`policies decide`、`policies provenance` 均通过。

### Fixed

- Axum 候选列表查询参数改为显式字符串解析，避免 `invalid type: string "20", expected i64`。
- 政策列表两处动态 provenance 标签改用 `tp()`，不再显示 `policies.provenance.unverified` 原始 key。
- 候选 diff 行改为紧凑两列；上次同步时间从原始 ISO 串改为 `formatDateTime` 本地化显示。

### Verification

- `./tools/verify.sh` exit 0：**158 Rust + 10 Web**；契约 **39 命令 / 23 工具 / 56 schema / 21 错误码**；schema v16 / 31 表；workspace members=12。
- 预算：入口 **77.27 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **100.35 / 101 KB**、CSS gzip **20.81 / 21 KB**、总 gzip **121.15 / 122 KB**；Policies 路由 15.88 / 24 KB。
- HTTP 端到端：保存 `policy` 连接 → 同步入队 1 个候选（`changed=1/queued=1/conflict=0`）→ 政策详情未变 → 候选列表返回字段 diff → 批准后落库且 provenance=`unverified`；第二次同步返回 304，feed 收到 `if_none_match=feed-etag-*`。
- Web 端到端：候选 2 页面批准后政策 notes 更新；候选 3 页面拒绝后政策不变；空候选面板自动隐藏；候选 4 展示字段 diff、390px 无横向溢出、亮/暗主题与移动端通过。
- CLI 端到端：`policies candidates --status all` 返回真实候选；`policies sync --allow-write` 返回 `notModified=true`；`policies decide 4 --action reject` 返回 rejected 且政策不变；`policies provenance 1 --status unverified --allow-write` 写回成功。

## [0.9.0-m5c] - 2026-11-20

### Added

- `connections.upsert` / `connections.delete`：HTTP `POST /api/v1/connections`、`DELETE /api/v1/connections/{id}`；CLI `connections save/delete`；MCP/Agent 不暴露。新增 `connection.upsert.input.json`、`connection.output.json`、`connection.delete.output.json`，错误码 `CONFIG_KEY_MISSING`（HTTP 409）。
- 连接配置密钥：`DIGILEGACY_CONFIG_KEY_FILE` 的 32 字节密钥；缺省由 `DIGILEGACY_DB_KEY_FILE` 经 HKDF 派生（域标签 `digilegacy/connection-config/v1`）；两者都缺失时写接口拒绝。配置以 `enc:v2` 整体加密，读模型不回显 endpoint/token/username/from/security。
- 连接来源枚举改为 `env | stored`；环境连接只读，改/删返回 `CONFLICT`；新增同类型同标签冲突，编辑必须带 id，不能改变 kind。
- Web Settings → 连接页：新增/编辑/删除表单，凭据只写不读，环境行只读，测试按钮，`CONFIG_KEY_MISSING`/写令牌缺失提示；390px 单列布局与亮/暗主题适配。
- `core/integrations` 新增 `SmtpTarget` / `send_smtp`：隐式 TLS、STARTTLS、明文、AUTH PLAIN、UTF-8 主题 base64、正文 CRLF 与 dot-stuffing；rustls + 系统 CA。
- 通知投递新增 `NotificationTarget::Smtp`：优先已启用 saved SMTP 连接，回退 `DIGILEGACY_SMTP_HOST/PORT/USERNAME/PASSWORD/FROM/SECURITY`；`DIGILEGACY_NOTIFY_CHANNEL=smtp` 可显式选择；自动顺序为 saved HTTP → env HTTP → SMTP。
- 新增本地明文 SMTP server 单测与 saved SMTP 配置解析/通知投递单测；`connections.test` 读取已保存连接的加密配置取 endpoint/token，无存储行仍可 ad-hoc，不写回。

### Changed

- Web 版本 `0.9.0-m5c`；Settings 路由新增连接管理 UI，入口预算仍 80 KB，JS gzip 预算 97 → 100 KB，总 gzip 118 → 121 KB。
- `tools/arch-check.mjs` 契约计数更新为 36 命令 / 22 工具 / 49 schema / 20 错误码。

### Fixed

- 删除连接后使用 `reload` 刷新列表，避免静默刷新留下旧行。
- 环境连接测试不再带 `id` 走存储配置读取，改为 ad-hoc 探针，避免 `config_enc='env'` 被误当加密 JSON。
- SMTP EHLO 只在明文/TLS 路径发送一次；STARTTLS 路径在 TLS 握手后重发；IPv6 主机连接地址加 `[]`。

### Verification

- `./tools/verify.sh` exit 0：**146 Rust + 10 Web**；契约 **36 命令 / 22 工具 / 49 schema / 20 错误码**；schema v15 / 30 表；workspace members=12。
- 预算：入口 **79.31 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **98.75 / 100 KB**、CSS gzip **20.50 / 21 KB**、总 gzip **119.25 / 121 KB**。
- HTTP 实测：保存 SMTP 连接返回 `source=stored`，列表不回显 token；探针 `TCP 已连接` 并回写 `ok`；环境连接只读；无配置密钥返回 `CONFIG_KEY_MISSING`；Web 表单新增/编辑/删除、390px 与暗色主题通过。
- SMTP 端到端：已保存 SMTP 连接投递 1 条 outbox 到本地 sink，sink 收到中文主题/正文，outbox 置 `sent`；手动发送返回 `sent=0/pending=0`。
- 未做真实 STARTTLS/TLS 服务器联调；当前只验证安全模式解析、TLS 客户端编译与本地明文链路。

## [0.9.0-m5b] - 2026-11-19

### Added

- `connections.list` 只读连接健康页与 `connections.test` 写权限真实探针：LLM / 政策 / SMTP / IMAP / 心跳 / 通用 Webhook / 飞书 / 企业微信 8 类。
- 环境变量连接以 `config_enc='env'` 仅存元数据；`core/domains/src/connections.rs` 提供列表、单条读取、健康回写和环境种子；读模型不返回配置。
- 通知通道扩展：通用 Webhook 保留原 payload；飞书 `msg_type=text`；企业微信 `msgtype=text`；`DIGILEGACY_NOTIFY_CHANNEL` 显式选择，未设置时按已配置 URL 自动选择。
- Web 设置连接页：写令牌、连接列表、来源、最近检查、状态、失败原因、单行测试按钮、成功/失败提示。
- CLI `connections list/test`；MCP 仅暴露只读 `connections.list`；HTTP `GET /api/v1/connections` 与写 token 保护的 `POST /api/v1/connections/test`。

### Fixed

- `connections.test` 从 `network` 权限改为 `write`，HTTP 走 `DIGILEGACY_API_TOKEN`，避免网络命令被 HTTP 永久 403，同时保留出站探针的显式授权。
- reqwest blocking 探针改在普通 OS 线程创建/销毁，修复从 Tokio `spawn_blocking` 关闭内部 runtime 时的 panic；HTTP 测试返回完整 JSON，不再空响应。
- 探针错误统一脱敏 URL，防止 Webhook token 进入 `last_error`、列表或审计；成功状态不保留 `HTTP 200` 作为错误文本。
- 启动时只启用当前环境可见的通道，不用 CLI 的局部环境反向停用服务端已存在连接。

### Verification

- `./tools/verify.sh` exit 0：**138 Rust + 10 Web**；契约 **34 命令 / 22 工具 / 46 schema / 19 错误码**；schema v15 / 30 表。
- 实测 Webhook 探针返回 `ok · HTTP 200` 并回写 `last_status=ok`；Connections 页面桌面与 390px 移动端均无横向溢出，移动端 `scrollWidth=390`。
- 实测飞书与企业微信各投递 1 条真实文本 payload，发件箱置 `sent`；通知测试行与临时通道元数据已清理。
- 预算：入口 **78.85 / 80 KB**、vendor 99.50 / 130 KB、JS gzip **96.84 / 97 KB**、CSS gzip **20.24 / 21 KB**、总 gzip **117.08 / 118 KB**。

## [0.9.0-m5a] - 2026-11-19

### Added

- 新增 `notification_outbox`（schema v15 / 30 表）：final 触发阶段持久化通知，唯一约束 `(trigger_event_id, recipient)`。
- `core/domains/src/notify.rs`：收件人解析（紧急联系人/主继承人/执行人，邮箱优先）、去重、无接收人 `blocked`、pending/sent/failed 状态机。
- `notifications.list/send`：HTTP/CLI/Web 接入；后台 worker 每 60 秒处理 pending；`retryFailed` 可将 failed 行重新排队；`DIGILEGACY_WEBHOOK_URL` + 可选 Bearer token。
- Web 规划页“最终通知”发件箱：状态、失败原因、手动发送、未配置提示；CLI `notifications list/send`；MCP 只读列表。

### Fixed

- `POST /api/v1/notifications/send` 使用 `spawn_blocking` 执行 blocking webhook client，避免 Tokio 运行时 drop panic；实测发送成功。
- 发送结果 `pending` 改为查询真实剩余量，不再受 50 条批次上限影响。
- 诊断测试与 schema v15 / 30 表同步。
- UI 里程碑标识更新为 M5；入口预算 76 → 80 KB，总 gzip 仍 115.42 / 118 KB。

### Verification

- `./tools/verify.sh` exit 0：**131 Rust + 10 Web**；契约 **33 命令 / 21 工具 / 45 schema / 19 错误码**；schema v15 / 30 表。
- v14 数据库重启自动迁移 v15；final 评估生成 2 条通知；mock webhook 2 条成功并写审计；无 webhook 时 `configured=false`，无 panic。
- Web 1920 桌面与 390px 移动端通知卡片无横向溢出。

## [0.8.9-m4i] - 2026-11-19

### Changed

- 新增 `AssetEditor`：资产新增/编辑共用一套字段、校验、提交与样式，替换两处重复表单。
- 新增 `features/assets/model.ts`：统一 `ASSET_KINDS`、`assetKindKey` 与 `draftFromAsset`。
- 新增 `shared/format.ts`：统一 `formatDate` / `formatDateTime`，替换资产、规划、审批、运行、自动化页面的 5 处重复实现。
- 资产详情移除重复说明块；资产页移除重复模式胶囊；编辑时隐藏“编辑资产”按钮。
- 移动端资产表隐藏类型/平台/更新时间列，首列副标题补类型，取消 720px 固定最小宽度，避免横向溢出。
- 删除未使用的 `/deliverables/:id` 占位详情路由与 `ScaffoldView`，移除对应死翻译键；概览运行说明改用 `overview.runtime.note`。
- 版本 `0.8.9-m4i`；契约与 schema 不变。

### Verification

- `./tools/verify.sh` exit 0：127 Rust + 10 Web；契约 31/20/41/19；schema v14/29。
- 预算：入口 75.51 KB / 76 KB、JS 94.48 KB / 97 KB、CSS 19.74 KB / 21 KB、总 114.22 KB / 118 KB。
- 实测共享资产编辑器在资产页与详情页均可打开，创建/编辑逻辑保持。

## [0.8.8-m4h] - 2026-11-19

### Added

- `content/assets/catalog.json`：8 个分类、36 条资产建议，包含中英文名称/理由、类型与平台。
- `assets.discover`：按分类/关键词读取建议，按现有资产 name+platform 标记 `exists`；只读，MCP/Agent 可调用。
- `assets.bulkCreate`：预校验后批量创建，最多 50 项；同名同平台自动跳过；返回 `created/skipped/items`，写 `assets.bulk_create` 审计。
- Web `/assets/discover`：三步资产发现向导，分类卡片、建议勾选、已入库禁用、确认清单、成功统计；资产页增加“发现向导”入口。
- CLI `assets discover --category --q`、`assets bulk-create --file <json> --allow-write`。
- HTTP `GET /api/v1/assets/discover`、`POST /api/v1/assets/bulk`。

### Changed

- 契约扩至 **31 命令 / 20 工具 / 41 schema / 19 错误码**；schema 仍 v14 / 29 表。
- Web 预算调整为入口 76 KB、JS gzip 97 KB、CSS gzip 21 KB、总 gzip 118 KB。

### Verification

- Rust 127 项、Web 10 项通过；`./tools/verify.sh` exit 0。
- HTTP：无 token 批量写 401；带 token 首次 `created=2`，重跑 `created=0/skipped=1`。
- CLI：`assets discover --category finance` 返回 6 条；批量文件导入成功 1 条；无 `--allow-write` 被拒绝。
- Web：实测“资金账户 → 微信零钱/支付宝余额 → 确认”成功新建 2 条；再次进入显示“已在库”且不可勾选。
- 预算：入口 74.68 KB / 76 KB、JS gzip 95.86 KB / 97 KB、CSS gzip 20.30 KB / 21 KB、总 gzip 116.17 KB / 118 KB；发现页路由块 8.72 KB。

### Known limits

- 目录是静态建议，不是网络扫描；不会自动发现真实账号。
- 批量导入仅接受 JSON 文件，不解析 CSV/Excel；同名同平台按精确匹配跳过。
- 发现向导只创建资产记录，不写入凭据、估值或继承人；导入后仍需逐条补充。
- 一次最多 50 项；建议内容更新需改 `catalog.json` 并重新构建。

## [0.8.7-m4g] - 2026-11-19

### Added

- schema v14：`trigger_events` 事件表，`dms_config` 增加 `triggered_at` / `trigger_stage`；29 表。
- `trigger.configure`：开关、签到间隔（1..365 天）、暂停/恢复（0..3650 天）；暂停或关闭会中止当前触发。
- `trigger.evaluate`：按 stage1/stage2/stage3 等待期推进到最终阶段，阶段推进幂等，只写一次事件。
- `trigger.events.list`：只追加触发时间线，支持游标分页；MCP 只暴露读取。
- `trigger.checkin` 清空触发状态并写入 `checkin` 事件；签到后重新布防。
- HTTP `PATCH /api/v1/trigger`、`POST /api/v1/trigger/evaluate`、`GET /api/v1/trigger/events`、`POST /api/v1/trigger/checkin`。
- CLI `trigger configure / evaluate / events`。
- Web 规划页：启用开关、间隔输入、保存、暂停/恢复、评估、阶段/暂停至/事件时间线。
- server 启动 30 分钟 DMS 自动评估线程；仅在状态或阶段变化时写 `trigger.evaluate.auto` 审计。

### Changed

- `plan.get` 的 trigger 增加 `pausedUntil`、`triggeredAt`、`stage`；状态枚举增加 `triggered`。
- 契约扩至 **29 命令 / 19 工具 / 37 schema / 19 错误码**。

### Verification

- Rust 123 项、Web 10 项通过；`./tools/verify.sh` exit 0，日志 `/tmp/digilegacy-verify-m4g.log`。
- HTTP 实测：无 token 配置 401；带 token 配置 `armed`；过期签到评估 `triggered/stage=1`；事件返回 `stage1`；签到后 `armed`、`triggeredAt=''`。
- Web 实测：暂停 7 天显示 `paused` + 暂停至；恢复、评估、时间线可用。
- CLI 实测 `trigger configure/evaluate/events`。
- 预算：入口 70.41 KB / 72 KB、JS gzip 91.85 KB / 93 KB、CSS 18.78 KB / 19 KB、总 gzip 110.63 KB / 112 KB。

### Known limits

- 自动评估只在平台进程运行期间生效；关机期间不会触发。
- 外部通知/邮件发件箱尚未接入；事件只落本地时间线。
- stage 等待期使用 `dms_config.stage1_wait_days / stage2_wait_days / stage3_wait_days`，暂未提供逐阶段配置 UI。

## [0.8.6-m4f] - 2026-11-19

### Removed

- 删除 `legacy/` 整个旧应用目录（280 个 tracked 文件，本地约 390MB），不再保留旧源码、旧文档、旧脚本、旧截图。
- 删除 `apps/desktop` 桌面空壳入口；工作区从 13 个成员收敛到 12 个，产品明确为 API-first 平台，只保留 server/CLI/MCP/Web。
- 清理 `.gitignore`、`.gitattributes` 中旧 `digilegacy/src-tauri`、旧 e2e、旧桌面产物路径。
- 更新 README、ARCHITECTURE、CONTRIBUTING、SECURITY、ADR-0004 与计划文档中“legacy 暂存参考”的表述。

### Kept

- `legacy_import` 保留：它是旧备份导入能力，不是旧应用代码；用于 v4/v5 备份迁移，有独立测试。

### Verification

- Rust 119 项、Web 10 项通过；`./tools/verify.sh` exit 0，日志 `/tmp/digilegacy-verify-m4f.log`。
- 工作区 12 个成员；契约 26 命令 / 18 工具 / 33 schema / 19 错误码不变。
- 预算：入口 68 KB、JS gzip 91 KB、CSS 19 KB、总 gzip 110 KB。
- `legacy` 目录和 `apps/desktop` 已不再存在。

## [0.8.5-m4e] - 2026-11-19

### Added

- `plan.get` 新增 `trigger.lastCheckIn` / `nextCheckAt`，由 `dms_config.last_check_in` 与等待天数推导。
- 规划页“完成签到”：调用 `trigger.checkin`，成功提示并静默刷新；写令牌缺失时给出明确提示。
- 签到页展示上次/下次签到时间与触发计时说明。

### Changed

- `plan.output` schema 触发链字段补齐；Web `Plan` 类型同步。
- 版本 `0.8.5-m4e`。

### Verification

- Rust 119 项、Web 10 项通过；全仓 verify 通过。
- 浏览器实测：无令牌时提示写令牌；带令牌签到成功并显示“已签到”时间。
- 预算：入口 68 KB、JS gzip 91 KB、总 gzip 110 KB。

### Known limits

- 签到不会自动启用 DMS；启用/暂停仍由安全设置管理。
- 尚无触发链阶段时间线/邮件发件箱界面。

## [0.8.4-m4d] - 2026-11-19

### Added

- schema v13：`agent_run_log.cancel_requested`，支持运行中协作取消；仍 28 表。
- `agent.runs.cancel` 对 running 返回 `cancelling` 并置位；`waiting_approval` 仍即时取消并过期审批。
- 执行器在步骤边界检查取消标志，保留已完成步骤、取消未执行步骤并写审计；`succeeded` 落库受取消条件保护。
- Web `RunStatus` 增加 `cancelling`，运行详情显示“取消中…”并禁用重复取消。

### Changed

- 运行状态机新增 `cancelling`；契约 schema 同步。
- 版本 `0.8.4-m4d`。

### Verification

- Rust 119 项、Web 10 项通过；全仓 verify 通过。
- schema v13 / 28 表；Store 15 项测试全绿。
- Application：running 取消返回 `cancelling`；`execute_from` 在取消置位后直接落 `cancelled`，未执行工具。
- 预算：入口 68 KB、JS gzip 90 KB、总 gzip 110 KB。

### Known limits

- 协作式取消只在步骤边界生效，单个长工具调用不能被强制抢占。
- 后台 `agent.resume` Job 仍只有 queued 可取消。

## [0.8.3-m4c] - 2026-11-19

### Added

- `queryCache`：读请求 10 秒 TTL、同 key 并发去重、成功才缓存、按域前缀失效。
- 侧栏 hover/focus 路由 chunk 预取，保持首屏最小加载。
- `queryCache` 4 项前端测试：TTL 命中、并发去重、失败不缓存、前缀失效。

### Changed

- `platformClient` 稳定读命令接入缓存；运行/Job 详情显式绕过缓存，避免轮询假实时。
- 写命令按域失效 `assets/policies/agent.runs/agent.approvals/jobs/overview/agent.usage`，并统一失效 `audit`。
- 版本 `0.8.3-m4c`。

### Verification

- Web 测试 10 项通过；Rust 117 项通过；全仓 verify 通过。
- 预算：入口 68 KB、JS gzip 90 KB、总 gzip 110 KB。

### Known limits

- 外部 CLI/Agent 写入不会主动清浏览器缓存，仪表盘数据最多滞后 10 秒。
- 运行中任务协作式取消仍未实现。

## [0.8.2-m4b] - 2026-11-19

### Added

- `agent.usage.summary`：按周期汇总规划 token/成本、运行数、按日与按模型数据；输入 `days` 1..90，默认 30。
- HTTP `GET /api/v1/agent/usage`、CLI `usage summary --days 30`。
- Web 自动化页“用量”卡：规划数、运行数、token、成本、模型明细与未定价提示。

### Changed

- 契约扩至 **26 命令 / 18 工具 / 33 schema / 19 错误码**；`arch-check` 同步。
- 版本 `0.8.2-m4b`。

### Verification

- Rust 测试 **117 项**（domains 14、application 16）；全仓 verify 通过。
- 领域测试证明 token 只从 `agent_plan_log` 汇总，运行继承 token 不双计。
- Application 测试：120-token 计划启动运行后汇总仍为 120，`plans=1/runs=1`。

### Known limits

- 只汇总规划器用量，工具执行成本未计。
- 无 CSV/账单导出；`byDay` 按 UTC 切分。

## [0.8.1-m4a] - 2026-11-19

### Added

- `agent_plan_log`（schema v12，28 表）：记录每次规划调用的模型、输入/输出 token、成本与定价状态。
- `agent.runs.plan` 返回 `planId` 与 `usage`；`agent.runs.start` 支持 `planId`，把用量继承到 `agent_run_log`。
- 运行列表/详情返回真实 `tokens`、`tokensIn`、`tokensOut`、`costMicros`、`usagePriced`、`planId`。
- Web 新建运行页展示规划 token/成本；运行详情新增“用量”卡；未配置单价时明确显示“成本未配置”。

### Changed

- schema v12：`agent_run_log` 增加 `tokens_in`、`tokens_out`、`cost_micros`、`cost_priced`、`plan_id`。
- 版本 `0.8.1-m4a`。
- 契约 schema 扩展 `runs.plan.output`、`runs.start.input`、`run.detail.output`。

### Fixed

- 运行详情在切换不同 Run 时，SSE 可能继续挂在旧 `runId`；改为同时监听 `runId` 与状态，切换后立即重建事件流。
- `jobs.cancel` 先读后写存在竞态：Worker 恰好在读后领取任务时可能返回“取消成功”但实际未取消；改为条件 UPDATE 并检查影响行数，冲突返回 `CONFLICT`。
- `claim_next` 未检查 UPDATE 行数，两个 Worker 竞争同一 queued job 时可能重复执行；领取失败直接返回空，不再执行。
- `planId` 只校验存在、不校验 task，可能把 A 任务的规划成本挂到 B 运行；现在 task 不匹配返回 `INVALID_ARGS`。
- 规划器 usage 兼容 `input_tokens` / `output_tokens`，不再只认 `prompt_tokens` / `completion_tokens`。

### Verification

- Rust 测试 **115 项**（application 15、agent 4）；全仓 verify 通过。
- HTTP E2E：plan `321/87` tokens、`costMicros=495`；run #3 继承 `408 tokens`、`¥0.000495`。
- Web E2E：新建运行页显示 `输入 321 / 输出 87 tokens · ¥0.000495`；运行 #4 显示 `408 tokens · ¥0.000495`。

### Known limits

- 成本只覆盖规划器调用，工具执行成本未计入。
- 无按日/模型聚合报表。
- 真实 DeepSeek/豆包 key 未配置，E2E 使用本地 mock provider。

## [0.8.0-m3i] - 2026-11-19

### Added

- `GET /api/v1/events?runId=<id>` / `?jobId=<id>`：SSE `snapshot` 事件流，服务端 500ms 读一次当前对象，只在中转状态变化时推送，15s keepalive；Web 运行详情与交付页优先 EventSource，失败自动回退 1.5s 轮询。
- `jobs.cancel`：取消排队中的后台任务，置 `cancelled` / `CANCELLED` / `finishedAt`，写 `job.cancel` 审计；HTTP、CLI、IPC 可用，mcp/agent 关闭。

### Changed

- 契约扩至 **25 命令 / 17 工具 / 31 schema / 19 错误码**；`arch-check` 同步。
- 版本 `0.8.0-m3i`；Web 关于页同步。
- 交付页对 queued 导出显示“取消导出”；SSE 连接成功时暂停轮询，断流回退。

### Verification

- Rust 测试 **113 项**：security 49、store 15、domains 13、application 14、agent 3、server 12、foundation 5、render 2；全仓 `./tools/verify.sh` 通过。
- 运行 #2 外部批准后 **0.6s** 由 SSE 更新为“已完成”；浏览器存在 `GET /api/v1/events?runId=2`。
- CLI `jobs cancel` 对 queued job 返回 `cancelled`、`errorCode=CANCELLED`；二次取消 `CONFLICT`。

### Known limits

- SSE 为单进程 DB 轮询，不是广播总线；多实例部署需换事件总线。
- `jobs.cancel` 只支持 queued；running 导出任务无法中断。
- token/成本仍未落库，`Run.tokens` 仍为 0。

所有重要变更记录在此。格式参考 Keep a Changelog，版本按平台里程碑推进。

## [0.7.9-m3h] - 2026-11-19

### Added

- `deliver.export` 支持 `wait=false`：HTTP/Web 入队 `jobs`（kind=`deliver.export`）并立即返回 `jobId`、`status=queued`；Worker 生成 HTML、写 `deliverable_exports`，`jobs.get` 返回文件名/下载地址。
- Web 交付页轮询 `jobs.get`：0.5 秒显示 jobId 与“生成中”，完成后自动切换为“已生成 + 下载 HTML”；失败显示 `errorCode`。
- CLI `deliver export --background` 可入队；默认仍同步，保持脚本兼容。
- `requeue_stale` 覆盖 `agent.resume` 与 `deliver.export`，僵尸任务回收不再只针对审批恢复。
- 失败路径统一落 `jobs.error_code`，同步导出失败也会标记 job `failed`。

### Changed

- `export.input.json` 增加可选 `wait` 布尔，默认 true；HTTP 两条入口强制 false。
- 版本 `0.7.9-m3h`。

### Verification

- Rust 测试 **112 项**：security 49、store 15、domains 13、application 13、agent 3、server 12、foundation 5、render 2；全仓 `./tools/verify.sh` 通过。
- HTTP `POST /api/v1/deliverables/export` **1.462 ms** 返回 `queued` + `jobId`；`jobs.get` 轮询到 `succeeded`；`GET /api/v1/exports/{jobId}` 返回 `200`、1912 bytes HTML。
- Web 交付页 0.5 秒显示后台 jobId，2 秒后自动出现下载链接。
- CLI `deliver export --background` 返回 queued，`jobs get` 返回 succeeded 与文件名。
- Web 总 gzip **105.33 KB / 110 KB**，CSS **18.20 KB / 19 KB**，入口 **63.84 KB / 64 KB**，vendor **98.89 KB / 130 KB**。

## [0.7.8-m3g] - 2026-11-19

### Added

- durable `jobs` 队列：`agent.resume` 后台执行审批恢复；server 启动单进程 Worker，250ms 轮询，回收超过 120 秒的 `running` 僵尸任务。
- `jobs.get`（read）：HTTP `GET /api/v1/jobs/{id}`、CLI `jobs get`、MCP/Agent 工具；返回状态、进度、结果、错误与时间戳。
- HTTP 审批后台化：`agent.approvals.decide` 在 HTTP/通用 invoke 路径强制 `background=true`，批准只更新审批并返回 `jobId`，Resume 交给 Worker；CLI 默认同步，新增 `--background`。
- `agent_run_log.duration_ms`（schema v11，仍 27 表）：运行详情与列表返回 `durationMs`，记录从启动到终态的耗时（含等待审批）。
- Web 审批页显示“已批准，后台正在执行”、`jobId` 与“查看运行”；运行详情对 `queued/running/waiting_approval` 每 1.5 秒静默刷新，页面隐藏暂停，并显示耗时。

### Changed

- 契约扩至 **24 命令 / 17 工具 / 31 schema / 19 错误码**；`arch-check` 同步，`jobs.get` 加入工具目录。
- `useResource` 拆分 `reload` / `refresh`：重试仍显示 skeleton，轮询静默更新不闪烁。
- 版本 `0.7.8-m3g`。

### Verification

- Rust 测试 **111 项**：security 49、store 15、domains 13、application 12、agent 3、server 12、foundation 5、render 2；全仓 `./tools/verify.sh` 通过。
- HTTP 批准耗时 **1.329 ms**，返回 `jobId`；Worker 约 **45 ms** 完成；运行 1 `succeeded`、`durationMs=4330`、步骤 `done`；`jobs.get` 返回 `succeeded`、`progress=1`。
- Web 审批页显示后台提示与 `job_...`；运行 4 详情显示耗时 **8.4 s**；运行 5 在等待审批时从外部批准，2.5 秒后页面自动刷新为“已完成”。
- CLI `approvals decide --background` 返回 `jobId`，`jobs get` 返回 `succeeded`；运行 6 `succeeded`、`durationMs=6945`。
- Web 总 gzip **104.98 KB / 110 KB**，CSS **18.20 KB / 19 KB**，入口 **63.80 KB / 64 KB**，vendor **98.89 KB / 130 KB**。

## [0.7.7-m3f] - 2026-11-19

### Added

- `agent.runs.plan`（draft）：用 OpenAI 兼容接口（默认 DeepSeek）把任务拆成注册工具步骤，只生成计划、不执行；工具目录给规划器带上权限、标题、说明和输入 schema。HTTP `POST /api/v1/agent/plan`，CLI `runs plan`，Web 新建运行页“AI 生成步骤”。
- `agent.runs.cancel`（write）：取消未终态运行，步骤置 `cancelled`，该运行的 pending 审批置 `expired`，写 `agent.run.cancelled` 审计；HTTP `POST /api/v1/agent/runs/{id}/cancel`，CLI `runs cancel`，Web 运行详情“取消运行”。
- `agent.runs.retry`（write）：为 failed/rejected/cancelled 运行创建新运行，保留旧运行记录；HTTP `POST /api/v1/agent/runs/{id}/retry`，CLI `runs retry`，Web 运行详情“重试运行”。
- `agent.runs.start` 增加 `maxSteps` / `maxWriteSteps` 静态预算；超过预算直接返回 `INVALID_ARGS`，不会创建运行。
- 规划器错误映射稳定：未配置 `UNSUPPORTED`、网络 `NETWORK`、超时 `TIMEOUT`、认证 `PROVIDER_AUTH`、服务端 `PROVIDER_ERROR`、返回结构非法 `INVALID_ARGS`。
- 运行详情路由参数变化时自动重载，修复重试后跳转仍显示旧运行的前端问题。

### Changed

- `ToolSpec` 扩展为 `name/command/permission/title/description/input_schema`，生成器把工具输入 schema 嵌入 Rust 契约。
- 契约扩至 **23 命令 / 16 工具 / 30 schema / 19 错误码**；`arch-check` 同步，取消/重试加入编排命令例外。
- 版本 `0.7.7-m3f`。

### Verification

- Rust 测试 **109 项**：security 49、store 15、domains 13、application 10、agent 3、server 12、foundation 5、render 2；全仓 `./tools/verify.sh` 通过（`/tmp/digilegacy-verify-m3f.log`）。
- 本地 OpenAI 兼容 mock 规划器实测：`POST /api/v1/agent/plan` 返回 `model=mock-planner` 与 `overview.get + assets.list` 两步。
- Web 新建运行页“AI 生成步骤”填充步骤 JSON，显示规划器模型；启动后运行完成。
- HTTP 写运行取消后步骤 `cancelled`、审批 `expired`；对过期审批再次决定返回 `409 CONFLICT`。
- HTTP 拒绝运行后 `runs retry` 返回新运行 `waiting_approval`，旧运行仍为 `rejected`；Web 重试跳转新运行。
- HTTP 超预算写运行返回 `400 INVALID_ARGS`（2 个写步骤 / 预算 1）。
- CLI `runs plan` / `runs retry` / `runs cancel` 实测通过。
- Web 总 gzip **104.15 KB / 110 KB**，CSS **18.12 KB / 19 KB**，入口 **62.82 KB / 64 KB**，vendor **98.88 KB / 130 KB**。

## [0.7.6-m3e] - 2026-11-19

### Added

- `agent.runs.start`（write）：按步骤执行注册工具；只读/草稿立即执行，`write` 工具创建审批并暂停，返回 `waiting_approval`。
- `agent.runs.get`（read）：读取任务、状态、逐步工具调用、结果、工具集合与最终说明；HTTP `GET /api/v1/agent/runs/{id}`，CLI `runs get`，MCP 开放。
- 确定性 Runtime：批准审批后从当前步骤恢复执行，拒绝则终止运行；步骤落 `agent_run_log.steps_json`，并写 `agent.run.start/succeeded/rejected/resume` 审计。
- Tool Registry 双解析：`overview.get` 等工具名自动映射到 `app.overview` 等命令名；Runtime 只接受注册工具。
- Web `/automation/runs/new` 新建运行、`/automation/runs/:id` 运行详情、`/automation/tools` 工具目录；自动化页增加“新建运行”。

### Changed

- `agent.runs.list` CLI 从 `runs --status` 改为 `runs list --status`。
- 契约扩至 20 命令 / 16 工具 / 28 schema / 19 错误码；`arch-check` 同步。
- Web 预算调整：入口 60→64 KB，JS 85→88 KB，CSS 17→19 KB；总 gzip 103.41 KB / 110 KB。
- 版本 `0.7.6-m3e`。

### Verification

- `core/domains` 13 tests；`core/application` 7 tests；`apps/server` 12 tests；全仓 `./tools/verify.sh` 通过（`/tmp/digilegacy-verify-m3e.log`）。
- HTTP 只读运行成功；写运行暂停生成 `appr_2_1`，批准后运行完成、资产 value 4200→5000。
- Web 新建写运行后跳审批页，批准后运行完成、资产 value 5200。
- CLI `runs get 2` 返回完整步骤；无 token 启动运行返回 401。
- Web 总 gzip **103.41 KB / 110 KB**，入口 **61.81 KB / 64 KB**，vendor **98.88 KB / 130 KB**。

## [0.7.5-m3d] - 2026-11-19

### Added

- `agent.approvals.list`（read）与 `agent.approvals.decide`（write）：`agent_approvals` 队列读取，按状态过滤；批准/拒绝只能从 `pending` 转移一次，非 pending 返回 `409 CONFLICT`。
- `agent.approvals.decide` 写审计 `agent.approval.decide`；审批只记录人工决定，不执行工具本身。
- HTTP `GET /api/v1/agent/approvals`、`POST /api/v1/agent/approvals/{id}/decide`；CLI `approvals list/decide`；Web `/automation/approvals` 审批页与自动化页真实待批队列。
- `DomainError::Conflict` 与 `CONFLICT` 错误映射。

### Changed

- 契约扩至 18 命令 / 15 工具 / 26 schema / 19 错误码；`arch-check` 同步。
- Web CSS 预算从 16 KB 调至 17 KB（审批页 2.2 KB CSS，总 gzip 仍 95.83 KB / 110 KB）。
- 版本 `0.7.5-m3d`。

### Verification

- `core/domains` 12 tests；`apps/server` 11 tests；全仓 `./tools/verify.sh` 通过（`/tmp/digilegacy-verify-m3d.log`）。
- HTTP 无 token 审批返回 401；带 token 批准 200，二次拒绝 409；CLI 待批从 2 降到 1；审计出现 2 条 `agent.approval.decide`。
- 浏览器审批页批准后待批归零、空态出现；自动化页接入真实待批队列。
- Web 总 gzip **95.83 KB / 110 KB**，入口 **56.43 KB / 60 KB**，vendor **98.88 KB / 130 KB**。

## [0.7.4-m3c] - 2026-11-19

### Added

- `audit.list`（read）：按 id 倒序分页读取审计哈希链，支持 `cursor` 与 `kind` 过滤，返回 `items/nextCursor/total`。
- Store 新增 `audit_page` / `audit_count`，不在 Application 或 HTTP 层重复拼接 SQL。
- HTTP `GET /api/v1/audit`；CLI `audit list --kind --limit --cursor`；MCP 工具由契约生成。
- Web 设置 → 审计：事件类型过滤、总数、时间/类型/对象/结果/hash 前缀表。

### Changed

- 契约扩至 17 命令 / 14 工具 / 24 schema / 19 错误码；`arch-check` 同步。
- 版本 `0.7.4-m3c`。

### Verification

- `core/domains` 11 tests；`apps/server` 10 tests；全仓 `./tools/verify.sh` 通过（`/tmp/digilegacy-verify-m3c.log`）。
- HTTP `GET /api/v1/audit?limit=2` 返回 14 条总数与 `nextCursor=13`；`kind=asset.update` 返回 1 条。
- Web 设置 → 审计显示 14 条真实记录与 hash 前缀；筛选下拉可用。
- Web 总 gzip **92.65 KB / 110 KB**，入口 **54.24 KB / 60 KB**，vendor **98.88 KB / 130 KB**。

## [0.7.3-m3b] - 2026-11-19

### Added

- HTTP `write` 命令接 `DIGILEGACY_API_TOKEN`：`Authorization: Bearer <token>`，常量时间比较；未配置/错误 token 返回 `401 AUTH_REQUIRED`。
- Web 设置 → 连接新增平台写令牌：只存 `sessionStorage`，只对 `write` 命令附加；清除即时生效。
- 资产新增/编辑表单：`assets.create` / `assets.update` 走 Platform API；写入后刷新列表/详情，审计 `asset.create` / `asset.update`。
- 资产详情备注显示修正为 `note`，不再误显示估值。

### Changed

- `draft` 命令按契约继续开放，不要求 token；`policies.provenance.set` 的 `requiresUnlock` 明确为 `false`，避免“声明需要解锁但未执行”的假承诺。
- 契约规模保持 16 命令 / 13 工具 / 22 schema / 19 错误码；没有新增命令，只补齐既有 `write` 入口与 UI。

### Verification

- `cargo test -p digilegacy-server` 9 通过（含正确/错误 token、draft 溯源写与审计）。
- HTTP：无 token `assets.create` → 401；带 `Bearer test-write-token` → 200，返回资产 id=2；Web 创建“微信零钱”id=3、编辑估值 3500→4200 成功。
- 审计：`GET /api/v1/overview` 可见 `asset.create`、`asset.update`。
- 全仓 `./tools/verify.sh` 通过，日志 `/tmp/digilegacy-verify-m3b.log`。
- Web：总 gzip **91.66 KB / 110 KB**，入口 **53.50 KB / 60 KB**，vendor **98.88 KB / 130 KB**。

## [0.7.2-m3] - 2026-11-19

### Added

- 政策溯源写：`policies.provenance.set`（draft / 免解锁 / idempotent / audit），四态 `unverified/source_cited/verified/disputed`。
- 溯源写入规则：来源为空回退官方 URL；备注为空保留旧备注；`verified/source_cited` 写 `verified_at`；写 `policy_provenance` 后追加审计 `policy.provenance.set`。
- HTTP `PATCH /api/v1/policies/{id}/provenance`；CLI `policies provenance <id> --status ... --source-url ... --note ...`；MCP 工具由契约生成。
- Web 政策详情新增“记录来源核实”编辑器：状态下拉、来源 URL、核对备注、保存与“已保存，审计已记录”；保存后同步更新列表状态。
- 契约扩至 16 命令 / 13 工具 / 22 schema / 19 错误码；生成器与 `arch-check` 同步。

### Verification

- `cargo test -p digilegacy-domains` 10 通过；`-p digilegacy-application` 5 通过；`-p digilegacy-server` 7 通过。
- HTTP `PATCH /policies/1/provenance` 返回 `verified` 与 `verifiedAt`；CLI `policies provenance` 返回 `source_cited`。
- Web 浏览器实测：详情编辑区选“已核实”、填备注、保存，详情状态更新，提示“已保存，审计已记录”。
- `GET /api/v1/overview` 出现三条 `policy.provenance.set` 审计；`arch-check: OK`。

## [0.7.1-m2b] - 2026-11-19

### Added

- 政策库：`policies` 原生表（schema v10、27 表）、`policy_provenance` 独立溯源、152 条平台政策内容源 `content/policies/platforms.json`。
- `core/domains/policy`：`seed_if_empty`、`list`（分类 + 关键词 + 游标）、`get`、`all`、`find_by_name`；种子状态一律 `unverified`，不伪造核实。
- `core/application` 接入 `policies.list` / `policies.get`；`from_env` 打开 Store 后自动 seed，空表才写入。
- HTTP `GET /api/v1/policies`、`GET /api/v1/policies/{id}`；CLI `policies list/get`；MCP 工具由契约生成。
- 交付物：`deliver.export` 真实生成 report/checklist/legal/toolkit 四类 HTML；`jobs`、`deliverable_exports` 落库；`GET /api/v1/exports/{jobId}` 下载，路径 canonical 校验防越界。
- 法律模板内容源 `content/legal/templates.json`，四类模板进入法律文本交付物，并附“需执业律师审阅”提示。
- Web：政策库并入资产二级标签（`/policies`），列表服务端搜索/分类/游标加载更多，详情展示材料、流程、风险、来源与未核实状态。
- Web 交付页接 `deliver.export`：四类生成按钮、结果文件、下载链接、会话语义记录；资产详情可跳转对应平台政策。
- `core/render` 增加可信 body 的可打印页面壳 `render_page`，标题仍转义。

### Changed

- 概览模块把 `policy`、`deliver` 标为 `ready`。
- Web 保持 5 个主入口：政策库不是第 6 个导航，作为资产域二级视图，避免主导航膨胀。
- 契约扩至 15 命令 / 12 工具 / 21 schema / 19 错误码；生成器 domain 集合加入 `policy`。

### Verification

- `./tools/verify.sh` 全绿，日志 `/tmp/digilegacy-verify-m2b.log`。
- Rust：security 49、store 15、domains 9、application 5、server 6、foundation 5、render 2。
- Web：typecheck、6 tests、build、bundle 预算通过；总 gzip **87.29 KB / 110 KB**，入口 raw 48.77 KB / 60 KB，vendor raw 98.88 KB / 130 KB。
- CLI：`policies list --q 支付宝` 命中；`policies get 1` 返回支付宝与 `unverified`；`deliver export --kind report` 生成 HTML。
- HTTP：政策列表/详情返回真实行；`POST /api/v1/deliverables/export` 返回 `jobId`；`GET /api/v1/exports/{jobId}` 返回 `text/html` 附件。
- 浏览器：政策页 152 条、详情可展开、深/亮主题正常；交付页点击生成后出现文件名与下载链接。

### Known limitations

- 政策溯源目前只读：没有“标记已核实/争议”的写命令与审核界面。
- 交付物为同步生成；Job 状态已落库，但未接 SSE/轮询进度与取消。
- Web 写操作仍受 HTTP 写 token 与解锁流程阻塞；资产创建/更新走 CLI `--allow-write`。
- 政策内容按来源整理，尚未逐条人工核对；页面明确显示 `unverified`。

## [0.7.0-m2] - 2026-11-19

### Added

- `core/application` 用例层：HTTP、CLI、MCP 共享同一套 `Application::dispatch`，入口不再各写业务分支。
- `core/domains` 真实领域：资产列表/详情/创建/更新、规划摘要、概览投影、Agent 运行记录、安全摘要。
- 资产领域适配 v9 `digital_assets`：`name/kind/platform/value/note` 映射旧列；平台自动落 `platforms`；空平台使用 `id=0` 占位。
- 触发签到：写入 `check_in_log`、更新 `dms_config.last_check_in`、返回 `nextCheckAt` 并写审计。
- HTTP 读接口：`/api/v1/overview`、`/assets`、`/assets/{id}`、`/plan`、`/security`、`/agent/runs`；写命令仍由 token 门禁拦截。
- CLI 真实子命令：`overview`、`assets list/get/create/update`、`plan`、`security status`、`runs`；写命令要求 `--allow-write`。
- MCP `tools/call` 接真实用例：工具 `_meta["digilegacy/command"]` 映射命令，失败返回 `isError=true` 与平台错误码。
- Web 默认 live：`VITE_USE_MOCK=false`；资产搜索/状态筛选改为服务端查询，游标分页“加载更多”，前端不再重复过滤。
- 安全设置页接 `security.status`：锁定状态、恢复配置、密钥来源、最近备份。
- 架构门禁扩到 13 个 workspace 成员；`core/application` 纳入 core 边界检查。

### Changed

- 平台 API 的 `health` 返回 schema 版本、表数、Store 状态；`mode=m2`。
- 前端状态文案从 M0 骨架切换为 M2 数据在线；活动时间改为本地时间；规划页补空态与触发状态本地化。
- 概览、规划、自动化、设置页的错误标题改用通用数据错误，不再误报“能力未接入”。

### Verification

- `./tools/verify.sh` 通过，日志 `/tmp/digilegacy-verify-m2-final.log`。
- Rust：security 49、store 15、domains 7、application 3、server 6、foundation 5、render 1。
- Web：typecheck、6 tests、build、bundle 预算通过；总 gzip 80.60 KB / 110 KB。
- CLI 实测创建资产返回真实 id；HTTP 实测 overview/assets/plan/security/runs 返回真实行；MCP 实测 `overview.get` 返回真实 envelope。

### Known limitations

- Web 写操作尚未接 HTTP 写 token / 解锁流程；资产创建与更新目前走 CLI 本地特权入口。
- 资产领域暂未接批量导入导出、附件、凭据解密展示。
- 政策库 152 条、交付物、审核、演练、SSE/Job 仍未开始。
- vault 解锁未接 server/Web；`security.status` 会显示锁定，但元数据读链路可读。

## [0.7.0-m1] - 2026-11-19

### Added

- 安全内核迁入 `core/security`：AES-256-GCM、PBKDF2 v1/v2、Argon2id v3、HKDF 域分离、KEK/DEK 两层密钥、v1/v2 字段格式兼容。
- 保险库生命周期：`create_vault`、`unlock`、`unlock_with_recovery`、`lock`、`change_password`；改密只重包 DEK，不重加密全库字段。
- 恢复码封印 RS3/RS2/v1 三代兼容；Shamir 2..10 份、默认 3-of-5、份额校验和/批次指纹、可选口令保护、可打印恢复包 HTML。
- 文件 KEK 托管 `FileKeyring`（Unix 0600），用于服务器/CI 的 SQLCipher 密钥。
- `core/store`：SQLite/SQLCipher 单写连接 + 只读连接、schema v9（26 张表）、`schema_migrations`、additive 迁移、`quick_check`/`integrity_check`。
- 旧 v4/v5 JSON 备份导入：源主密码先验证、v8 表映射到 v9、连接配置合并到 `connections`、replace 前清业务表。
- 认证备份：DB 文件 SHA-256 + backup 域 HMAC；篡改 DB 或清单均拒绝。
- 命名检查点：最多 12 份、带标签/自动标记、认证备份、校验后恢复。
- 整库加密迁移：`sqlcipher_export` 明文→加密、加密→明文，保留 schema 版本。
- 审计链：每行 SHA-256(prev || at || kind || subject || outcome || detail)，`verify_audit_chain` 定位首个断点。
- 诊断包：schema、表计数、完整性、审计链状态；不导出任何数据值。

### Changed

- `core/security` 状态 `m1-security`；`core/store` 状态 `m1-store`。
- 测试基线：security 49、store 15；全仓 `tools/verify.sh` 通过。

### Known limitations

- M1 仅完成数据/安全底座，尚未把 vault/store 生命周期接到 server/CLI/Agent 工具。
- 旧备份导入已覆盖 v4/v5 主要表，尚无 10 份真实旧库样本的端到端回归清单。
- 桌面 DPAPI 密钥托管留到 M3；当前服务器/CI 使用文件 KEK。

## [0.7.0-m0] - 2026-11-19

### Added

- 新仓库骨架：`core/` 8 个 crate、`apps/` 4 个入口、`web/`、`contracts/`、`content/`、`tools/`、`docs/`。
- 契约单一事实源：13 个命令、10 个工具、18 个 schema、19 个错误码。
- `tools/generate.mjs`：生成 Web IPC/mock/tools、Rust 命令表/错误表、CLI/Server/MCP manifest、OpenAPI。
- Rust `CommandBus`、Envelope、错误映射、权限分档、trace id。
- axum Platform API：health、commands、overview、invoke；写入在 M0 返回 `AUTH_REQUIRED`，network/key/desktop 返回 `FORBIDDEN`。
- clap CLI：`commands --json`、`health`、`invoke`、生成式 help。
- stdio MCP：initialize、ping、tools/list、resources/list；`tools/call` 返回真实 tool error，不伪造成功。
- Vue 3 + Vite 控制台：概览、资产、规划、交付、自动化、设置、详情/空壳、404。
- Agent 抽屉、命令面板 `⌘K`、锁定层、亮暗主题、中英文案、mock 数据、骨架/错误/空态。
- `tools/arch-check.mjs`、`tools/bench/web-budget.mjs`、`tools/bench/core-bus.sh`、`tools/verify.sh`。
- GitHub Actions CI：contracts、rust、web 三组门禁。
- 工程治理文档：定位、架构、验收、DoR/DoD、ADR、RFC 模板、v1.2 计划。

### Changed

- 旧应用整体移入 `legacy/`，退出新构建；旧路由全部 404，不做兼容重定向。
- 平台定位明确为 API-first，Web 为主客户端，Agent 工具一等公民。
- HTTP 写权限口径改为“需要写权限 token”；M0 未接 token，因此写命令保持关闭。
- CLI 移除与 clap 内置 `help` 冲突的显式子命令。

### Known limitations

- M0 领域命令仍返回 `UNSUPPORTED`；真实数据在 M1/M2 接入。
- 安全内核与 SQLite/SQLCipher 尚未迁移，`core/security`、`core/store` 只有边界。
- 桌面 Tauri 窗口/托盘留到 M3/M5。
- `legacy/` 源码尚未删除，仅作迁移参考。
