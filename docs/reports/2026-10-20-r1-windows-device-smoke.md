# Windows 原生设备演练（2026-10-20）

## 现在能做的事

独立 keeper 可在真实 Windows 上打开 WASAPI 默认音频输出与像素窗口，播放固定低音量测试音，观察租约到期、P0及主线程暂停时的音频清零。用户入口与复现见 [设备演练指南](../windows-device-smoke.md)。批准候选的生产播放仍未接通；没有把此演练作为 R1 首句或设备完成认证。

## 实现

- 先扩展 `schemas/domain/media.proto` 的 NativeDeviceSmokeReport，再通过 `cargo xtask schema` 生成 Rust/TS/C++，未手改生成文件。
- keeper 的 Windows-only cpal 0.16.0 / minifb 0.28.0，原生库不进入 kernel，不新增常驻服务或 Python/Node 运行依赖。成本和隔离边界记录 [ADR 0133](../adr/0133-native-device-smoke-boundary.md)。
- 只接受显式 expire/mute/stall 场景，使用明确 rehearsal 授权与固定测试音，不访问生产审批/凭据。既有 headless watchdog 的 output_available 仍为false。
- 复用 DeviceChunk 独立 P0、epoch 与固定600ms期限。原生缓冲≤100ms，内部分≤10ms段并计入采样偏移和设备预测排队时间；未知/超100ms预测时间拒绝并P0。错误回调也触发P0。
- 原生报告运行前先替换旧报告；非 Windows 显式 DEVICE_PLATFORM_UNAVAILABLE。实际测试通过父进程10秒期限隔离原生阻塞。

## 真实 Windows 结果

命令为指南的显式 ignored 原生集成测试；测试单元1项包含3个真实子进程。Windows 10.0.26200.0，MSVC x64，48kHz 双声道f32，原生 WASAPI/minifb。最终日志 `.nai-diagnostics/device-windows-smoke-final3.log`，报告 `.nai-diagnostics/windows-device-smoke-final/*.pb`，大产物不提交。

| 场景 | 实际回调 | 复制交错样本 | 停止后静音回调 | 停止后非零回调 | 窗口提交 |
| --- | ---: | ---: | ---: | ---: | ---: |
| expire | 114 | 26112 | 86 | 0 | 216 |
| mute | 145 | 11712 | 134 | 0 | 209 |
| stall | 146 | 55872 | 86 | 0 | 70 |

三场景 max_callback_frames=1056、最大预计排队22ms。最后非零回调分别在579038/115604/569651微秒，P0场景120486微秒触发静音，其余600000微秒期限；这些是回调时刻，不能解读成硬件发声/静音时间。三个报告 actual_local_playback_completed=false。窗口是API成功提交证据，无人工观看或声学回环验收。

## 发现并修复

首次设备已打开，但请求240帧并未限制实际1056帧回调，10ms单块门控拒绝，报告 DEVICE_CALLBACK_TOO_LARGE，没有冒充通过。修为有界原生缓冲内按偏移分片，并新增期限跨片测试。

第二次P0测试在慢启动首次回调前就静音，因此复制数0而失败；改为以首次实际非零回调为锚点100ms后触发P0，避免把没有发声当成取消成功。三次 LNK1104 短暂锁定测试exe，随后重试成功；首轮核对没有残留进程，未终止无关进程或放宽链接检查。

最终复测又发现设备初始化可以耗尽预先签发的600ms租约。改为先创建流，后签发一次性授权，经容量1内部媒体通道交给回调；不是延长租约或对超时自动续期。另补整个原生缓冲提交前的撤权复检，P0在分片间发生也清除之前复制的前缀。

## 工程验证

- Windows `cargo clippy -p nai-output-keeper --all-targets --locked -- -D warnings`：最终实现通过；`--lib` 软件测试10/10通过。
- WSL `cargo test -p nai-output-keeper --lib --offline`：最终软件测试11/11通过。Windows设备测试另行显式执行，Linux未运行它，不计为通过。
- 新增回归覆盖原生缓冲分片的采样偏移、跨期限只保留有效片段、P0清零整个缓冲、未知/超限预测时间与超大缓冲拒绝、非Windows旧成功报告失效。
- `cargo xtask schema` 实际生成完成，最终三个设备报告由生成的Rust类型解码；初轮报告亦用独立protoc从源协议读回。
- `cargo xtask check --profile full`：EXIT=0，106个Rust测试目标、1222 passed / 0 failed；Node 451 passed / 0 failed。workspace fmt/clippy及前端typecheck/build通过；不等于设备/SDK/GPU/安装认证。
- `cargo xtask schema --check`：无漂移；`cargo xtask test --suite contracts`：EXIT=0，Rust/TS/C++编译、schema/API校验与14类消息双向互通通过。设备报告专门验证64位整数不丢精度、失败状态和非设备完成语义；新增fixture的clippy通过。
- OpenSpec 归档 `2026-10-20-r1-windows-device-smoke`；归档后主规格43/43、归档163/163，合并/归档完整性回归8/8通过。

## 未完成

批准候选的生产接线/原始媒体传输、音画时轴与端到端窗口显示、画面独立失联待机、设备实际完成ACK、真实直播工具及设备矩阵仍未实现或未实测。付费Provider和正式签名不在本轮。registry验收状态未改，安装器未重建，改动未提交。
