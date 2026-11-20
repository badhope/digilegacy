# 本地模型功能测试记录（0.0.20 / 0.0.21）

测试日期：2026-11-20 至 2026-11-21  
环境：Linux，2 核 CPU，3.9 GiB 内存，`llama-cpp-python 0.3.37`  
测试对象：当前 `digilegacy` 工作区的本地模型目录、下载、SHA256 校验、启用/切换/停止、规划器接入和 Web 面板。

## 结论

第一轮主链路和异常分支已实测，发现的 5 个问题已修复并复验；第二轮补做并发、资源边界、恢复性和 i18n，又修了 6 个问题。

- Rust workspace：187 passed / 0 failed。
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

## 第二轮：并发、异常与资源边界（2026-11-21）

使用独立测试服务 8791 和假 sidecar，避免每次都加载真实模型；路径穿越、只读目录、写配额另开 8797/8798。

| 场景 | 结果 | 证据 |
| --- | --- | --- |
| 同时启用 1B 和 2B | PASS | 1B 先完成，2B 后完成时终止旧 sidecar，最终只剩 2B，无进程泄漏 |
| 同时启用同一个 1B（修复前） | FAIL | 一个请求 200，另一个 502 `PROVIDER_ERROR` |
| 同时启用同一个 1B（互斥锁修复后） | PASS | 两个请求均 200，约 13.55 秒返回，active 均为 PID 1615，只有一个 sidecar |
| 损坏 `active.json` | PASS | 写入非法 JSON 后状态接口仍 200，`active=null`、`runtime.running=false`，服务不崩溃 |
| 幽灵 PID | PASS | `pid=999999` 时状态显示记录但 `running=false`；stop 返回 `{"stopped":true}` 并删除文件 |
| 启动清理幽灵 active（0.0.21） | PASS | 重启后自动删除死 PID + 死端口记录；sidecar 仍存活时保留记录并恢复 `active=true` |
| 端口耗尽 | PASS | 占满 11435–11439 后启用 1B，12.9 秒返回 500 `INTERNAL`，提示“没有可用的本地推理端口（11435..11440）” |
| 只读模型目录下载 | PASS | Spark 下载先 `downloading`，随后 `failed`：“创建模型目录失败: Permission denied (os error 13)” |
| 只读模型目录启用 | PASS | 1B 启用 12.99 秒后返回 500 `IO`：“打开运行时日志失败: Permission denied (os error 13)”，无残留 sidecar |
| 文件大小配额 | PASS | `ulimit -f 2048` 下载 Spark 后 `failed`，错误“下载源返回失败（curl exit -1）”，留下 2.0 MB `.part` |
| HTTP 方法限制 | PASS | `GET /api/v1/local-models/minicpm5-1b/download` 返回 405 |
| 路径穿越 | PASS | `%2e%2e%2f` 解码后 404 `NOT_FOUND`“未知本地模型: ../../etc”；`%00` 返回 404，未越出模型目录 |
| 英文界面 | PASS | 按钮、状态、徽标、参数、描述和运行时提示均为英文；中文界面回归正常 |
| 中英文切换 | PASS | 同一页面切换 locale 后文案完整替换，无混排 |
| 8K 上下文边界 | PASS | 5,028 prompt tokens 成功（79 秒）；9,028 tokens 返回 500，sidecar 日志 `exceed context window of 8192` |
| 16K 上下文配置 | PASS | `DIGILEGACY_LOCAL_MODEL_CTX=16384` 后同一份 9,028-token prompt 成功，210.8 秒 |
| API 不可达 | PASS | 面板显示“本地模型服务暂不可用（HTTP 502）”，不再暴露 `Unexpected end of JSON input` |
| 规划器回归 | PASS | 1B 在 16K 运行时下返回 `assets.list`，`tokensIn=3598`、`tokensOut=19`，51.27 秒 |

### 第二轮发现并修复

6. 同一模型并发启用时，`activate` 的“读 active、哈希、停旧、找端口、启动、写 active”整段没有互斥，两个调用互相踩，后到的请求返回 502。现在 `Manager` 增加 `activation: Mutex<()>`，`activate` 和 `stop` 都持有该锁，并发请求串行化。
7. 规划器默认 180 秒超时被 `clamp(5, 120)` 悄悄压成 120 秒，2B 实测 116 秒已接近上限。现在本地默认 180 秒、上限 300 秒，并新增 `local_model_timeout_keeps_three_minutes` 单测。
8. 服务重启后若 `active.json` 指向已死 sidecar，旧逻辑只在状态接口显示未运行，文件仍保留，规划器会继续请求死端口。现在启动时检查 PID 和端口，两者都失效才删除；sidecar 仍监听则保留。
9. sidecar 运行时上下文固定 8K，而界面展示模型原生 128K/1M。现在增加 `DIGILEGACY_LOCAL_MODEL_CTX`（2,048–131,072，默认 8,192），并在界面明确写“默认 8K 运行时上下文”。
10. 英文界面仍混有中文“推荐”徽标、中文参数、中文描述和“上下文”。现在前端按 `id` 提供完整英文模型文案，徽标和上下文标签走 i18n；中文界面不变。
11. API 不可达时前端直接显示 JSON 解析异常。现在 `invoke` 和本地模型请求都捕获网络/解析失败，返回 `NETWORK` 信封，面板显示“暂不可用（HTTP n）”。

## 未覆盖或仍需注意

- 未压测 32K/128K/1M 上下文；已验证 16K 下 9,028 tokens，但更高上下文需要更大内存和更长首 token 时间。
- 下载中断后重试会从头开始，没有断点续传。
- 未压测两个不同模型同时下载；磁盘写满只用 `ulimit -f` 模拟，没有真实写满整块盘；只读目录已实测。
- 发布包尚未做“干净机无 Python 依赖”的模型启用演练；当前依赖系统 Python 与 `llama-cpp-python`。
- 状态页的“SHA256 已校验”表示下载时校验过；同尺寸的后期篡改只会在启用前被重新计算拦截。
