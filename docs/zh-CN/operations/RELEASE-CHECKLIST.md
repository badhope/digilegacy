# 发布清单

版本：**0.0.17**（Linux x86_64，2026-11-20）

状态：**工程证据齐全，尚未人工签字。**

归档校验值在 `packages/digilegacy-0.0.17-linux-x86_64.tar.gz.sha256`，归档内也有
`SHA256SUMS`。不要从聊天消息里抄校验值，必须以归档同目录文件为准。

## 自动证据

| 门禁 | 结果 | 证据 |
| --- | --- | --- |
| 契约 / 架构 / 格式 / clippy | 通过 | `./tools/verify.sh` |
| Rust + Web 测试 | **175 Rust + 68 Web** | `./tools/verify.sh` exit 0 |
| 契约 | **39 命令 / 23 工具 / 56 schema / 21 错误码** | `./tools/generate.mjs --check` |
| Schema | **v16 / 31 表** | health + diagnostics |
| 体积预算 | JS 103.00/103.1 KB、CSS 21.05/21.1 KB、总 124.05/124.2 KB gzip | `web-budget` |
| 真实 Chromium + axe | 15/15 路由通过，axe 0 违规 | `measurements/live-browser-2026-11-20.json` |
| 视觉基线 | 9/9 在 0.4% 内 | `tools/visual/regress.py` |
| 容量 | 读 p95 9.54 ms、写 p95 3.70 ms | `measurements/capacity-2026-11-20.json` |
| 恢复 | 158 ms 健康，10,120 条审计有效 | `measurements/restore-2026-11-20.json` |
| 干净机安装 | 12 步 PASS，重装可读保留库 | `measurements/install-2026-11-20.json` |
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
