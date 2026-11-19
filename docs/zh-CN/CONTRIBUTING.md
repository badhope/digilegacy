# 参与贡献

[English](../../CONTRIBUTING.md) | 简体中文

DigiLegacy 是契约先行的平台项目。改业务能力时，先改契约，再改实现，最后改 UI。

## 开工前

- 读[产品定位](POSITIONING.md)、[架构说明](ARCHITECTURE.md)和[验证与门禁](VERIFICATION.md)。
- 确认改动属于哪个领域、哪个模块、哪条命令。
- 新业务能力先走 RFC，再落进 `contracts/`。
- 新代码写在 `core/`、`apps/` 或 `web/`；不要恢复已删除的桌面端。

## 契约规则

1. `contracts/commands.json`、`tools.json`、`errors.json`、`schemas/` 和 `mocks.json` 是唯一事实源。
2. 不要手改生成文件，它们由契约重新生成。
3. 改完契约后执行：

```bash
node tools/generate.mjs
node tools/generate.mjs --check
```

4. 新命令需要输入/输出 schema、权限档、HTTP/CLI/MCP 传输声明、错误码引用、Web 需要的 mock 数据，以及至少一个架构或行为测试。
5. 新工具必须引用已有命令；写工具必须 `approval=required`；MCP 只暴露读与草稿。

## 实现规则

- 业务逻辑放在 `core/domains` 或对应应用层，不放在入口层。
- 所有入口走同一个 `CommandBus`，不新建第二套后端。
- Web 只通过类型化命令访问平台；mock 只用于本地演示。
- 错误使用 `contracts/errors.json` 里的 code，不在传输层临时造字符串。
- 写路径同时考虑权限、审批和审计。
- 安全代码迁移保留算法和测试向量，不在搬代码时重写加密。

## 分支与提交

分支使用 `feat/<module>-<short>`、`fix/<issue>-<short>` 或 `docs/<topic>`。

```text
feat(assets): add asset list cursor pagination
fix(agent): reject self-approved write runs
docs(adr): record contract-first decision
```

一个 Pull Request 只解决一个可验收问题。重构、格式化和行为改动不要混在一起。

## 合并前

```bash
./tools/verify.sh
```

所有门禁必须通过。PR 里写清受影响的模块、命令或页面，用户可见变化，权限与数据影响，验证命令和结果，以及有意留到后续的部分。

## 完成的含义

- 代码、契约、测试和文档在同一个 PR 交付。
- 不留下伪造的成功态。
- UI 覆盖加载、空态和错误态。
- 数据库改动按安全顺序：先查约束，小样本试写，再复验列表、详情和下载。
- 符合[工程治理](GOVERNANCE.md)里的完成定义。
