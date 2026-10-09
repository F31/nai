# D11 媒体基础与样本时轴实测报告

- 日期：2026-10-08
- 环境：Linux/WSL，Rust 1.96.0，锁定 pnpm/OpenSpec/protoc/libprotobuf
- 变更：`openspec/changes/d11-media-basics/`（本轮归档）
- 结论：可移植媒体时序内核（样本时轴/时钟/缓冲/录制/ACK）已实现并实测；Windows 独立窗口/音轨与设备为 Blocked。

## 实现

`nai-media-core::timeline`（audio/clock/ring/resample/record/ack + utterance 时轴），纯逻辑仅依赖 contracts。
audio 校验 seq 单调、同句采样率/声道/格式不变、end_of_stream 终块；clock 位置只增、静音连续、有理 pts，
跨率用精确比例映射不直接比较异率 sample_start；ring 超限返回 Capacity 不静默丢弃；
timeline 音频+视频双水位才 commit，持续迟到返回 Late；recorder 按媒体时钟位置原子补静音、PTS 单调；
ack 仅在实际播放完成 ACK（committed+actual+样本匹配）时标 Completed，部分 ACK 保持 Playing，状态不明按取消处理。

## 已执行检查

| 命令 | 结果 |
|---|---|
| `cargo test -p nai-media-core` | Passed：lib 20 项 + gate 7 项 + timeline 集成 1 项 |
| `cargo xtask check --profile portable` | Passed：全 workspace 199 项 Rust 测试；11 包边界、fmt、Clippy `-D warnings` |
| `cargo xtask schema --check` | Passed：生成无漂移（本轮无 schema 变更） |
| `pnpm typecheck` / `pnpm build` | Passed：desktop 与 contracts-ts |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate d11-media-basics --strict --no-interactive` | Passed |

测试覆盖：seq/格式变化拒绝、end_of_stream 终块、时钟单调与静音连续性、跨采样率精确映射、
缓冲容量与会话水位、音频视频双水位提交与持续迟到、录制句间补静音与 PTS 单调、ACK 完成判定与矛盾/迟到 ACK 拒绝。

## 状态与边界

- **已实现/实测**：媒体时序内核共 28 项新增测试；全 workspace 199 项 Rust 测试与前端/生成检查通过。
- **Blocked**：Windows 独立窗口/音轨、虚拟摄像头/音频设备、真实媒体进程与录制文件装配；WSL 不验收设备。
- **未实现**：设备侧音量/声卡缓冲、P0 静音 to delay、OBS/伴侣采集与真实播放；这些属设备/宿主里程碑。
- **NotApplicable/未执行**：Windows 原生设备/SDK/GPU、T/E/U 完整认证；本轮时序内核实测不等于设备认证。

录制 PTS 连续语义已在内核测试，但真实声卡缓冲、抢占与平台上传仍需设备验收。
D11 活动变更已归档为 `openspec/changes/archive/2026-10-08-d11-media-basics/`，
主规格 `media-basics-timeline` 生效。