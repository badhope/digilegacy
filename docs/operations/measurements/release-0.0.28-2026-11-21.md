# DigiLegacy 0.0.28 界面收敛与发布报告

日期：2026-11-21  
版本：0.0.28  
结论：**已做完**。本轮没有新增业务模块，只收敛资产与交付两处重复/割裂入口，补齐版本单源，重跑全量门禁、真实浏览器和干净机安装演练，并重新出包。

## 1. 本轮调整

1. **批量添加资产并入资产板块。** `/assets/discover` 以前是从资产页跳出去的独立页面，顶部另有“返回资产”按钮。现在资产板块是三个 Tab：资产清单、平台政策、批量添加；`/assets`、`/assets/:id`、`/assets/discover`、`/policies` 各自只高亮对应 Tab，用户始终知道自己还在资产板块。
2. **交付页删除重复结果列表。** 每张交付卡已经包含文件、状态和下载动作，底部“本次导出”只是同一批结果再列一遍，已删除；本地快照说明并入 API-first 边界说明。
3. **版本号只留一个来源。** `tools/generate.mjs` 从 `Cargo.toml` 读取工作区版本生成 OpenAPI，避免发版时 Cargo、Web、契约三处版本漂移。

其余页面不动：政策库已在资产 Tab 内，自动化是运行/审批/工具三个 Tab，设置是连接/安全与数据两个 Tab，详情与新建运行是明确的子页，命令面板与 Agent 抽屉只是快捷入口。

## 2. 全量门禁

`bash tools/verify.sh` 退出码 0：

| 项目 | 结果 |
| --- | --- |
| 契约 | 44 commands、23 tools、60 schemas、21 errors；arch-check OK |
| Rust | 195 项通过，0 失败 |
| Web | 13 个测试文件、82 项通过，0 失败 |
| 真实浏览器 | 16/16 路由 PASS，横向溢出 0，axe 违规 0 |
| Web 预算 | 总 gzip 130.42 / 131.1 KB |

浏览器证据：`docs/operations/measurements/live-browser-0.0.28-2026-11-21.json`。资产批量添加页已实际渲染三个 Tab，交付页已无底部重复列表。

## 3. 发布包

- 包：`packages/digilegacy-0.0.28-linux-x86_64.tar.gz`
- 大小：18,461,922 B（约 18 MB）
- SHA256：`95e48998458b485b9f96c0bddd33dca70746b1e674093445b95cdec8c0d0e3c7`
- 校验文件：`packages/digilegacy-0.0.28-linux-x86_64.tar.gz.sha256`
- 包内 `VERSION` 为 `0.0.28`。

干净前缀安装演练 12 步全部 PASS，证据：`docs/operations/measurements/install-0.0.28-2026-11-21.json`。安装后健康检查返回 `version: 0.0.28, schemaVersion: 16, tables: 31`；诊断导出、检查点创建与校验、卸载保留数据、重装读取旧库、清理均通过。

## 4. 五平台仓库同步

同一提交推送到 GitHub `X33834/digilegacy`、`Morningstar202604/digilegacy`、`badhope/digilegacy`，Gitee `badhope/digilegacy`，GitCode `badhope/digilegacy`。五个远端只保留 `main`，并指向同一提交。

## 5. 验收边界

- 真实 SMTP/Webhook 服务商账号下的投递时延与失败重试仍未验收；现有验证使用本地假 SMTP 与 httpbin。
- 外部真实政策提供方未接，政策同步仍测本地签名 feed。
- 阶段推进使用 SQLite 时间旅行，不是自然等待 30/60/90 天。
- Gitee 与 GitCode 没有原生 CI 运行记录；CI 证据来自三个 GitHub 镜像。
- GitCode 的合并方式设置通过 API 提交成功，但平台不回显，无法用接口二次确认。
- Dependabot 自动合并工作流已配置，但还没有真实 Dependabot PR 触发过。

## 6. 结论

两处界面重复/割裂已收敛，版本单源已落地，全量门禁、16 条真实浏览器路由、干净机安装和五平台同步均完成。**已做完。**
