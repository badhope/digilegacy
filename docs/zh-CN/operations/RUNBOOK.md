# 运行手册

本手册假设数据库、密钥文件与 CLI 在同一台单节点主机上。命令使用 `digilegacy-cli`；从源码运行时等价于 `cargo run -q -p digilegacy-cli --`。

## 环境变量

把变量放进 root 所有、权限 `0600` 的环境文件。不要把密钥写进仓库、shell history 或 unit 文件。

| 变量 | 必需 | 含义 |
|---|---|---|
| `DIGILEGACY_DB` | 建议 | 数据库绝对路径，例如 `/var/lib/digilegacy/digilegacy.db`；默认相对工作目录的 `data/digilegacy.db`。 |
| `DIGILEGACY_DB_KEY_FILE` | 加密部署必需 | 32 字节 SQLCipher 密钥文件。丢失后数据库无法解密；必须与数据库分开备份。 |
| `DIGILEGACY_CONFIG_KEY_FILE` | 可选 | 连接配置的独立密钥。缺省时从数据库密钥派生域密钥。 |
| `DIGILEGACY_BIND` | 是 | API 绑定地址，默认 `127.0.0.1:8787`；远程访问应经反向代理。 |
| `DIGILEGACY_API_TOKEN` | 写入需要 | 写路由的 Bearer token。未配置时写路由保持关闭。 |
| `DIGILEGACY_ALLOW_PRIVATE_NETWORK` | 仅 LAN 集成 | `1`、`true` 或 `yes` 放行私网字面地址；云元数据始终拒绝。 |
| `DIGILEGACY_POLICY_URL` / `TOKEN` | 可选 | 远端政策 feed 回退；优先使用已保存的 `policy` 连接。 |
| `DIGILEGACY_SMTP_*`、`DIGILEGACY_WEBHOOK_*` | 可选 | 通知回退；优先使用已保存连接。 |

启动 API：

```bash
set -a
. /etc/digilegacy.env
set +a
exec /opt/digilegacy/bin/digilegacy-server
```

健康检查：

```bash
curl -fsS http://127.0.0.1:8787/api/v1/health
```

期望包含 `success: true`、`schemaVersion: 16`、`tables: 31`。健康响应只证明进程能打开存储，不证明写 token 或外部集成可用。

## 日常检查

```bash
# 进程与 API
curl -fsS http://127.0.0.1:8787/api/v1/health

# 备份新鲜度与恢复状态
digilegacy-cli security status --json

# schema、完整性与审计链
digilegacy-cli invoke diagnostics.export --json
```

通过条件：

- 健康检查成功
- `audit.valid` 为 `true`
- `lastBackupAt` 不超过 24 小时
- 代理日志无异常 5xx 峰值

## 检查点与备份

检查点是数据库的认证副本加清单。清单保存 SHA-256；数据库密钥可用时再加 HMAC。检查点放在数据库旁的 `checkpoints/`，最多保留 12 个。

```bash
# 升级、迁移或高风险修复前创建
digilegacy-cli security checkpoint --label before-upgrade --json

# 列出检查点及绝对路径
digilegacy-cli security checkpoints --json

# 只读校验大小、哈希与 HMAC
digilegacy-cli security verify <checkpoint-id> --json
```

命令会先做 WAL checkpoint 再复制，因此 API 运行中也可执行。它会写入 `backup.create` 审计，并更新 `security status` 的 `lastBackupAt`。

检查点命令是**本地 CLI 命令**，不对 HTTP 或 MCP 暴露，因为它需要数据库密钥并在存储旁写文件。

每日调度示例（按实际路径与用户调整）：

```cron
0 3 * * * cd /opt/digilegacy && set -a && . /etc/digilegacy.env && set +a && /opt/digilegacy/bin/digilegacy-cli security checkpoint --label scheduled >> /var/log/digilegacy/checkpoint.log 2>&1
```

密钥文件必须单独备份。没有匹配密钥的检查点在密码学上不可用。

## 恢复

恢复刻意保持人工、离线。产品不提供 `security restore` 命令：覆盖在线数据库是最高风险操作。

