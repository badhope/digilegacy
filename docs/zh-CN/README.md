# 文档索引

[English](../README.md) | 简体中文

## 从这里开始

- [产品定位](POSITIONING.md)：说明 DigiLegacy 是什么、服务谁、明确不做什么。
- [架构说明](ARCHITECTURE.md)：命令层、领域边界、数据模型、Agent 审批与部署形态。
- [验证与门禁](VERIFICATION.md)：发布门禁与上线前的人工 smoke 路径。
- [运维](operations/README.md)：SLO、检查点、恢复与事故处理。
- [工程治理](GOVERNANCE.md)：进入开发、完成定义与评审规则。
- [参与贡献](CONTRIBUTING.md)：如何改代码而不破坏生成客户端。
- [安全策略](SECURITY.md)：如何报告安全问题。
- [行为准则](CODE_OF_CONDUCT.md)：社区交流的基本约定。

## 决策记录

- [ADR](adr/)：记录难以回退的架构选择。
- [RFC 模板](../rfc/0001-template.md)：还没定、需要讨论的方案从这里开始。

## 工程档案

交付计划和模块注册表放在 [plan/](../plan/README.md)。它们是内部工作笔记，保留中文，不是使用或参与项目的前提。公开文档以英文维护，并在 [zh-CN](README.md) 下提供中文翻译。

## 阅读顺序

1. 定位：这个产品是不是要解决的问题？
2. 架构：这次改动应该放在哪一层？
3. 治理：改动是否 Ready，什么算 Done？
4. 验证：哪些门禁和用户路径必须通过？
5. ADR / RFC：这个决定是否需要留档？
