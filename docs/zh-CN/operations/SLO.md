# 服务等级目标

本文描述单节点、自托管的 DigiLegacy 部署。主机丢失、网络分区、DNS、上游邮件/Webhook 服务商以及用户浏览器故障不在运营方边界内。

这些数字是**暂定目标**。首轮容量与恢复演练已于 2026-11-20 在本地参考主机完成（见“实测基线”）。它们验证的是流程与当前构建，不代表每位运营者的硬件。正式对外承诺 p95 前，请在实际主机上重跑容量演练。

## 目标

| 领域 | 目标 | 窗口 | 测量方式 |
|---|---|---|---|
| API 可用性 | 健康探针成功率 **99.5%** | 滚动 30 天 | 主机外部每 60 秒请求 `GET /api/v1/health` |
| 读延迟 | p95 ≤ **250 ms**，p99 ≤ **800 ms** | 滚动 7 天 | 参考数据集上的 `overview`、`assets.list`、`policies.list` |
| 写延迟 | p95 ≤ **500 ms**，不含外部投递 | 滚动 7 天 | 资产创建/更新、审批决定、检查点创建 |
| 服务端错误率 | 5xx **< 1%** | 5 分钟窗口 | 反向代理或访问日志 |
| 审计链 | 每日检查 **100%** 有效 | 每日 | `invoke diagnostics.export --json` 的 `audit.valid` |
| 备份新鲜度 | RPO **24 小时**，RTO **2 小时** | 持续 | `security status --json` 的 `lastBackupAt`；恢复演练 |
| 最终通知 | **95%** 在 **5 分钟**内被接受 | 滚动 7 天 | `notifications.list` 与服务商日志 |
| 发布质量 | `./tools/verify.sh` **100%** 通过 | 每次发布 | CI 日志与本地复跑 |

### 可用性预算

30 天 = 720 小时。99.5% 可用性对应每月 **3 小时 36 分钟**错误预算。每 60 秒一次探针，30 天共 43,200 次；216 次失败即耗尽预算。

可用性必须由外部探针测量，不能用 `ps` 或进程守护状态代替。进程还活着但持续返回 500，仍然算不可用。

### 延迟参考数据集

暂定 p95 目标假设：

- 1,000 个资产、250 个继承人/联系人、10,000 条审计记录
- 一个运营者，最多两个并发 Agent 运行
- SQLCipher 位于本地 SSD
- 读请求由平台进程直接服务，不经过远程数据库

每次基准测试都要记录实际数据集规模。空库上的 p95 不能作为该目标的证据。

## 测量

`GET /api/v1/metrics` 是只读的进程内 JSON 快照，包含请求总数、状态码分类、并发中请求、延迟 p50/p95/p99/max 与延迟桶。它刻意不记录路径、资产名、继承人姓名、连接标签或凭据。进程重启后计数归零，因此告警必须结合外部健康探针。

```bash
# 可用性探针
curl -fsS http://127.0.0.1:8787/api/v1/health

# 进程启动以来的 HTTP 计数与延迟桶
curl -fsS -H "Authorization: Bearer $DIGILEGACY_API_TOKEN" \
  http://127.0.0.1:8787/api/v1/metrics

# 审计链状态与表行数（会写入一条审计记录）
cargo run -q -p digilegacy-cli -- diagnostics export --allow-write --json

# 备份新鲜度与恢复/密钥状态
cargo run -q -p digilegacy-cli -- security status --json
```

metrics 的 p95 是桶上界估算，只适合告警，不是精确分位数；精确值由下方演练脚本在客户端记录。

建议告警条件：

- 连续 3 次健康失败，或 15 分钟内超过 5 次失败
- 5 分钟内 `requests.status5xx / requests.total` 超过 1%
- p95 延迟连续 30 分钟超目标
- 诊断导出中 `audit.valid = false`
- `lastBackupAt` 超过 24 小时
- `uptimeSeconds` 意外归零（进程重启）

单次请求失败不要直接开事故；先重试一次。

## 容量演练

参考数据集可在一张临时库上重复生成：

```bash
export DIGILEGACY_DB=/tmp/digilegacy-capacity/live.db
cargo run -q -p digilegacy-server --example seed_reference
# 补齐到 1,000 资产、250 继承人、10,000 条审计记录。

DIGILEGACY_BIND=127.0.0.1:8799 DIGILEGACY_API_TOKEN=capacity-token \
  ./target/release/digilegacy-server &
python3 tools/bench/capacity.py \
  --base-url http://127.0.0.1:8799 --token capacity-token \
  --reads 1200 --writes 120 --concurrency 8 \
  --out docs/operations/measurements/capacity-2026-11-20.json --strict
```

脚本混合请求 health、overview、assets、plan、audit、security 与 metrics，再通过正常 invoke 接口创建临时资产。结果记录数据规模、客户端 p50/p95/p99、错误数与服务端计数。`--strict` 在超目标或出现错误时返回非零。

## 实测基线

2026-11-20，release 构建 `0.0.15`，Linux x86-64 本地主机，回环 HTTP，写阶段前 1,000 资产 / 250 继承人 / 10,000 条审计：

| 指标 | 结果 | 目标 |
|---|---|---|
| 读，1,200 次，并发 8 | p50 6.38 ms，p95 **9.54 ms**，p99 **11.09 ms**，0 错误 | p95 ≤ 250 ms，p99 ≤ 800 ms |
| 写，120 次资产创建，并发 4 | p50 1.98 ms，p95 **3.70 ms**，p99 9.38 ms，0 错误 | p95 ≤ 500 ms |
| 恢复到健康 | 复制 3.5 MB 检查点后 **158 ms**；health、1,120 资产、10,120 审计可读 | RTO ≤ 2 小时 |
| 恢复后审计链 | `valid: true`，`checked: 10,120` | 100% 有效 |

原始证据：`docs/operations/measurements/capacity-2026-11-20.json` 与 `restore-2026-11-20.json`。这些是本地合成数据，不覆盖服务商投递、磁盘故障或真实用户负载。

## 错误预算策略

- 消耗 **50%**：停止非必要功能开发，复盘失败信号。
- 消耗 **80%**：只做修复与运维工作，直到预算恢复。
- 消耗 **100%**：冻结发布；下一次发布必须包含纠正措施与验证记录。

对单节点自托管产品，坏发布通常就是可用性事件，即使根因是迁移错误或密钥缺失，也应计入预算。

## 当前缺口

| 缺口 | 影响 | 下一步 |
|---|---|---|
| metrics 是 JSON，不是 Prometheus 文本 | Prometheus 需要小适配器 | 增加文本导出或抓取 JSON 快照 |
| 容量演练只有一台合成主机 | 换磁盘/CPU 可能达不到目标 | 上线前在目标主机重跑 `tools/bench/capacity.py` |
| 恢复演练是手工流程 | RTO 只测了 3.5 MB 数据库 | 每季度按生产库规模复跑 |
| 没有服务商级投递 SLO | SMTP/Webhook 延迟取决于服务商 | 用真实账号分别测量“已接受”与“最终送达” |

负责人：值班运营者。每次事故后以及至少每季度复核一次目标。
