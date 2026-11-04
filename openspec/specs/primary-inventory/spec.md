# primary-inventory Specification

## Purpose
TBD - created by archiving change v52-primary-inventory. Update Purpose after archive.

## Requirements

### Requirement: 固定主组合并区分事实和目标
实施记录 SHALL 标明所选硬件、前端、模型路线及运行边界，对未经核实的 Qwen 能力和身份保留未知，不能将历史在线证据作为当前状态。

#### Scenario: 服务未监听
- **WHEN** 本机已知 Qwen 端点不可达
- **THEN** 记录当前不可达且不自动启动服务，历史合成证据保持独立

### Requirement: 验收映射不得冒充基线切换
实施记录 SHALL 保留已有注册表和编号，并将 V5.2 新验收目标与已有项目显式关联；映射不能改变通过状态。

#### Scenario: 实际 GPU 算子成功
- **WHEN** 已校验的 PyTorch 在目标 GPU 完成算子
- **THEN** 仅记录 GPU 算子证据，不标记写实模型、OBS 或整机长稳通过
