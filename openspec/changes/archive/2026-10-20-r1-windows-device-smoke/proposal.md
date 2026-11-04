# Proposal

## Why

keeper 尚无原生设备后端，文件预览不能证明能打开音频设备或显示画面。需要明确演练入口验证实际设备调用及媒体侧静音。

## What Changes

- Windows keeper 增加显式 `--device-smoke <report.pb> <expire|mute|stall>`：默认输出设备、短低音量测试音和独立像素窗口。
- 复用 DeviceChunk，测试 P0、600ms 失联期限；报告真实回调/窗口提交，不合成 PlaybackAck。
- 非 Windows 显式 unavailable；演练不读取候选、凭据或生产媒体。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `media-basics-timeline`: 原生设备演练与事实报告。

## Impact

新增 Protobuf 报告并先生成 Rust/TS/C++ 契约；Windows-only CPAL/WASAPI 与 minifb 依赖仅在隔离 keeper。无常驻服务。生产候选接线、真实设备完成与发布验收仍未完成。
