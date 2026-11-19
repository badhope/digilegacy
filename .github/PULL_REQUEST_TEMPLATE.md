## 变更说明

<!-- 一句话说明用户可见变化 -->

## 关联

- 模块 / 命令：
- 页面 / 入口：
- ADR / RFC：
- Issue：

## 类型

- [ ] feat
- [ ] fix
- [ ] refactor
- [ ] docs
- [ ] test
- [ ] chore

## 契约与权限

- [ ] 改了 `contracts/*`，已运行 `node tools/generate.mjs`
- [ ] 新增命令有 input/output schema
- [ ] 写工具 `approval=required`
- [ ] MCP 未暴露 `key/network/desktop`
- [ ] 前端页面动作能在 manifest 找到对应命令

## 验证

- [ ] `./tools/verify.sh` 通过
- [ ] Rust fmt/clippy/test 通过
- [ ] Web typecheck/test/build 通过
- [ ] 页面加载/空/错误三态已验证
- [ ] bundle 预算未回退超过 20%

## 数据与安全

- [ ] 无新增密钥/敏感信息
- [ ] 数据库变更含迁移与回滚
- [ ] 写操作有审计
- [ ] 未把旧版代码直接拷入新包

## 截图 / 录屏

<!-- 前端改动请附关键页面截图；不要只写“UI 已改” -->

## 未覆盖项

<!-- 明确写出，不要用“后续优化”糊过去 -->
