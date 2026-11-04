# R1 设备回调授权边界（2026-10-20）

本轮实现设备接线前的一个软件切片，**未完成真实设备输出主线**。原 keeper 演练和批准后文件预览均未改变；没有声卡、窗口或硬件完成实测，不升级 tests/registry.yaml。

## 实现

- nai-media-core 新增 DeviceChunk，只能经 MediaGate 的当前 Preplay 校验及队列消费取得；旧审批、错误事实/候选哈希等沿用既有完整检查。
- PCM 明确 8–192kHz、单/双声道、有限且归一化 f32、最多 600ms。无新增依赖或跨语言契约，未修改生成代码。
- 回调清零输出后逐帧复制，检查独立 P0 原子信号和 epoch；换代开始、期限失效及 owner 析构撤销旧对象。
- 单次回调最多 10ms，按整个缓冲跨度预留授权期限。候选/审批/值守与租约取最早期限；续租不会延长已交出的块。异常对象永久停止，不自动恢复。
- 回调无锁、分配、IO；返回的是复制样本数，不是设备已播放数，也不产生 OutputAck。

## 验证

- WSL/Linux `cargo test -p nai-media-core --offline`：100 passed / 0 failed（含新增 8 项）。
- 真实 Windows/MSVC `cargo test -p nai-media-core --test gate --locked`：19 passed / 0 failed。仅软件门控，不是设备实测。
- `cargo clippy -p nai-media-core --all-targets --offline -- -D warnings` 与 `cargo fmt --all --check`：通过。
- `cargo xtask check --profile portable`：EXIT=0，105 个 Rust 测试目标、1217 passed / 0 failed；workspace fmt/clippy、全部 Rust 回归及便携工程检查通过。检查不代表设备/SDK/GPU/安装认证。
- OpenSpec 实施前及实施后 strict 校验通过。
- 变更归档为 `openspec/changes/archive/2026-10-20-r1-device-callback-gate/`，增量合入 media-basics-timeline。
- 归档后主规格 43/43、归档 162/162；spec-merge/archived-proposal 回归 8/8。

新增回归覆盖正常复制/尾部静音、持有业务锁时 P0、owner 析构、换代、控制停顿、缓冲跨度越界、永久停止、PCM 非法/超预算拒绝不消费、审批撤销/提前到期、续租不延长旧块、双声道及其他 gate 隔离。使用单调时间推进，不使用 sleep。

## 后续缺口

Windows 原生音频/窗口后端、keeper 原始媒体传输、批准候选端到端调用与设备缓冲延迟约束仍未实现；并非外部阻塞。实际后端必须把设备排队延迟纳入期限，并提供自己的完成证据。付费 Provider、正式签名和硬件验收仍未完成。本次没有真实音频播放，没有重新生成安装器，改动未提交。
