## 1. 规格与配置

- [x] 1.1 严格校验 proposal/specs/design/tasks，记录架构边界 ADR 0132。
- [x] 1.2 配置 NSIS、资源和图标，实际 Linux/Windows Tauri 构建及 NSIS 打包接受配置。

## 2. 安装态接线

- [x] 2.1 宿主从资源目录提供默认场景，独立壳集成 5/5，安装态真实 engine 预检通过资源轴。
- [x] 2.2 有界 Windows 构建脚本实际生成 NSIS，完整安装烟测 EXIT=0，覆盖活动/同目录/异目录拒绝及卸载数据哈希保留；操作文档已更新。

## 3. 综合验证

- [x] 3.1 独立壳 11+5、clippy/fmt；根 full EXIT=0（1209 Rust/451 Node），Windows 实测与签名/设备缺口分别记录。
- [x] 3.2 开发计划、交付计划、本轮报告均已更新，A12/O01 状态未改变。

## Workflow follow-up

- 验证后归档，并复核主规格、归档合入与文档链接。