1. 宣布维护窗口，停止 API，确认端口已关闭。
2. 数据库仍可读时，先创建最后一个检查点。
3. 加载服务使用的同一环境文件。
4. 列出并校验选中的检查点：
   ```bash
   digilegacy-cli security checkpoints --json
   digilegacy-cli security verify <checkpoint-id> --json
   ```
5. 把校验过的 `.db` 复制到新目标，例如 `/var/lib/digilegacy/restore/digilegacy.db`；此时不要覆盖在线文件。
6. 用备用端口启动临时 API，`DIGILEGACY_DB` 指向恢复副本，密钥文件保持一致：
   ```bash
   DIGILEGACY_BIND=127.0.0.1:8788 DIGILEGACY_DB=/var/lib/digilegacy/restore/digilegacy.db \
     /opt/digilegacy/bin/digilegacy-server
   curl -fsS http://127.0.0.1:8788/api/v1/health
   ```
7. 做读 smoke：打开概览、读取一个资产、核对审计数量。
8. 停止临时 API。把旧库及 `-wal`、`-shm` 移到事故目录，再把恢复库移到在线路径。
9. 启动正式服务，再跑健康与读 smoke。
10. 创建 `after-restore` 检查点，记录事故编号、恢复耗时与所用检查点。

第 6 或第 9 步失败时立即停止并放回旧库。不要让两个进程同时操作同一个数据库文件。

## 常见故障

| 现象 | 检查 | 处理 |
|---|---|---|
| 写操作 `401` / `403` | `DIGILEGACY_API_TOKEN` 与 Web 写令牌 | 服务端与设置页使用同一 token；不要关闭门禁。 |
| `unable to open database file` | 路径、属主、磁盘空间、密钥文件 | 修权限或从已校验检查点恢复；不要为了“能打开”删除密钥。 |
| `file is not a database` | 密钥不匹配或副本损坏 | 用匹配密钥；恢复前先 `security verify`。 |
| 政策同步失败 | URL allowlist、HMAC 密钥、`If-None-Match`、远端状态 | 用 `curl` 检查 feed；候选批准前不要改政策库。 |
| SMTP 或 Webhook 测试失败 | DNS、TLS、token、服务商状态 | 用连接测试行；不要绕过证书校验。 |
| 审批队列增长 | worker、`jobs.get`、`approvals.list` | 检查点后重启服务；不要批量批准来清队列。 |
| 审计链无效 | `diagnostics.export` → `audit.valid=false` | 停止写入、保留数据库、开事故、恢复最后验证过的检查点。 |
| 健康正常但读失败 | 密钥/配置不匹配、存储锁 | 查日志与诊断包；健康不等于可读。 |

## 演练

以下演练只在临时库执行，禁止对线上库操作：

- **容量**：用 `cargo run -q -p digilegacy-server --example seed_reference`
  灌入 1,000 资产、250 继承人、10,000 条审计，再以 `--strict` 跑
  `tools/bench/capacity.py`。最新结果在 `docs/operations/measurements/`。
- **恢复**：创建检查点、复制 `.db`、启动临时实例、探测 health 与行数、校验
  审计链。2026-11-20 演练达到健康耗时 158 ms，校验 10,120 条审计。
- **通知**：使用真实 SMTP 或 Webhook 账号，分别记录“已接受”与“最终送达”。
  本地 TLS 测试只能证明握手。

记录数据规模、构建版本、主机、结果文件与耗时。缺这些字段就不算证据。

## 升级与回滚

1. 创建并校验 `before-upgrade` 检查点。
2. 对新构建运行 `./tools/verify.sh`。
3. 停止 API。
4. 保留旧二进制与数据库，直到新实例通过健康与读 smoke。
5. 只有检查点校验通过后，才允许对在线副本跑迁移。
6. schema 或读 smoke 失败时，停止、恢复升级前检查点、启动旧二进制；记录失败版本与 schema。

没有已验证检查点时，禁止跑迁移。

## 事故交接

记录：

- 开始时间、发现来源、受影响入口
- 最后一次成功健康检查
- 所用检查点 ID 与 `security verify` 结果
- 恢复开始/结束时间与 RTO
- 数据丢失窗口（最后检查点到事故之间）
- 后续动作与负责人

不要把 token、密钥字节、个人资产值或连接密码写进事故工单。
