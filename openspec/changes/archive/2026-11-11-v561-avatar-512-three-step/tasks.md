# Tasks

- [x] B01 Rust `Config` 校验接受 2/3/4，更新序列化往返与拒绝用例测试；nai-workbench-engine 测试通过。
- [x] B02 Python `config_sample_steps`/`override_sample_steps` 支持 3（映射 [1000,500,250]），CPU 测试与相邻回归通过。
- [x] B03 验收运行目录形象配置切到 512/3；重建 Windows 二进制并核对 exe mtime（DrvFs 缓存教训）。
- [x] B04 原生三句验收（新 artifacts 目录）：既有断言全量保留，记录 TTS 调用/首声/句间/RTF 与前导。（5 次尝试账本；attempt 11 三句端到端协议全绿、model_api_calls=3；唯一红断言 ready_count=0 为既有 gapless 前沿、canonical 同红，断言未放宽；详见报告）
- [x] B05 报告与开发计划如实记录；34ACC 状态不变，失败账本保留。
