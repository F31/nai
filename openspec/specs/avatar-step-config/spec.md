# avatar-step-config Specification

## Purpose
TBD - created by archiving change v561-step-config-schema-alignment. Update Purpose after archive.

## Requirements

### Requirement: 步数配置跨消费者一致
avatar-worker配置 SHALL 将sample_steps声明为可选整数2/3/4，缺省保持既有四步行为；正式生成类型不得遗漏此字段。
#### Scenario: 三步配置
- **WHEN** 配置显式sample_steps为3
- **THEN** Schema接受并与Rust/Python消费者一致，不自动修改用户配置。
#### Scenario: 未支持实验字段
- **WHEN** 配置含任意实验时间表或未装配候选入口
- **THEN** Schema拒绝，不把实验选项作为生产配置接受。
