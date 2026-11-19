# 验收与门禁

## 1. 门禁总表

计划口径为 G01–G19。M0 是工程壳阶段，只强制其中一部分；未纳入的项在 M1/M2 逐步补齐，不能把“M0 通过”误读成“全部门禁已上线”。

| 编号 | 门禁 | M0 状态 | 实现位置 |
|---|---|---|---|
| G01 | `cargo fmt --check` | 已接入 | `tools/verify.sh` |
| G02 | `cargo clippy -D warnings` | 已接入 | `tools/verify.sh` |
| G03 | 前端类型/风格 | 部分：`vue-tsc` 已接入；ESLint/Prettier 未接入 | `web` scripts |
| G04 | Rust 单测/集成 + 覆盖率 | 部分：单测已接入；覆盖率未接入 | `cargo test` |
| G05 | 前端单测 + 覆盖率 | 部分：vitest 已接入；覆盖率未接入 | `web/src/**/*.test.ts` |
| G06 | 契约检查（生成物 + OpenAPI） | 已接入生成一致性；OpenAPI 已派生 | `tools/generate.mjs --check`、`arch-check` |
| G07 | 生产构建 + 无 mock 分块 | 部分：build 已接入；mock 分块未强制排除 | `web build` |
| G08 | 文档事实一致性 | 未接入 | 计划 M2 |
| G09 | 政策溯源棘轮 | 未接入 | 计划 M2 |
| G10 | 依赖漏洞（npm/cargo） | 未接入 | 计划 M1 |
| G11 | 密钥扫描 | 未接入 | 计划 M1 |
| G12 | E2E 主路径 + 可访问性 | 未接入 | 计划 M2/M3 |
| G13 | 性能冒烟（回归 >20%） | 部分：bundle 预算 + bus 基线 | `tools/bench/*` |
| G14 | 视觉回归（关键 6 页） | 未接入 | 计划 M2 |
| G15 | Agent 工具权限与审批 | 已接入 | 生成器 + `arch-check` + MCP 过滤 |
| G16 | 页面→命令映射 | 部分：路由 meta 已声明；未做自动校验 | `web/src/app/router.ts` |
| G17 | 加载预算与首屏 | 部分：bundle 预算；CLS/长任务未测 | `tools/bench/web-budget.mjs` |
| G18 | 模块边界/架构测试 | 已接入 | `tools/arch-check.mjs` |
| G19 | ADR/变更单 | 已建立流程与模板 | `docs/adr/`、`docs/GOVERNANCE.md` |

## 2. 一键验证

```bash
./tools/verify.sh
```

步骤：

1. `node tools/generate.mjs --check`
2. `node tools/arch-check.mjs`
3. `cargo fmt --all -- --check`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `cargo test --workspace`
6. Web `npm run typecheck`
7. Web `npm test`
8. Web `npm run build`
9. `node tools/bench/web-budget.mjs`

## 3. M0 已验证结果

### 契约

```text
contract check: 13 commands, 10 tools, 18 schemas, 19 error codes
arch-check: OK
- workspace members: 12
- core crates: 8; app crates: 4
- legacy route gate: 8 routes unresolved by catch-all
- generated artifacts: 10
```

### Rust

```text
cargo fmt --all -- --check        OK
cargo clippy --workspace --all-targets -- -D warnings  OK
cargo test --workspace            OK
```

已观察测试：

- foundation：5 个
- render：1 个
- server：5 个
- 其余 crate：0 个（M0 边界）

### Web

```text
npm run typecheck   OK
npm test            OK（6 tests）
npm run build       OK
```

包体基线：

```text
总 gzip         约 81.27 KB / 110 KB
入口 raw        41.33 KB / 60 KB
vendor raw      98.50 KB / 130 KB
路由最大 raw    6.48 KB / 24 KB
```

### 运行态 smoke

| 入口 | 请求 | 结果 |
|---|---|---|
| Server | `GET /api/v1/health` | `200 OK`，13 commands |
| Server | `GET /api/v1/overview` | `501 UNSUPPORTED`（M0 预期） |
| Server | 未知命令 | `404 NOT_FOUND` |
| Server | `assets.create` | `401 AUTH_REQUIRED`（M0 写 token 未接） |
| Server | `connections.test` | `403 FORBIDDEN` |
| CLI | `commands` / `health` | OK |
| CLI | 未知命令 | `NOT_FOUND` |
| MCP | `initialize` / `tools/list` | OK |
| MCP | `tools/call assets.list` | `isError=true`，不伪造成功 |

### 性能基线

```text
bus_bench: 1,000,000 dispatches in 250.19ms
          ≈ 250.2 ns/op，约 4.0M ops/s
```

## 4. M0 退出条件

M0 可以判定“工程壳完成”的条件：

- [x] 新仓库结构可编译
- [x] 契约单一事实源可生成、可校验
- [x] 四入口空壳可启动/可调用
- [x] Web 可启动、可浏览 5 主入口
- [x] 旧路由 404
- [x] 架构门禁与 bundle 预算
- [x] CI 与 verify 脚本
- [x] 定位/架构/验收/DoR-DoD/ADR/RFC
- [ ] 真实领域 handler
- [ ] 安全内核与数据库
- [ ] E2E 与视觉回归
- [ ] 无 mock 生产分块

后四项属于 M1/M2，不应在 M0 里伪造为已完成。

## 5. 回归规则

- 性能回归超过 20%：必须给出原因、影响面与回滚方案。
- 契约变更导致 Web/CLI/MCP 任一消费者漂移：阻断合并。
- 新增写命令没有 `approval=required` 或没有审计字段：阻断合并。
- 把 `key/network/desktop` 暴露给 Agent/MCP：阻断合并。
- 旧路由重新出现：阻断合并。
- 前端新增页面没有加载/空/错误三态：阻断合并。
