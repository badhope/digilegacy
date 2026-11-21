# DigiLegacy 0.0.27 修复验收与发布报告

日期：2026-11-21  
版本：0.0.27  
结论：**已做完**。本轮修复三个真实缺陷，重跑全量门禁、真实浏览器路由和干净机安装演练，并完成五平台仓库同步。

## 1. 本轮修复

1. **保险库锁定时仍可下载导出文件。** `GET /api/v1/exports/{job_id}` 以前只校验任务存在，不校验保险库状态。锁定后该接口仍返回文件字节；现在锁定态统一返回 `423 VAULT_LOCKED`，读取安全状态失败时也按锁定处理。
2. **法律文本和家人工具包在资产未匹配政策时出现空白。** 交付渲染现在列出未匹配政策的资产，并为每个无政策资产生成一页兜底卡，家属拿到的材料不再缺项。
3. **政策详情不显示补充说明。** `notes` 字段已写入契约和数据库，但详情页没有渲染；现在新增“补充说明”区块，中英文文案同步。

## 2. 全量门禁

`bash tools/verify.sh` 退出码 0：

| 项目 | 结果 |
| --- | --- |
| 契约 | 44 commands、23 tools、60 schemas、21 errors；arch-check OK |
| Rust | 195 项通过，0 失败 |
| Web | 12 个测试文件、79 项通过，0 失败 |
| 真实浏览器 | 16/16 路由 PASS，横向溢出 0，axe 违规 0 |
| Web 预算 | 总 gzip 130.68 / 131.1 KB |
| actionlint | CI 与 Dependabot auto-merge 工作流 0 问题 |

浏览器证据：`docs/operations/measurements/live-browser-0.0.27-2026-11-21.json`。

真实保险库回归：锁定态导出返回 423；解锁后返回 200；再次锁定回到 423。

## 3. 五平台仓库同步

同一提交已推送到 GitHub `X33834/digilegacy`、`Morningstar202604/digilegacy`、`badhope/digilegacy`，Gitee `badhope/digilegacy`，GitCode `badhope/digilegacy`。五个远端 `main` 指向同一 SHA。

- 英文描述、中文切换链接和核心文档入口已统一。
- GitHub 三个仓库统一 13 个 topics、squash-only、自动合并、合并后删除分支、Discussions；Gitee 19 个 issue labels 与 13 个 project labels；GitCode 5 个 project labels（平台上限）。
- 三个 GitHub 仓库仅保留 `main`，旧 Dependabot PR 与分支已清理。
- 三个 GitHub 仓库启用 `main` 分支保护，要求 `Contracts & architecture`、`Rust core & entries`、`Web console` 三项检查通过，禁止强推和删除分支，要求线性历史和会话解决。
- Dependabot 按 Cargo、npm、GitHub Actions 分组为 minor/patch 单一 PR，忽略 major；`dependabot-auto-merge.yml` 对 patch/minor 开启自动合并，等待三项 CI 通过后 squash 合并并删除源分支。

GitHub Actions 对最新提交 `ccaea95` 的三个 CI 运行均为 `success`。

## 4. 发布包

- 包：`packages/digilegacy-0.0.27-linux-x86_64.tar.gz`
- 大小：18,460,277 B（约 18 MB）
- SHA256：`822068288d3b7e5c26efdbd45883c777f233b2451949e0f98fc568789bf918c4`
- 校验文件：`packages/digilegacy-0.0.27-linux-x86_64.tar.gz.sha256`
- 包内 `VERSION` 为 `0.0.27`。

干净前缀安装演练 12 步全部 PASS，证据：`docs/operations/measurements/install-0.0.27-2026-11-21.json`。安装后健康检查返回 `version: 0.0.27, schemaVersion: 16, tables: 31`，诊断导出、检查点创建与校验、卸载保留数据、重装读取旧库、清理均通过。

## 5. 验收边界

- 真实飞书/企业微信机器人、外部 SMTP 服务商和公网 HTTPS 目标仍未接入；现有验证使用本地假 SMTP 与 httpbin。
- 外部真实政策提供方未接，政策同步仍测本地签名 feed。
- 阶段推进使用 SQLite 时间旅行，不是自然等待 30/60/90 天。
- Gitee 与 GitCode 没有原生 CI 运行记录；本轮 CI 证据来自三个 GitHub 镜像。
- GitCode 的合并方式设置通过 API 提交成功，但平台不回显，无法用接口二次确认。

## 6. 结论

三个缺陷已修复并有回归覆盖，全量门禁、16 条真实浏览器路由、干净机安装和五平台同步均完成。**已做完。**
