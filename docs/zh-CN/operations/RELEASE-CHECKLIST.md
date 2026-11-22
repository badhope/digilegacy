# 发布清单

版本：**0.0.34**（Linux x86_64，2026-11-22）

状态：**工程证据齐全，尚未人工签字。**

归档校验值在 `packages/digilegacy-0.0.34-linux-x86_64.tar.gz.sha256`，归档内也有
`SHA256SUMS`。不要从聊天消息里抄校验值，必须以归档同目录文件为准。

## 自动证据

| 门禁 | 结果 | 证据 |
| --- | --- | --- |
| 契约 / 架构 / 格式 / clippy | 通过 | `./tools/verify.sh` exit 0 |
| Rust + Web 测试 | **211 Rust + 99 Web** | `./tools/verify.sh` |
| 契约 | **49 命令 / 23 工具 / 67 schema / 21 错误码** | `./tools/generate.mjs --check` |
| Schema | **v19 / 32 表** | health + diagnostics |
| 体积预算 | 入口 87.36/88 KB；vendor 101.41/130 KB；单路由最大 22.48/24 KB；JS 120.97/121 KB；CSS 22.12/23 KB；总 143.09/144 KB gzip | `web-budget` |
| 真实 Chromium + axe | 18/18 路由通过，axe 0 违规，0 失败 | `measurements/live-browser-0.0.34-2026-11-22.json` |
| 资产深度录入 | 6 条资产、类型筛选、深字段表单、保存并继续、草稿恢复、归档删除；列表 6→5、GET 404、审计有记录 | `measurements/ux-asset-depth-0.0.34-2026-11-22.json` |
| 干净机安装 | 12/12 PASS；重装可读保留库 | `measurements/install-0.0.34-2026-11-22.json` |
| 归档 | `packages/digilegacy-0.0.34-linux-x86_64.tar.gz`；SHA256 见同目录 `.sha256`，归档内也有 `SHA256SUMS` | `...tar.gz.sha256` |

## 继承的运行基线

这些演练测于 2026-11-20 代码线，0.0.34 未重跑。资产 schema 与界面改动不触及出站
代理、容量、恢复与 DNS 钉住路径；把它们当作基线证据，不要当成新的测量。

| 门禁 | 基线 | 证据 |
| --- | --- | --- |
| 容量 | 读 p95 9.54 ms、写 p95 3.70 ms | `measurements/capacity-2026-11-20.json` |
| 恢复 | 158 ms 健康，10,120 条审计有效 | `measurements/restore-2026-11-20.json` |
| DNS 重绑定 | 3/3 PASS，仅 1 次 A 查询，切换后解析被拒绝 | `measurements/dns-rebind-2026-11-20.json` |
| nginx 同源 | `/`、SPA fallback、`/api`、资源均 200；头正确 | `measurements/proxy-2026-11-20.json` |
| 政策来源预审 | 152/152 待人工复核；150 首页、2 深链 | `measurements/policy-sources-2026-11-20.json`、`policy-source-worklist.csv` |

## 仍需人工完成

- [ ] 真实 SMTP/Webhook 送达时延：账号本人在 `/settings/connections` 填写并发真实测试；本地协议测试不能替代服务商送达。
- [ ] 真人读屏与亮暗人工观感：自动 axe 与截图不能替代真人。
- [ ] 政策来源核对：152 条仍是 `unverified`；清单只是入口，不是核实结论。
- [ ] 发布负责人签字。

## 签字

| 角色 | 姓名 | 结论 | 日期 |
| --- | --- | --- | --- |
| 发布负责人 |  |  |  |
| 安全/数据评审 |  |  |  |
| 内容负责人（政策） |  |  |  |

只有上述人工项完成，或明确接受为发布限制后，才能签字。在此之前这只是候选版本，
不是已签字发布。
