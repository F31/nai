# 已批准候选的本机播放

2026-10-21尾部证据更新：已替换复制完成后的固定220ms等待，改为后续真实音频回调越过预计尾部且授权仍有效，才记录完整callback_drained_samples。宿主核对本次media_sha256、独立重采样预期数、复制数与排空数，再保存playback.pb。CPAL预计时间不是实际设备进度，仍保留submitted与actual_local_playback_completed=false，不自动推进节目。见 [ADR0142](adr/0142-callback-tail-observation.md) 和 [报告](reports/2026-10-21-r1-callback-drain-evidence.md)。

直播台新增“准备直播输出（不调用API）”：连接后显式打开当前已准入形象的合法闭嘴窗口与静音音频设备，不调用LLM/TTS、不生成/批准候选。任务ACK不是就绪，界面等待真实output_ready与待机摘要后才显示本次确认；后续正常审批播放复用该进程与stream。设备忙、素材/设备缺失、会话换代或超时如实失败，不自动重试。关闭输出窗口后需重新准备。本轮不更新现有安装文件，详见 [报告](reports/2026-10-21-r1-output-prewarm.md) 与 [ADR0139](adr/0139-neutral-output-prewarm.md)。

现在可从桌面完成：连接已配置真实API与2D素材的engine → 手工提问 → 审阅最终文本并两次确认批准 → 点击“合成并播放到本机默认设备（15%音量，最长20秒，常驻输出窗口）”。批准有效30秒，候选自身期限仍生效。设备输出支持Windows人工辅助模式；缺后端在TTS前拒绝。API与素材仍需真实配置，默认演示端点不是可用付费服务。

“生成语音与2D预览（不播放）”仍只写文件。两个动作是分别显式请求，分别最多一次合成；先预览再播放会有第二次TTS调用，重复同模式命令不会再次收费或自动重播。播放超过20秒的句段会失败，不截成一句不完整语音。

同一引擎内从首次播放开始复用 `nai Live Output` 窗口，首个完整授权后句间显示合法闭嘴静音待机；每句仍需独立批准。播放中“立即静音（控制撤权）”与“人工接管”始终可点击；它换代并使引擎停止当前输出。keeper独立执行短租约与EOF静音；已准入的中性图可在父进程EOF后继续待机，至本地关闭或会话上限。撤权后确认静音待机则保留窗口，750ms内未确认则回收子进程，不自动重播。成功界面只说“已提交本机默认设备”，不说硬件播放完成；没有生成PlaybackAck。失败不自动重发，需查看原因并重新生成候选。

## 运行边界

- engine与keeper须使用同一次构建的同目录侧车。engine只从自己的目录寻找keeper，不接受HTTP传入的程序或媒体路径。
- 输出为默认WASAPI设备，48kHz f32单/双声道；内部对TTS s16单声道有界重采样。固定15%增益，实际音量仍受操作系统设置影响。
- 独立窗口显示已准入素材渲染的真实帧；演示/测试素材尺寸可能很小，这不代表完成形象质量或音画延迟认证。
- 临时PCM/RGBA最多20秒，v2逐帧文件每句512MiB、单帧16MiB（双上限同时生效），旧v1仍限整句64MiB，正常完成或控制撤权后删除；强杀遗留不会被复用或自动播放。成功保留 `operator-artifacts/<候选>-<epoch>.playback.pb`，仅为设备提交观测。
- v2描述符的逐帧摘要与中性图摘要由私有grant绑定；展示前seek/read/hash，只保留有限帧。全文件摘要为producer离线诊断。上层每100ms复检审批、事实和代次并刷新≤600ms租约；输出回调不等网络/存储/模型锁。计数缺省表示未知，不等于零。

常驻模式当前最多30分钟；窗口/PID及WASAPI stream复用，静音时持续输出零。首次明确播放才打开输出，首次完整授权前没有中性图。尚无独立预热UI、待机微动作、连续讲品调度、麦克风音轨或OBS实录，不能当作整场直播已验收。见 [最新报告](reports/2026-10-21-r1-continuous-device-standby.md)。

## 工程复现

Windows/MSVC构建环境，在仓库根：

```powershell
$env:CARGO_TARGET_DIR = 'E:/projects/nai/target-windows'
$env:NAI_PLAYBACK_ARTIFACTS = 'E:/projects/nai/.nai-diagnostics/approved-local-playback'
cargo build -p nai-output-keeper --locked
cargo test -p nai-engine --test engine approved_local_playback_reaches_device_and_mute_revokes_inflight_output --locked -- --ignored --nocapture
cargo test -p nai-output-keeper --test local_playback --locked -- --ignored --nocapture
```

显式测试会使用真实默认音频设备与窗口；Provider为本地协议fixture，不消耗付费API，不能计为真实TTS认证。第一项验证实际存储批准、文件预览后播放、同一文本、完整样本提交、幂等、输出后静音和临时文件回收；第二项验证篡改、EOF和停止续租。无需为测试提供密钥。

参见 [ADR 0134](adr/0134-approved-local-playback-boundary.md) 与 [本轮报告](reports/2026-10-20-r1-approved-local-playback.md)。

### 设备时钟准入验证（2026-10-21）

Windows可显式执行 `cargo test -p nai-output-keeper --test device_clock --offline -- --ignored --nocapture`。它只向默认设备提交零样本，直接读取 IAudioClock position/frequency，并验证停止后位置冻结；无Provider调用。默认测试不会占用设备。此验证未替换生产CPAL后端，不生成候选完成ACK。实测和下一步边界见 [报告](reports/2026-10-21-r1-device-clock-admission.md)。

### 常驻设备实际音频进度（2026-10-21）

生产常驻输出现使用安全WASAPI事件线程和IAudioClock，保留单stream跨句/中性待机。完整音频尾部状态经本次描述符、实际帧数、epoch/撤权/期限和宿主末次授权复检，保存在playback.pb；旧预计回调字段不授予成功。桌面显示实际音频到尾部，完整音画仍需人工确认。没有自动完成ACK/节目推进或OBS验收，详见 [报告](reports/2026-10-21-r1-wasapi-resident-progress.md)。
