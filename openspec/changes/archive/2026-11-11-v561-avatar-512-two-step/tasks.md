# Tasks

- [x] A01 Rust `Config` 增加可选 `sample_steps`（仅 2/4）与校验、序列化往返测试。
- [x] A02 Python `Backend` 按配置应用时间表覆盖，非法值拒绝；CPU 测试与相邻回归通过。
- [x] A03 开发 profile 形象配置切到 512/2 步；重建解析该配置的 Windows 二进制到独立验收运行目录（不替换用户运行版）。
- [x] A04 原生三句验收：既有断言全量保留（含 ready_before_previous_device_end_count>0），记录真实 TTS 调用次数/费用、首声/句间/RTF 与机器状态前导；附人工抽看帧。（已执行 6 次尝试；三句端到端协议全绿、model_api_calls=3；唯一红断言 ready_count=0 为既有 gapless 前沿、canonical 同红，断言未放宽；详见 2026-11-11 报告）
- [x] A05 报告与开发计划如实记录；34ACC 状态不变，失败账本保留。
