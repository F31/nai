# nai 设计入口

## 新版规划目标（2026-10-23）

已审视[V4.8完整方案](digital-host-agent-design-V4.8.md)，后续任务见[V4.8开发计划](v48-development-plan.md)及[ADR0151采纳提案](adr/0151-v48-adoption-plan.md)。V48-01已完成实际基线切换：当前执行V4.8注册表164ACC/62INV，全部pending；旧通过状态不自动沿用。默认边界见[架构裁决](architecture/v48-boundaries-and-command-inventory.md)。

## V4.3历史重构目标

此前重构产品/技术目标为 [digital-host-agent-design-V4.3.md](digital-host-agent-design-V4.3.md)，由 [ADR0147](adr/0147-v43-product-reconstruction-baseline.md) 记录采纳范围与仓库裁决。以真人主导的“备播—预演—协同直播—复盘—下场复用”为主线，不承担旧代码/ABI/场景配置兼容；工程标识仍为nai-*。

- [仓库重构设计](architecture/v43-product-reconstruction.md)：领域、进程、存储/契约、UI、回执与数据保护。
- [后续任务与第一张工程卡](v43-reconstruction-plan.md)：V43-RF01–12、依赖与里程碑。
- [V4.3全部ACC目标映射](acceptance/v43-target-matrix.md)：仅目标索引，不是通过记录。
- [本轮审视报告](reports/2026-10-22-v43-product-reconstruction-review.md)。

## 当前实现和证据边界

现有安装仍为V3.11演进的Engineering实现，未被新线替换；当前可操作能力与限制见[开发计划](development-plan.md)。RF01最小装配与RF02存储/私有交付基础已实现，用户备播/直播等完整业务目标尚未实现，不能把设计章节、历史测试数、单项设备或Provider证据当成新产品完成。

[tests/registry.yaml](../tests/registry.yaml)为V4.3唯一实际验收注册表，RF01已切换Schema/检查器和消费者，原表归档至tests/legacy/v311。90项ACC仍pending；[RF01报告](reports/2026-10-22-v43-rf01-contracts-and-bootstrap.md)中的前置证据不授予整项通过。

V3.11及此前方案、ADR和OpenSpec用于历史追溯，不自行拼接旧条款覆盖V4.3。现有库/Key/素材/许可保留，新数据根与凭据隔离；是否需要旧资产导入另按映射、预览、备份与授权执行。
