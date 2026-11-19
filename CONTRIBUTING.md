# Contributing

DigiLegacy 是契约先行的平台项目。改动业务能力时，先改契约，再改实现，最后改 UI。

## 1. 开工前

- 读 [平台定位](docs/POSITIONING.md)、[架构说明](docs/ARCHITECTURE.md)、[验收与门禁](docs/VERIFICATION.md)。
- 确认改动属于哪个领域、哪个模块、哪个命令。
- 新的业务能力先走 RFC，再落到 `contracts/`。
- 旧版源码已删除；新能力只在 `core/`、`apps/`、`web/` 内实现，按契约与模块边界落地并补测试。

## 2. 契约规则

1. `contracts/commands.json`、`tools.json`、`errors.json`、`schemas/`、`mocks.json` 是唯一事实源。
2. 不手改以下生成文件：
   - `web/src/platform/generated/*`
   - `core/contracts/src/generated.rs`
   - `core/contracts/src/errors.rs`
   - `apps/cli/src/generated.rs`
   - `apps/server/src/generated.rs`
   - `apps/mcp/tools.generated.json`
   - `contracts/generated/tool-manifest.json`
   - `contracts/openapi.json`
3. 改完契约后执行：

```bash
node tools/generate.mjs
node tools/generate.mjs --check
```

4. 新增命令必须同时有：
   - input/output schema
   - 权限档
   - HTTP/CLI/MCP 传输声明
   - 错误码引用
   - mock 数据（如果 Web 需要展示）
   - 至少一个架构或行为测试
5. 新增工具必须引用已有命令；写工具必须 `approval=required`；MCP 只能暴露 read/draft。

## 3. 实现规则

- 业务逻辑只放在 `core/domains` 或对应领域 crate；入口层不写业务规则。
- 所有入口最终走同一个 `CommandBus`，不新增第二套后端。
- Web 只通过平台命令访问数据；M0 可走 mock，M1 起走 Platform API。
- 错误使用 `contracts/errors.json` 中的 code，不在入口层临时造字符串。
- 任何写操作都要有权限、审批、审计三类边界的考虑。
- 安全内核迁移不得重写加密算法；按模块迁入并保留测试向量。

## 4. 分支与提交

- 分支：`feat/<module>-<short>`、`fix/<issue>-<short>`、`docs/<topic>`。
- 提交信息：

```text
feat(assets): add asset list cursor pagination
fix(agent): reject self-approved write runs
docs(adr): record contract-first decision
```

- 一个 PR 只解决一个可验收问题；不要把重构、格式化、业务改动混在一起。

## 5. 合并前检查

```bash
./tools/verify.sh
```

必须全部通过。

PR 描述中写清楚：

- 对应模块 / 命令 / 页面
- 用户可见变化
- 权限与数据影响
- 验证命令与结果
- 未覆盖项与后续

## 6. 完成定义

- 代码、契约、测试、文档同一 PR 交付。
- 没有伪造的成功态；M0 空壳必须明确返回 `UNSUPPORTED` 或边界说明。
- 前端改动至少覆盖加载、空、错误三种状态。
- 涉及数据库/迁移时，遵循“先查枚举与约束、小样本试写、改后复验列表/详情/下载三条链路”。

详细 DoR/DoD 见 [docs/GOVERNANCE.md](docs/GOVERNANCE.md)。
