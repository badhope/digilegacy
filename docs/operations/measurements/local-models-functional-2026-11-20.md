# 本地模型功能测试记录（0.0.20）

测试日期：2026-11-20  
环境：Linux，2 核 CPU，3.9 GiB 内存，`llama-cpp-python 0.3.37`  
测试对象：当前 `digilegacy` 工作区的本地模型目录、下载、SHA256 校验、启用/切换/停止、规划器接入和 Web 面板。

## 结论

主链路和异常分支均已实测，发现的 5 个问题已修复并复验。

- Rust workspace：185 passed / 0 failed。
- Web：79 passed / 0 failed；typecheck、build、web-budget 通过。
- 真实 Chromium：14/14 路由通过；设置页桌面与 390px 移动端 axe 均为 0 违规。
- 三个模型均完成真实下载、SHA256 校验、sidecar 启动和一次真实规划。

## 功能矩阵

| 场景 | 结果 | 证据 |
| --- | --- | --- |
| 目录、推荐标记、严格 <2B 标记 | PASS | 3 个模型字段与大小正确 |
| 下载/启用/停止鉴权 | PASS | 无 token 均返回 401 `AUTH_REQUIRED` |
| 未知模型下载 | PASS | 404 `NOT_FOUND` |
| 未知模型启用 | PASS | 404 `NOT_FOUND` |
| 未下载模型启用 | PASS | 409 `CONFLICT` |
| MiniCPM5-1B 下载与校验 | PASS | 688,065,920 B，SHA256 与官方一致 |
| MiniCPM5-2B 下载与校验 | PASS | 1,561,318,368 B，SHA256 与官方一致 |
| Spark-X2.5-1.7B 下载与校验 | PASS | 1,107,457,856 B，SHA256 与官方一致 |
| 重复下载同一模型 | PASS | 返回 `downloading`，不重复启动下载 |
| 已下载模型再次下载 | PASS | 返回 `ready` |
| 下载失败状态 | PASS | 无 `curl` 时返回 `failed` 和明确错误，不崩溃 |
| 启用 1B / 2B / Spark | PASS | 三者均启动 sidecar 并通过 `/health` |
| 重复启用同一模型 | PASS | 0.002 s 返回，PID 不变 |
| 切换模型 | PASS | 旧 sidecar 退出，新模型接管同一端口 |
| 停止模型 | PASS | active.json 删除，端口释放，再停幂等 |
| 服务重启恢复 | PASS | 重启后从端口探测恢复已启用状态 |
| 重启后停止 | PASS | 依据 active.json 中的 PID 正常终止 sidecar |
| PID 安全防护 | PASS | active.json 写入无关进程 PID，stop 未误杀 |
| sidecar 提前退出 | PASS | 不再等 45 秒，返回 `PROVIDER_ERROR` 并带 exit code |
| 伪造校验标记 | PASS | 同尺寸稀疏文件不再被当作有效模型；启用前重新计算 SHA256 |
| 规划器 1B | PASS | `assets.list`，3596 in / 19 out，55 s |
| 规划器 2B | PASS | `assets.list`，3596 in / 20 out，116 s |
| 规划器 Spark | PASS | `assets.list`，4162 in / 28 out，83 s |
| Web 无 token | PASS | 启用/下载按钮禁用并提示填写令牌 |
| Web 保存令牌 | PASS | 保存后按钮即时解除禁用 |
| Web 下载 Spark | PASS | 进度轮询到 100%，最终显示已下载 |
| Web 启用 1B、切换 2B、启用 Spark、停止 | PASS | 状态、按钮和提示同步更新 |
| Web 错误令牌 | PASS | 返回 401 后显示“填写平台写令牌”，不显示 500/502 |
| Web 桌面与移动端 | PASS | 3 张卡片正常，390px 无横向溢出 |
| 全路由真实浏览器 | PASS | 14/14；无横向溢出；无页面异常；无 API 5xx |
| axe 可访问性 | PASS | 设置页桌面 0、移动端 0 |

## 本轮发现并修复

1. `activate` 的所有失败都映射成 502 `PROVIDER_ERROR`，未知模型和未下载模型语义错误。现按 `NOT_FOUND` / `CONFLICT` / `CORRUPT_DATA` / `PROVIDER_ERROR` / `TIMEOUT` 返回。
2. `is_verified` 只检查标记文件，不检查模型大小；伪造标记可让无效文件显示“已校验”。现增加大小校验，并在启用前重新计算 SHA256，失败时删除标记、提示重新下载。
3. sidecar 启动即崩溃时仍等待 45 秒。现每 500 ms 检查子进程退出，立即返回错误和日志路径。
4. 下载完成后提示仍停留在“下载中”。现轮询结束即清除提示。
5. 面板挂载后保存写令牌，按钮不会即时解锁。现通过 `digilegacy:write-token` 事件同步。

## 未覆盖或仍需注意

- 未做 128K/1M 长上下文压力测试；sidecar 目前固定 `--ctx 8192`。
- 下载中断后重试会从头开始，没有断点续传。
- 未压测两个不同模型同时下载、磁盘写满、目录只读等资源异常。
- 发布包尚未做“干净机无 Python 依赖”的模型启用演练；当前依赖系统 Python 与 `llama-cpp-python`。
- 状态页的“SHA256 已校验”表示下载时校验过；同尺寸的后期篡改只会在启用前被重新计算拦截。
