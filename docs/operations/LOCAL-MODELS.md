# 本地模型

DigiLegacy 0.0.20 在“设置 → 连接”内加入本地模型目录、下载和启用。目标不是做一个模型市场，而是让默认模型下载后马上能被规划器使用，避免用户再去装 Ollama 或手工改环境变量。

## 默认模型

| 模型 | 参数 | 上下文 | Q4_K_M 大小 | 许可 | 运行时 | 说明 |
| --- | --- | --- | --- | --- | --- | --- |
| **MiniCPM5-1B** | 1.08B 稠密 | 131,072 | 688,065,920 B | Apache-2.0 | 标准 llama.cpp | 默认推荐；1B 级开源 SOTA，标准 `LlamaForCausalLM`，接入成本最低 |
| MiniCPM5-2B | 2.52B 稠密 | 131,072 | 1,561,318,368 B | Apache-2.0 | 标准 llama.cpp | 质量档；实际参数超过 2B，不满足“严格 <2B” |
| Spark-X2.5-1.7B | 1.71B 稠密 | 1,048,576 | 1,107,457,856 B | Apache-2.0 | 实验 | 严格 <2B 里上下文最长；本机 `llama-cpp-python 0.3.37` 已能加载并完成规划，但仍按实验运行时标注 |

MiniCPM5-1B 的下载源为 ModelScope：

`https://modelscope.cn/models/OpenBMB/MiniCPM5-1B-GGUF/resolve/master/MiniCPM5-1B-Q4_K_M.gguf`

SHA256：

`81b64d05a23b17b34c475f42b3e72fbde62d4b92cc34541f7a8031d0752deafa`

## 下载

- 默认目录：`data/models/<model-id>/`，可用 `DIGILEGACY_MODELS_DIR` 覆盖。
- 下载由服务端发起，使用 `curl -L --fail --retry 3` 写入 `.gguf.part`，前端轮询进度。
- 完成后流式计算 SHA256，只有与目录中的官方值一致才改名为最终 `.gguf`，并写入 `.gguf.sha256` 标记。
- 下载和启用都需要平台写令牌（`DIGILEGACY_API_TOKEN`）。

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

`PlannerConfig::from_env()` 在未设置 `DIGILEGACY_LLM_BASE_URL` / `DIGILEGACY_LLM_API_KEY` 时会读取这个文件，因此 Web、CLI 和 HTTP 规划器都会自动使用已启用的本地模型。sidecar 的默认超时为 180 秒，以适应小模型在弱 CPU 上的首轮规划。

## 依赖

```bash
python3 -m pip install llama-cpp-python fastapi uvicorn jinja2
```

程序不会自动安装 Python 包；缺少依赖时启用接口会返回错误，日志在 `data/models/runtime.log`。当前发布包已包含 `tools/local_model_server.py`。

## 实测

在 2 核 CPU、3.9 GiB 内存的测试机上：

- MiniCPM5-1B Q4_K_M 加载约 1 秒（`n_ctx=4096`）。
- `digilegacy-cli runs plan --task '列出当前所有账户资产。'` 使用当前程序链路成功返回 `assets.list`，`tokensIn=3596`、`tokensOut=19`，耗时约 51.9 秒。
- 写任务同样返回合法 JSON，JSON 语法约束修复了未约束时少一个右括号的问题。
- 2B Q4_K_M 通过服务端下载管理器完成 1,561,318,368 B 下载并通过 SHA256 校验；真实规划耗时 116 秒。
- Spark-X2.5-1.7B Q4_K_M 通过 Web 面板真实下载 1,107,457,856 B 并校验通过；sidecar 启动 3 秒，真实规划 83 秒。
- 启用前会重新计算 SHA256，避免只信任 `.gguf.sha256` 标记；完整测试矩阵见 `docs/operations/measurements/local-models-functional-2026-11-20.md`。

这些数据只证明“当前程序 + 默认模型”可用，不代表 128K 长上下文已经在 2 核机器上压测通过；长上下文需要更大的内存和更长的首 token 时间。
