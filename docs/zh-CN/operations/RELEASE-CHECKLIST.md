# 发布清单

版本：**0.0.19**（Linux x86_64，2026-11-20）

状态：**工程证据齐全，尚未人工签字。**

归档校验值在 `packages/digilegacy-0.0.19-linux-x86_64.tar.gz.sha256`，归档内也有
`SHA256SUMS`。不要从聊天消息里抄校验值，必须以归档同目录文件为准。

## 自动证据

| 门禁 | 结果 | 证据 |
| --- | --- | --- |
| 契约 / 架构 / 格式 / clippy | 通过 | `./tools/verify.sh` exit 0 |
| Rust + Web 测试 | **184 Rust + 79 Web** | `./tools/verify.sh` |
| 契约 | **44 命令 / 23 工具 / 60 schema / 21 错误码** | `./tools/generate.mjs --check` |
| Schema | **v16 / 31 表** | health + diagnostics |
| 体积预算 | 入口 80.75/82 KB；JS 104.95/105.2 KB；CSS 20.22/21.1 KB；总 125.17/125.5 KB gzip | `web-budget` |
| 真实 Chromium + axe | 14/14 路由通过，axe 0 违规 | `measurements/live-browser-0.0.19-2026-11-20.json` |
| 视觉基线 | 9/9 在 0.4% 内；最大 overview-mobile 0.173% | `tools/visual/regress.py` |
| 恢复码 + 失败限速 | 锁定 423；错误 400；解锁 200；再锁定；恢复码 200；资产 200；5 次失败以 429 收尾；限速中正确口令仍 429 | `measurements/security-recovery-0.0.19-2026-11-20.json` |
| 保险库锁 API + CLI | 423 → 400 → 200 → 200 → 423；CLI 无口令环境变量被拒 | `measurements/security-lock-0.0.18-2026-11-20.json`（0.0.18 基线） |
| 干净机安装 | 12 步 PASS，重装可读保留库 | `measurements/install-0.0.19-2026-11-20.json` |
| 归档 | `packages/digilegacy-0.0.19-linux-x86_64.tar.gz`，18,293,344 bytes，SHA256 `53f86e570d59571dccfd27c0df789af665978551f4d413d61c14296cb805e8ee` | `...tar.gz.sha256` |

## 继承自 0.0.18 的运行基线

这些演练测于 0.0.18 代码线，本补丁未重跑。恢复码与限速改动不触及出站代理、容量、
恢复、DNS 钉住与政策来源路径；把它们当作基线证据，不要当成新的 0.0.19 测量。

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
