# 运维

自托管 DigiLegacy 实例的运行指南。

- [SLO](SLO.md)：服务目标、错误预算、测量命令与实测基线。
- [测量记录](measurements/)：可重复的容量与恢复演练证据。
- [安装、升级与卸载](INSTALL.md) 说明发布包与干净机演练。
- [运行手册](RUNBOOK.md)：启动、日常检查、检查点、恢复、升级与常见故障。
- [连接配置交接](CONNECTION-SETUP.md)：邮箱或 Webhook 账号属于别人时，直接发给填写人。

产品默认是单节点部署。`GET /api/v1/metrics` 以 JSON 暴露进程内计数与延迟桶，不是 Prometheus 文本格式；SLO 文档说明健康探针、metrics、诊断导出与检查点的测量方式。

相关文档：

- [验证与门禁](../VERIFICATION.md)
- [架构说明](../ARCHITECTURE.md)
- [安全策略](../SECURITY.md)
