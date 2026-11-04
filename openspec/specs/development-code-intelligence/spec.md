# development-code-intelligence Specification

## Purpose
为nai后续开发提供本地代码图查询，在严格保留已有开发约束、缓存位置和隐私边界的条件下辅助源码导航与改动影响分析，不作为功能验收依据。

## Requirements

### Requirement: 本地代码智能工具须有界接入
系统 SHALL 将CodeGraph和Graphify开发工具及新索引保存在项目E盘缓存目录，两者互相排除图谱产物和本机凭据。CodeGraph承担日常增量代码定位，Graphify按需提供整体图与显式文档引用分析；不要求每次双全量，不将未经语义提取的图冒称设计语义理解。保留历史产物，不替代源码、契约、测试或认证。
#### Scenario: 安装并验证
- **WHEN** 两个工具均完成项目配置和各自图谱查询
- **THEN** 同一当前模块可定位，图谱来源/版本可追溯，索引和缓存隔离，业务资料/凭据与既有开发约束保持有效
