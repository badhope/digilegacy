# DigiLegacy

[English](README.md) | 简体中文

**面向数字资产规划与传承的自托管、API-first 平台。**

数字遗产往往散落在笔记、密码管理器和各家平台规则里。DigiLegacy 把它们收进一个可审计的工作区：你拥有什么、交给谁、什么时候释放，以及每一步留下了什么依据。

[文档](docs/zh-CN/README.md) · [架构](docs/zh-CN/ARCHITECTURE.md) · [参与贡献](docs/zh-CN/CONTRIBUTING.md) · [安全](docs/zh-CN/SECURITY.md)

## 为什么做 DigiLegacy

- **同一份事实。** 资产、继承人、触发状态、平台政策与交付物共用一套数据模型。
- **有 Agent，但不放权。** Agent 可以读取、规划和起草；任何改动数据的动作都要等人确认。
- **默认私密。** 敏感字段在写入数据库前加密，平台可以跑在自己的电脑或服务器上。
- **全程可审计。** 导入、审批、政策变更和交付都留下可回看的记录。

## 主要能力

| 模块 | 你能得到什么 |
|---|---|
| **资产** | 账号、订阅、设备和其他数字财产的清单，附带各平台的继承规则提示。 |
| **规划** | 继承人、执行人、紧急联系人，以及可暂停、恢复和签到的释放链。 |
| **交付** | 人能读懂的报告、清单、法律文本模板，以及给执行人使用的工具包。 |
| **自动化** | 由工具注册表驱动的 Agent 工作台。只读工具直接执行，写工具等待审批。 |
| **政策库** | 带来源记录的平台政策，以及远程更新的人工审核队列；批准前不覆盖。 |
| **运行能力** | 加密连接、通知渠道、后台任务、审批和审计哈希链。 |

## 架构概览

```text
Web 控制台          CLI        MCP        HTTP 客户端
      \             |          |            /
       \            |          |           /
        └────── 平台 API / CommandBus ──────┘
                           |
                    领域层与应用层
                           |
              存储 · 安全 · 外部集成
                           |
                 审计 · 任务 · 审批
```

Web、CLI、MCP 和 HTTP API 都调用同一个命令层。命令只在 `contracts/` 描述一次，再生成到 Rust、TypeScript、OpenAPI 和工具清单，避免接口漂移。

后端是模块化 Rust 单体，前端是 Vue 3 并按路由拆包，数据库使用 SQLCipher，敏感字段额外做字段级加密。

## 快速开始

需要 Rust、Node.js，以及用于编译 SQLCipher 的 C 工具链。

```bash
# 1. 生成契约产物
node tools/generate.mjs

# 2. 启动 API
DIGILEGACY_BIND=127.0.0.1:8787 \
DIGILEGACY_API_TOKEN=change-me \
cargo run -p digilegacy-server

# 3. 启动 Web 控制台
cd web
npm install
npm run dev
```

打开 `http://127.0.0.1:5173`。写操作使用 **设置 → 连接** 中配置的写令牌。

CLI 和 MCP 也是本地一等入口：

```bash
cargo run -p digilegacy-cli -- health
cargo run -p digilegacy-cli -- commands --json

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | cargo run -p digilegacy-mcp
```

## 仓库结构

```text
core/          领域逻辑、契约、存储、安全、外部集成
apps/          HTTP 服务、CLI 与 MCP 入口
web/           Vue 控制台
contracts/     命令、工具、错误、schema 与生成的 OpenAPI
content/       政策数据与翻译
docs/          产品、架构、运维与交付文档
tools/         生成器、架构门禁与性能基线
```

## 文档

- [中文文档索引](docs/zh-CN/README.md)
- [产品定位](docs/zh-CN/POSITIONING.md)
- [架构说明](docs/zh-CN/ARCHITECTURE.md)
- [验证与发布门禁](docs/zh-CN/VERIFICATION.md)
- [协作治理](docs/zh-CN/GOVERNANCE.md)
- [架构决策](docs/zh-CN/adr/)
- [English documentation](docs/README.md)

## 项目状态

核心产品链路已经可用，仓库仍在持续开发。公开接口由契约驱动，测试与发布门禁已就位，后续工作记录在更新日志和 issue 中。

## 参与贡献

修改命令、schema 或政策前，请先读 [CONTRIBUTING](docs/zh-CN/CONTRIBUTING.md)。安全问题请按 [SECURITY](docs/zh-CN/SECURITY.md) 私下报告。

## 许可证

MIT，见 [LICENSE](LICENSE)。
