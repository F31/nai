# ADR 0133：隔离 keeper 的 Windows 设备演练

日期：2026-10-20。状态：Accepted。

## 决策

Windows 原生音频与窗口适配放在 `nai-output-keeper`，target-specific 锁定 `cpal=0.16.0`（WASAPI）与 `minifb=0.28.0`（原生像素窗口）。工程代码继续禁止 unsafe，不把 SDK 引入 kernel，也不依赖 Tauri 或插件。两个库及传递依赖增加构建/供应链维护成本，适用 Windows Local；只有显式设备入口打开资源，没有新常驻服务，第三方线程随 keeper 回收。

首个入口为 `--device-smoke <report.pb> <expire|mute|stall>`，只播放固定低音量测试音、显示原创测试图案。它是明确演练，不接受生产候选、API 凭据或观众内容，不具有生产外播权限；既有 headless drill 保持设备不可用。

复用 DeviceChunk 完整演练授权和回调撤权边界。WASAPI 请求缓冲大小不能视为实际回调大小；原生缓冲有上限，按采样位置拆分为 ≤10ms 门控片段。预计设备排队时间必须明确且有界，否则静音。P0 和期限在音频回调内复检，不依赖窗口更新线程。

报告采用源 Protobuf 生成 Rust/TS/C++，只记录设备打开、回调复制、窗口提交和静音观察。CPAL 的 playback 时间是预计交付时间，不是 DAC 已完成，所以 `actual_local_playback_completed` 恒 false。不产生 PlaybackAck、不提升 registry。

API依据：[CPAL 0.16 OutputStreamTimestamp](https://docs.rs/cpal/0.16.0/cpal/struct.OutputStreamTimestamp.html)、[minifb 0.28 Window](https://docs.rs/minifb/0.28.0/minifb/struct.Window.html)。以锁定版本源码和本机实际回调为准，不假定请求缓冲大小等于实际回调长度。

## 未覆盖

批准候选→原始媒体通道→keeper 的生产接线尚未实现。窗口主线程停顿期间画面可能冻结，当前不能宣称画面在600ms内独立待机；本轮 stall 仅验证音频静音。没有声学回环、真实直播工具或完整设备矩阵。原生初始化/窗口API本身可能阻塞，真实测试由父进程施加10秒强制回收期限；未来生产宿主必须保留进程级监督，不能直接依赖同线程取消。
