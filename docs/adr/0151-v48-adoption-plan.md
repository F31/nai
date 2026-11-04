# ADR0151：V4.8目标采纳与执行基线切换计划

2026-10-23。状态：Accepted（2026-10-24，V48-01执行切换）。

用户要求阅读V4.8并规划后续开发。采用V4.8为后续规划目标，完整范围保留，命名仍nai-*。当前tests/registry.yaml、catalog/source.json与AGENTS执行约束仍绑定V4.3，不能仅改文档指针就声称164项验收已接入。

V48-01负责实际切换：保留90个旧ACC编号与历史证据，追加74个待映射项，核验INV-01–62；新的状态初始pending。项目源契约机制与V4.8附录M单源要求、默认crate/进程映射分别形成明确决定；无消费者不造空crate。完成生成/注册表/消费者检查后将本ADR改Accepted，并统一更新设计入口、工程上下文和注册表来源。

不要求兼容旧ABI/代码结构，不删除模型、Key、素材或用户数据库。共享模型与回收站永久删除须独立迁移/引用保护与验收。规划不授权真实商务副作用或付费调用。

详见[V4.8计划](../v48-development-plan.md)。本ADR存在不授予Ready或认证。

实际切换：tests/registry.yaml与catalog/source.json已指向V4.8，164ACC/62INV、原90项快照和证据保留。默认边界及单源裁决见[架构与命令清单](../architecture/v48-boundaries-and-command-inventory.md)。本轮无业务数据迁移或新进程。
