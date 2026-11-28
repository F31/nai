# avatar-render-parameters Specification

## Purpose
TBD - created by archiving change v561-avatar-512-two-step. Update Purpose after archive.

## Requirements

### Requirement: 形象渲染分辨率与采样步数可配置
形象 worker 配置 SHALL 显式携带渲染分辨率（width/height）与可选采样步数（sample_steps）；
sample_steps 仅接受 2、3 或 4，缺省或缺字段 SHALL 等价于 4（上游 lite 默认）。
任一消费者对其他值 SHALL 显式拒绝，不得静默取最近合法值。
3 步 SHALL 映射为 4 步网格子集时间表 [1000, 500, 250]（加终止 0，经相同 shift 变换）。

#### Scenario: 缺省配置行为不变
- **WHEN** 配置不含 sample_steps 字段
- **THEN** worker 按 4 步时间表初始化，事件统计 inference_steps 为 4。

#### Scenario: 显式 2 步生效并可观测
- **WHEN** 配置 sample_steps 为 2 且通过双侧校验
- **THEN** 生成使用上游 2 步固定时间表（[1000, 500] 加终止 0，经相同 shift 变换），
  事件统计 inference_steps 为 2。

#### Scenario: 显式 3 步生效并可观测
- **WHEN** 配置 sample_steps 为 3 且通过双侧校验
- **THEN** 生成使用 4 步网格子集时间表（[1000, 500, 250] 加终止 0，经相同 shift 变换），
  事件统计 inference_steps 为 3。

#### Scenario: 非法步数拒绝
- **WHEN** 配置 sample_steps 为 2/3/4 以外的值
- **THEN** Rust 配置校验与 Python 后端初始化均显式失败，不装载模型、不产出帧。

### Requirement: 直播路径分辨率切换需质量门
直播路径渲染分辨率从既有值切换 SHALL 以同 PCM/同种子矩阵计时与人工抽看帧为前置证据，
且三句原生验收既有性能断言不放宽；768 分辨率 SHALL NOT 作为直播路径默认。

#### Scenario: 切换证据链
- **WHEN** 开发 profile 切到 512/2 步
- **THEN** 存在同会话 768/4 对照计时、像素指标与抽看帧产物，三句验收结果含
  ready_before_previous_device_end_count 实际值，失败时保留账本。
