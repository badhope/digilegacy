# 本地模型

DigiLegacy 0.0.24 在“设置 → 连接”内提供本地模型目录、下载和启用。这些模型是能力测试候选，不内置、不默认启用；用户自己选择下载、启用和切换，程序不会替用户指定默认模型。发布包只带适配层，不携带任何模型文件。

## 测试模型

| 模型 | 参数 | 上下文 | Q4_K_M 大小 | 许可 | 运行时 | 说明 |
| --- | --- | --- | --- | --- | --- | --- |
| MiniCPM5-1B | 1.08B 稠密 | 131,072 | 688,065,920 B | Apache-2.0 | 标准 llama.cpp | 轻量测试候选；标准 `LlamaForCausalLM`。本次测试环境未预装，用户可按需下载 |
| MiniCPM5-2B | 2.52B 稠密 | 131,072 | 1,561,318,368 B | Apache-2.0 | 标准 llama.cpp | 较大测试候选；本次测试环境预装并启用 |

Spark-X2.5-1.7B 已在 0.0.22 从目录和 Web 面板移除：它虽然能加载并返回合法计划，但在 2 核 / 3.9 GiB 目标环境上无法利用 1M 原生上下文，速度也慢于 1B，保留只会增加实验运行时和 1.1 GB 磁盘。

MiniCPM5-1B 的下载源为 ModelScope：

`https://modelscope.cn/models/OpenBMB/MiniCPM5-1B-GGUF/resolve/master/MiniCPM5-1B-Q4_K_M.gguf`

SHA256：

`81b64d05a23b17b34c475f42b3e72fbde62d4b92cc34541f7a8031d0752deafa`

## 下载

- 默认目录：`data/models/<model-id>/`，可用 `DIGILEGACY_MODELS_DIR` 覆盖。
- 下载由服务端发起，使用 `curl -L --fail --retry 3` 写入 `.gguf.part`，前端轮询进度。
- 完成后流式计算 SHA256，只有与目录中的官方值一致才改名为最终 `.gguf`，并写入 `.gguf.sha256` 标记。
- 下载和启用都需要平台写令牌（`DIGILEGACY_API_TOKEN`）。
- 启动时若 `active.json` 损坏或指向已移除的模型，会清理该文件；若 sidecar 仍存活则保留运行时。

## 启用

“启用”会启动 `tools/local_model_server.py`，它是一个最小的 OpenAI 兼容 sidecar：

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions`

sidecar 用 GGUF 自带 chat template 渲染，默认 `enable_thinking=false`。规划器请求会触发 JSON 语法约束，避免 1B 模型少写右括号导致计划解析失败。

启用成功后写入 `data/models/active.json`：

```json
{
  "id": "minicpm5-1b",
  "model": "MiniCPM5-1B-Q4_K_M",
  "baseUrl": "http://127.0.0.1:11436/v1",
  "apiKey": "local",
  "path": "data/models/minicpm5-1b/MiniCPM5-1B-Q4_K_M.gguf",
  "pid": 9577
}
```

`PlannerConfig::from_env()` 在未设置 `DIGILEGACY_LLM_BASE_URL` / `DIGILEGACY_LLM_API_KEY` 时会读取这个文件，因此 Web、CLI 和 HTTP 规划器都会自动使用已启用的本地模型。sidecar 的默认超时为 180 秒，可用 `DIGILEGACY_LLM_TIMEOUT_SECS` 调整，上限 300 秒，以适应小模型在弱 CPU 上的首轮规划。

sidecar 默认以 `--ctx 8192` 启动，可用 `DIGILEGACY_LOCAL_MODEL_CTX` 覆盖，允许 2,048–131,072。上下文越大，KV cache 占用和首 token 时间越高；在 2 核 / 3.9 GiB 环境建议从 8K 或 16K 起步，不要直接按模型原生 128K/1M 设置。

如果服务启动时 `active.json` 指向的 sidecar 已退出且端口不可达，启动过程会删除这条过期记录，避免规划器继续使用死端口；如果 sidecar 仍在监听，则保留记录并在状态接口中恢复。

## 依赖

```bash
python3 -m pip install llama-cpp-python fastapi uvicorn jinja2
```

程序不会自动安装 Python 包；缺少依赖时启用接口会返回错误，日志在 `data/models/runtime.log`。当前发布包已包含 `tools/local_model_server.py`。

## 实测

在 2 核 CPU、3.9 GiB 内存的测试机上：

- 1B Q4_K_M 曾在旧测试中加载约 1 秒（`n_ctx=4096`）；0.0.23 为纠正选型，只预装 2B，1B 本机文件已删除。
- `digilegacy-cli runs plan --task '列出当前所有账户资产。'` 曾使用 1B 成功返回 `assets.list`，`tokensIn=3596`、`tokensOut=19`，耗时约 51.9 秒。
- 写任务同样返回合法 JSON，JSON 语法约束修复了未约束时少一个右括号的问题。
- 2B Q4_K_M 通过服务端下载管理器完成 1,561,318,368 B 下载并通过 SHA256 校验；真实规划耗时 116 秒，0.0.23 作为本机测试模型启用。
- 启用前会重新计算 SHA256，避免只信任 `.gguf.sha256` 标记；完整测试矩阵见 `docs/operations/measurements/local-models-functional-2026-11-20.md`。
- 8K 默认上下文下，向 1B 发送 5,028 prompt tokens 成功（79 秒）；发送 9,028 tokens 时 sidecar 返回 `Requested tokens (9028) exceed context window of 8192`。
- 以 `DIGILEGACY_LOCAL_MODEL_CTX=16384` 重启并启用后，同一份 9,028-token prompt 成功返回，耗时 210.8 秒，证明运行时上下文可调。
- 同一模型并发启用修复后，两个请求均在约 13.55 秒返回 200，指向同一 PID，只留下一个 sidecar；损坏 `active.json`、幽灵 PID、端口耗尽、只读目录、写配额和路径穿越均已实测，结果见测试记录第二轮。

这些数据只证明“当前程序 + 所选测试模型”可用，不代表 128K 长上下文已经在 2 核机器上压测通过；长上下文需要更大的内存和更长的首 token 时间。发布包不携带模型，用户选择的模型只存在本机。
