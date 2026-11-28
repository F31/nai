# Proposal

## Why
2026-11-11 步数×分辨率矩阵（实验轨）实测：现行 768/4 步稳态 RTF 1.151 超预算；
512/4 步 0.704、512/2 步 0.505 均在 ≤0.80 预算内，512 为上游原生默认分辨率，
2 步时间表为上游一等配置；768/2 步质量拒绝。三句验收长期失败于
ready_before_previous_device_end_count=0，其根因是生成吞吐低于音频消费。
ADR 0203 已把分辨率/步数列为需质量门护送的变量，本变更把 512+可配步数接入正式配置面。

## What Changes
- Rust 形象 worker `Config` 增加可选 `sample_steps`（仅 2 或 4，缺省维持 4），
  `deny_unknown_fields` 语义不变；旧配置无此字段行为不变。
- Python `Backend` 初始化时按配置应用时间表覆盖（复刻上游 2/4 步时间步与变换顺序），
  非 2/4 值拒绝；统计如实上报实际步数。
- 开发运行 profile 的形象配置由 768/4 步切到 512/2 步；768 仅保留为离线/诊断用途，
  不删旧产物。人工口型/身份质量门在三句验收中一并执行，不预先宣称质量通过。

## Impact
- 受影响：apps/workbench-engine 形象 worker 配置、scripts/flashhead_resident_backend.py、
  开发 profile 配置；v561_live_control 等解析该配置的二进制须重建后方可接受新字段。
- 不移动 34ACC 状态；三句原生验收（ready_before_previous_device_end_count>0 等既有断言）
  作为本轮证据目标，断言不放宽。
- 决策门（ADR 0203）记录：512/2 为首选候选，512/4 为质量优先退路，768/2 已拒绝。
