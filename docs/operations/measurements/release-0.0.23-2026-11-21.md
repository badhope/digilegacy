# DigiLegacy 0.0.23 发布验收报告

日期：2026-11-21  
版本：0.0.23  
提交：`b4bcc14`  
分支：`main`

## 结论

通过。0.0.23 修正了本地模型的定位：所有模型只作为“能力测试候选”，不内置、不默认、不推荐；测试环境实际安装并启用 MiniCPM5-2B，1B 本机文件已删除但目录保留供用户自行选择。完整 Rust/Web 门禁、真实浏览器巡检、打包和包内验收均通过。

## 关键变更

- 后端目录删除 `recommended` 与 `strictUnder2b` 字段；1B/2B 描述改为中性的“测试候选”，不再出现默认或质量档措辞。
- 设置页删除“推荐”徽标、`<2B` / `实际 >2B` 档位徽标；说明统一为“用于能力测试、按需下载和启用；不内置、不默认启用，用户自行选择”。
- 发布包不携带任何 `.gguf` 模型文件，解包后目录内无模型、无 `active.json`。
- 测试环境预装并启用 MiniCPM5-2B；MiniCPM5-1B 本机文件删除，目录保留为可下载测试候选。

## 验证结果

| 项目 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| Rust workspace | 189 passed / 0 failed |
| Web | 79 passed / 0 failed（12 个测试文件） |
| `./tools/verify.sh` | exit 0；契约、架构、Rust、Web、web-budget 全部 OK |
| Web 包体 | JS 109.49 / 110.1 KB gzip；CSS 20.88 / 21.1 KB；总 130.37 / 131.1 KB |
| 真实 Chromium 全路由 | 14/14 PASS；0 failures；axe WCAG A/AA/2.2 AA 0 违规 |
| 设置页桌面 / 390px | 无横向溢出；中文、英文均无“推荐/默认/质量档”文案 |
| 2B 启用 | HTTP 200，41.316 s；PID 20212；`http://127.0.0.1:11435/v1`；默认 8K 上下文 |
| 2B 真实规划 | HTTP 200，104.369 s；`MiniCPM5-2B-Q4_K_M`；`overview.get`；3598 in / 12 out |
| 发布包解包启动 | 0.0.23；1B/2B 均为 `missing`；无 `recommended` / `strictUnder2b` 字段；目录无模型 |
| 五个远端 | GitCode、Gitee、GitHub Morningstar202604、X33834、badhope 全部推送成功 |

## 测试环境模型状态

- `data/models` 约 1.5 GB，仅 `minicpm5-2b/` 与 `runtime.log`。
- 1B：`missing`；2B：`ready` 且 active。
- 2B 文件：`MiniCPM5-2B-Q4_K_M.gguf`，1,561,318,368 B，SHA256 `ec2d5801640099e97d8d7e8003ad4d81f336e757811f03a26173dddf386602fd`。

## 发布物

- 包：`packages/digilegacy-0.0.23-linux-x86_64.tar.gz`，18 MB
- SHA256：`38c9b37b1317733950d46e5ba6348da70fde63fd8c31f6d4d49b05a602f822e3`
