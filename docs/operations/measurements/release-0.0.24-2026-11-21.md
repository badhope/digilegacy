# DigiLegacy 0.0.24 发布验收报告

日期：2026-11-21  
版本：0.0.24  
代码提交：`7d0719c`  
分支：`main`

## 结论

通过。0.0.24 在 0.0.23 的模型定位修正上，补掉了真实测试暴露的两个问题：损坏的 `active.json` 不再残留；已校验模型的进度固定为 100%。本地模型链路已用真实 API、真实 1.5 GB ModelScope 下载、真实 sidecar 和真实 Chromium 复测。

## 修复

- 启动时 `active.json` 解析失败会直接删除，不再静默保留损坏文件。
- 状态为 `ready` 的模型 `progress` 固定返回 `1.0`。
- 真实浏览器巡检新增设置根路径桌面与 390px 移动端，覆盖本地模型面板本身。

## 本地模型全链路结果

| 场景 | 结果 | 证据 |
| --- | --- | --- |
| 0.0.24 服务 | PASS | `/health` 返回 0.0.24 |
| 损坏 `active.json` | PASS | 写入 `{corrupt` 后重启，文件被删除；runtime `false`；单测通过 |
| 2B 真实下载 | PASS | ModelScope 1.5 GB；curl 约 55 s；SHA256 落盘校验通过 |
| 下载完成状态 | PASS | `ready`、`progress=1.0`、`verified=true`、大小 1,561,318,368 B |
| 并发启用 | PASS | 两个 activate 均 HTTP 200，33.85 s，同一 PID 33598、同一端口 11435 |
| 重复启用 | PASS | 已启用时再 activate 返回 200，0.012 s，PID 不变 |
| 停止幂等 | PASS | 连续两次 stop 均 200 `stopped=true`，无残留 sidecar |
| 重启恢复 | PASS | 重启 API 后 runtime `true`，沿用 PID 33598 |
| 鉴权与错误路径 | PASS | 无 token 401；未知模型 404；未下载启用 409；路径穿越 404 |
| Web UI 停止/启用 | PASS | 真实按钮触发，Enabled→Not enabled→Enabled，sidecar 同步退出/启动 |
| 2B 真实规划 | PASS | HTTP 200，103.81 s；3598 in / 12 out；返回 `overview.get` |
| 2B 纯聊天 | PASS | HTTP 200，0.826 s；23 in / 5 out；返回“本地模型测试成功。” |

## 门禁与浏览器

| 项目 | 结果 |
| --- | --- |
| Rust workspace | 190 passed / 0 failed |
| Web | 79 passed / 0 failed（12 个测试文件） |
| `./tools/verify.sh` | exit 0；契约、架构、Rust、Web、web-budget 全部 OK |
| Web 包体 | JS 109.47 / 110.1 KB gzip；CSS 20.88 / 21.1 KB；总 130.35 / 131.1 KB |
| 真实 Chromium | 16/16 PASS；0 failures；设置根桌面与 390px 均无横向溢出；axe 0 违规 |
| 0.0.24 包解包启动 | 0.0.24；1B/2B 均 `missing`；无模型文件、无 `active.json`、无旧字段 |

## 测试边界

- 1B 未做真实下载：按用户要求本次只安装和使用 2B，1B 仅保留目录候选。
- 双模型热切换未在本轮重测：本机只安装 2B；两模型切换逻辑在 0.0.21 已实测，0.0.24 未改该路径。
- 超过 8K 上下文未在本轮重测：2B 以默认 8K 跑通真实规划；16K 历史结果见 0.0.21 记录。
- 真实 SMTP/Webhook、全站每条业务 CRUD 不属于本次本地模型改动，未在本轮用真实账号重跑。

## 发布物

- 包：`packages/digilegacy-0.0.24-linux-x86_64.tar.gz`，18 MB
- SHA256：`122aea9d19e69754bd54318c39ef90ce7b8f0ea32cf24f8f5d469c899689233c`
