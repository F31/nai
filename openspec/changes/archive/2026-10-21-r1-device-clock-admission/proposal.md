## Why
V3.11 W05/W10 的实际完成证据缺失。CPAL 回调时间是预计时间，必须先验证原生 IAudioClock 位置，不能直接生成 PlaybackAck。
## What Changes
新增独立设备时钟游标及 Windows 显式无声设备测试，验证频率换算、单调性、已提交上界和停止行为。生产输出暂保持现状。
## Impact
仅 media-core 纯算法和 output-keeper 测试；wasapi 0.23.0 为 Windows 开发依赖，无新增常驻服务，无 API 费用，无契约变更。验收映射 R07/A01 只补工程证据，不升级认证。
