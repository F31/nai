# Proposal

## Why
512/2 步接入受控播放后（归档 2026-11-11-v561-avatar-512-two-step），用户实测视频模糊。
原生分辨率抽看帧证实：2 步面部细节（皮肤纹理/胡茬/唇须）明显丢失，512/4 完好。
上游 lite 检查点官方默认即 4 步（flash_head_pipeline.py:166-172），2 步非蒸馏主目标。
同日 3 步候选实验（实验轨）：[1000,500,250]（4 步网格子集）抽看帧质量≈4 步，
compiled-warm 1812ms（RTF 0.47，含采集 IO）；[1000,500,1] 仍偏软，拒绝。

## What Changes
- 配置面 sample_steps 由 {2,4} 扩到 {2,3,4}；3 步固定映射上游网格子集时间表
  [1000, 500, 250]（加终止 0，经相同 shift 变换），与 2/4 步同一覆盖点、同一顺序。
- 双侧校验同步：Rust Config 接受 2/3/4，Python config_sample_steps/override_sample_steps 同步；
  其他值（含浮点/bool/字符串）照旧拒绝。
- 验收运行目录形象配置由 512/2 切到 512/3；诊断用 override_timesteps 保持实验轨，
  不进生产配置面。
- 质量证据：同 PCM/同种子 3 步抽看帧（048/095）与 4 步对照；三句原生验收既有断言不放宽。

## Impact
- 受影响：apps/workbench-engine Config 校验与测试、scripts/avatar_step_override.py、
  tests/avatar_step_override_test.py、验收运行目录配置。
- 3 步为自定义网格子集时间表（上游仅特判 2/4）：分钟级长序列身份/口型稳定性未验证，
  512/4 保留为质量优先退路；不移动 34ACC，gapless 红断言维持既有状态。
- 决策门（ADR 0203）记录：512/3 取代 512/2 成为首选候选，512/2 因用户实测模糊降级为
  诊断用途，768/2 维持拒绝。
