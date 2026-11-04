# Windows 音频与窗口演练

当前可用：独立 `nai-output-keeper` 能打开默认 Windows 音频设备与 320×180 像素窗口，输出短促、低音量的 440Hz 测试音，并独立验证到期/P0 静音。窗口标题明确标记 DEVICE SMOKE，关闭窗口或 Esc 会撤权。

这不是批准候选的播放按钮，不读取生成的 WAV、API 密钥或观众数据。生产媒体通道、审批续租接线与播放完成反馈仍待实现。旧 NSIS 安装器没有重新生成，使用本轮构建的 keeper。

## 原生有界测试

在已配置项目 MSVC/Rust 环境的 Windows PowerShell 中，从仓库根执行：

```powershell
$env:CARGO_TARGET_DIR = 'E:/projects/nai/target-windows'
$env:NAI_DEVICE_SMOKE_ARTIFACTS = 'E:/projects/nai/.nai-diagnostics/windows-device-smoke'
cargo test -p nai-output-keeper --test native_device --locked -- --ignored --nocapture
```

这是显式真机测试，正常工程测试不自动播放测试音。父进程逐个启动 expire/mute/stall 三个进程，每个最多10秒，超时只终止自己的子进程。分别输出 `expire.pb`、`mute.pb`、`stall.pb`，由生成的 `NativeDeviceSmokeReport` 解码并断言。没有默认设备、非48kHz f32 单/双声道、不可用时间戳或原生错误均失败，不替换为软件假设备。

keeper 原始入口为：

```powershell
./target-windows/debug/nai-output-keeper.exe --device-smoke .nai-diagnostics/device-report.pb expire
```

该直接入口没有父进程强杀兜底；涉及原生初始化时优先使用上面的有界测试。文件写入失败返回2；其他失败也返回2，报告包含稳定原因码。运行前先用未成功报告替换旧报告。

## 报告解释

- `audio_opened/window_opened`：真实设备流与窗口已经创建。
- `callbacks/copied_interleaved_samples/window_updates`：真实回调复制与窗口提交次数，不是实际播放完成。
- `max_callback_frames`：本机实测1056帧；不能假设等于请求的240帧。回调有100ms总上限，内部按≤10ms分片，每片根据采样偏移加上设备预计排队时间复检。
- `silent_callbacks_after_stop/nonzero_callbacks_after_stop`：停止之后回调输出的观测。P0直接触发独立原子信号；失联使用固定600ms单调期限，不需要主线程轮询。
- `actual_local_playback_completed`：恒false。CPAL预测交付时间不能证明DAC完成，没有生成PlaybackAck或声学回环证据。

stall 暂停窗口线程900ms，只验证音频能够独立静音。暂停期间窗口可能冻结，恢复后提交黑色待机画面；不能据此声称画面满足600ms独立待机。参见 [ADR 0133](adr/0133-native-device-smoke-boundary.md)。
