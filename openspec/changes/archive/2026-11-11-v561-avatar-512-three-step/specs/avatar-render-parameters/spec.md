# avatar-render-parameters Specification Delta

## MODIFIED Requirements

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
