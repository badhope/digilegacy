# DigiLegacy 0.0.26 全功能补测与发布验收报告

日期：2026-11-21  
版本：0.0.26  
结论：**已做完**。本轮修完继承人联系方式与通知列表两个真实缺口，重跑全量门禁、真实触发链、真实 SMTP/HTTPS 投递、真实浏览器表单与发送失败重试，并重新出包。

## 1. 本轮修复

0.0.25 只覆盖全站正常路径，不是全功能验收。本轮补测跨阶段触发链时发现两个用户可见缺口：

1. **继承人邮箱/电话全链路丢失**。数据库 `heirs` 表本来就有 `email`、`phone` 列，但 `HeirInput`、`Heir`、SQL 新增/更新、契约、生成 mock 和规划页表单都没有接通。API 传 `email`/`phone` 会被 Serde 静默丢弃，最终通知的收件人退化成继承人姓名，`POST /notifications/send` 返回 `failed=2`，错误“收件人不是有效邮箱地址”。
2. **通知列表数字查询被拒**。`GET /api/v1/notifications?limit=10` 的处理器使用 `Query<Value>`，`limit` 以字符串进入领域层 `ListInput`，返回 `400 INVALID_ARGS`。规划页加载通知时调用的正是这个端点。

0.0.26 已修复：

- `HeirInput`/`Heir` 增加可选 `email`、`phone`；新增邮箱格式校验，拒绝 `foo@`、`@bar.com`、`foo@bar`、多 `@` 和空白；电话保留可选文本。
- `set_heirs` 新增和更新 SQL 写入 `email`、`phone`；更新使用 `COALESCE`，不传字段时保留旧值，显式传空字符串时清空。
- HTTP 路由返回 `email`、`phone`；契约 schema、`contracts/mocks.json`、生成的 mock 同步更新。
- 通知列表处理器改为 `Query<notify::ListInput>`，数字 `limit` 正常解析。
- 规划页表单新增邮箱、电话输入；列表显示邮箱和电话；未填邮箱显示“未填邮箱，无法接收最终通知”。
- 设置页“安全与数据”在 1280 px 下不再横向溢出；审计表横向滚动区域加 `tabindex=0`，可用键盘聚焦滚动。
- 通知说明文案补上 SMTP 通道，不再只写 Webhook / 飞书 / 企业微信。

## 2. 真实触发链与通知投递

测试使用独立数据库和真实服务进程，通过 SQLite 时间旅行推进阶段，不等待自然 30/60/90 天：

- `check_in_interval_days=1`；
- 把 `last_check_in` 改到 2 天前，触发 stage 1；
- 把 `triggered_at` 改到 11 天前，推进到 stage 3；
- 再改到 26 天前，推进到 stage 4。

事件表依次出现 `stage1 → stage2 → stage3 → final`，stage 4 自动为继承人写入最终通知。

修复前（无联系方式）：

```text
POST /notifications/send
{"configured":true,"errors":[],"failed":2,"pending":2,"sent":0}
错误：收件人不是有效邮箱地址
```

修复后（两位继承人均有邮箱）：

```text
POST /notifications/send
{"configured":true,"errors":[],"failed":0,"pending":0,"sent":2}
```

本地假 SMTP 日志实际出现：

```text
To: heir1@example.com
To: heir2@example.com
Subject: =?UTF-8?B?RGlnaUxlZ2FjeSDmnIDnu4jkuqTku5jpgJrnn6U=?=
正文包含：当前登记的有效资产：2 项
```

公网 HTTPS Webhook：

- 直接 `POST https://httpbin.org/post`：HTTP 200，2.409 s。
- 应用 `/connections/test`：`ok:true`，`latencyMs:1733`，`message:"HTTP 200"`。
- 对 httpbin 发送最终通知：`configured:true, failed:0, sent:2, pending:0`。

## 3. 真实浏览器 UI

环境：Chromium 真实浏览器、Vue dev server、8787 API、独立 `/tmp/dg-ui.db`。

规划页（1280×1000）：

