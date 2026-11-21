# QS 云模型补测与本地模型缺口审计（2026-11-21）

对象：`设置 → 连接` 中真实保存的 `QS`（`https://api.qs.al/v1`），模型 `glm-5.3-flash`。所有规划/执行走真实 HTTP API 和真实 Store；不是 mock。

## 结论

- 本地模型旧测试里，**多步规划**和**规划后的真实执行**没有覆盖；本轮用 QS 补齐，均 PASS。
- **长上下文不能由这条 QS 通道替代本地压测**：4,986 prompt tokens 可返回正确尾标记，但更长请求在约 30.6 s 被网关断连。本地 32K/128K 仍需真实内存与 sidecar 测试。
- 发现并修正一处 UI 口径：已保存的 LLM 连接存在时，启用本地模型并不会把规划器切过去；面板文案和 `LOCAL-MODELS.md` 已改为真实优先级。
- 远端规划延迟仍明显高于 2 s 首事件目标：3 步规划端到端 21.12 s，200 行/4,986 tokens 的长上下文请求 25.64 s。

## 本轮实测

| 场景 | 结果 | 证据 |
| --- | --- | --- |
| 简单规划 | PASS | HTTP 200；`glm-5.3-flash`；`assets.list`；3,468 in / 21 out |
| 三步只读规划 | PASS | HTTP 200，21.12 s；`overview.get` + `assets.list` + `agent.approvals.list`；3,497 in / 45 out |
| 规划后真实执行 | PASS | `POST /api/v1/agent/runs` 200，0.1 s；status `succeeded`；3 rounds；工具 `agent.approvals.list, assets.list, overview.get`；写步骤 0 |
| 长上下文 100 行 | PASS | 2,495 prompt tokens；20.52 s；返回 `LAST=0100` |
| 长上下文 200 行 | PASS | 4,986 prompt tokens；25.64 s；返回 `LAST=0200` |
| 长上下文 450/900/1800 行 | FAIL（网关） | 均在约 30.6 s 返回 `RemoteDisconnected`，无 HTTP 响应；说明该通道不能作为 32K/128K 压测替身 |

## 旧本地模型测试仍缺什么

来源：`docs/operations/measurements/local-models-functional-2026-11-20.md` 的“未覆盖或仍需注意”。

| 缺口 | 能否用 QS 补 | 说明 |
| --- | --- | --- |
| 32K/128K/1M 上下文 | 否 | QS 在约 30.6 s 断连长 prompt；必须用本地 sidecar + 足够内存实测 |
| 下载中断后续传 | 否 | 产品尚未实现断点续传，与模型无关 |
| 两个不同模型同时下载 | 否 | 本地下载管理器/磁盘问题，与模型无关 |
| 真实磁盘写满 | 否 | 需要独立盘或配额环境，不能用 `ulimit -f` 当真实写满 |
| 干净机无 Python 依赖启用模型 | 否 | 需要无 `llama-cpp-python` 的干净机演练 |
| 多步规划 + 真实工具执行 | 是，已补 | 本轮 3 步只读规划 + 3 rounds `succeeded` |
| 云端连接 vs 本地启用优先级 | 是，已查并修文案 | 保存的 LLM 连接优先；本地模型只有在该连接停用后才接管 |

## 未改动的边界

- 本轮没有启用 MiniCPM5-2B，没有占用本地 CPU；2B 文件仍在 `data/models/minicpm5-2b/`。
- 规划器任务 schema 上限为 4,000 字符，长上下文主要通过直接 chat/completions 探测；规划器本身不会发送 32K prompt。
- QS 价格按公开价表估算，`glm-5.3-flash` 仍是当前四个可发现模型里可查单价最低的。