- 点击“添加继承人”，出现 4 个文本输入框：姓名、关系/角色、邮箱、电话。
- 填写 `林若岚 / 配偶 / ui-heir@example.com / 13800000001`，保存提示“继承人已保存”。
- 刷新后邮箱和电话仍在。
- 通知面板错误数 0；直接 `GET /notifications?limit=10` 从 400 变为 200。
- 点击“发送待发”，SMTP 正常时返回“已发送 1，失败 0，剩余 0”，假 SMTP 实际收到 `To: ui-heir@example.com`。

发送失败与重试：

- 停掉 SMTP 后发送，通知行显示“发送失败”和 `连接 SMTP 失败: Connection refused (os error 111)`；平台返回 `已发送 0，失败 1`。
- 重启 SMTP 后再点“发送待发”，状态回到“已发送”，返回 `已发送 1，失败 0，剩余 0`，邮件实际送达。
- 无 JS pageerror，无未处理异常。

移动端（390×844）：

- 进入继承人编辑态，5 个 input（4 个文本 + 1 个复选）全部可见。
- 文档 `scrollWidth = 390`，无横向溢出，无越界元素。
- 截图：`/tmp/dg-ui-mobile-edit.png`。

## 4. 全量门禁

`bash tools/verify.sh` 最终结果：

| 项目 | 结果 |
| --- | --- |
| 契约 | 44 commands、23 tools、60 schemas、21 errors；arch-check OK |
| Rust | 194 项通过，0 失败 |
| Web | typecheck 通过；79 项通过，0 失败 |
| 路由 axe | 15 条路由 WCAG AA，0 违规 |
| 真实浏览器 | 16/16 路由 PASS，a11y violation 0 |
| Web 预算 | entry 82.19 / 83 KB；总 gzip 130.63 / 131.1 KB |
| 退出码 | 0 |

Rust 的 28 个 server 测试中包含：

- `notifications_list_accepts_numeric_query_limit`
- `plan_write_routes_update_holder_and_heirs`
- `plan_write_routes_reject_invalid_heir_email`
- `diagnostics_export_route_matches_contract`

领域层新增 `heir_email_validation_and_clear`，覆盖非法邮箱和清空已保存邮箱。

浏览器证据：`docs/operations/measurements/live-browser-2026-11-21.json`；截图目录 `/tmp/dg-live-026`。

## 5. 发布包

- 包：`packages/digilegacy-0.0.26-linux-x86_64.tar.gz`
- 大小：18,451,545 B（约 18 MB）
- SHA256：`b75eb0b6ad9080d9cbff7bbb7ecd64f60855beed8d9f7290f3c07dddab9dc682`
- 校验文件：`packages/digilegacy-0.0.26-linux-x86_64.tar.gz.sha256`
- 包内 `VERSION` 为 `0.0.26`。

干净前缀安装演练：

```text
./install.sh --prefix /tmp/dg-026-install --bind 127.0.0.1:8792 --token package-check
DigiLegacy is ready at http://127.0.0.1:8792
GET /api/v1/health
version: 0.0.26, schemaVersion: 16, tables: 31, status: ok
GET /api/v1/notifications?limit=10
200 OK, items: [], total: 0
```

启动、健康检查、通知列表、停止均成功。

## 6. 验收边界

以下内容本轮仍未真实覆盖，不能算“全功能验收”已完成的部分：

- 阶段推进使用 SQLite 时间旅行，不是自然等待 30/60/90 天。
- SMTP 使用本地假 SMTP 服务，不是外部邮件服务商；HTTPS 使用 httpbin，不是真实飞书/企业微信机器人。真实飞书/企微 token 未接入。
- 外部真实政策提供方未接；政策同步只测本地签名 feed。
- 1B 模型、双模型热切换、超过 8K 上下文未测；按用户要求只保留 2B。
- 全 UI 每个表单的逐项交互仍未穷举；16 条浏览器路由覆盖渲染、溢出和 axe，不替代逐表单验收。
- 磁盘满、断电、并发写冲突等破坏性场景未全量演练，已有部分单测覆盖。

## 7. 结论

0.0.26 修复了 0.0.25 全站验收中暴露的两个真实缺陷，并完成对应回归、真实触发链、真实 SMTP/HTTPS 投递、真实浏览器表单与失败重试、全量门禁、干净安装和重新出包。

**已做完。** 0.0.25 包不再作为最终交付包，由 0.0.26 替代。外部账号类验收仍是明确边界，需要真实飞书/企微或邮件服务商凭据后才能继续。
