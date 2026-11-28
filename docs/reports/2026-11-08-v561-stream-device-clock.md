# 分句流式设备时钟与帧调度出口

状态：新分句语音链路的认证设备时钟、有限观察出口及Windows帧调度接线已实现并实测。不是FlashHead真实帧/公共窗口/OBS或M0验收；34ACC不提升。

OpenSpec已归档为[2026-11-08-v561-stream-device-clock](../../openspec/changes/archive/2026-11-08-v561-stream-device-clock/tasks.md)，主规格为[stream-device-clock](../../openspec/specs/stream-device-clock/spec.md)。

## 改动

streamaudio控制/回执版本升为2，使用生成的WireVersion枚举；回执强制携带PlaybackClockObservation和设备观察年龄，整体进入HMAC。缺失、旧版本、epoch/设备代次/序号/样本不符、倒退、状态不符均拒绝，没有旧协议回退。

WASAPI线程发布同源源样本与实际轮询时间快照，设备代次每次open递增并固定于该执行实例。媒体线程只短暂try_lock更新，不跨设备、网络或模型调用持锁；控制线程复制快照。控制应答次数不能重新刷新设备观察的真实年龄。

根端先验签，再复检时钟与回执、单调性及范围。复用ClockBridge，新增保守观察时间入口：运行时钟以根请求发起时间减设备报告年龄作为下界，200ms新鲜度包含往返和设备年龄。准备只给Unavailable，完成/停止给Stopped；墙钟不推进源样本，设备卡住不能靠新回执继续伪报运行。

观察通道固定容量2且发布不阻塞续租。Running满时可丢中间观察，准备/终态不能伪报交付成功；消费者断开使当前观察装配失败。ClockEvent保留当前Operation与期限，过期或父取消后running()不给运行时钟，元数据本身不授帧访问或播放许可。

## 实测

- 纯StreamClock 3项：缺时钟/旧版本、身份/序号/范围、状态、设备年龄与慢往返、失败后不可恢复；共享ClockBridge相邻3项通过。
- 实际传输验签函数1项：改嵌套clock、设备代次、played_sample或age均使原MAC失效；观察通道/过期取消2项通过。
- Windows真实静音设备1项：两句跨句观察驱动有界Scheduler，4槽/2×2合成像素帧在前后两句都被实际样本选中，结束转安全状态。这只是调度证据，不是FlashHead画质、窗口提交或25fps认证。
- Windows消费者断连1项、既有音频版本/MAC/epoch/租约/半包/断流4项、宿主停止/迟到/关闭4项通过。
- 对应包clippy、正式契约生成无漂移、依赖边界及OpenSpec严格校验完成；只执行相关测试，不运行旧V48全量门槛。

本地CosyVoice实际联测2句、2次合成，无自动重试，不调用付费供应商、不推流。报告位于`target-windows/v561-stream-clock-acceptance/real-01/report.json`。

| 指标 | 实测 |
|---|---:|
| 实际消费 / 收到源样本 | 143040 / 143040 |
| 源音频时长 | 5.96秒 |
| 认证观察 / 交付观察 | 656 / 656 |
| Running观察 | 654 |
| 最大保守观察年龄 | 39ms |
| 首PCM / 首设备源样本 | 3958 / 4098ms |
| 最后HTTP EOF / 整段完成 | 8295 / 11336ms |
| 预取与当前播放重叠 | 1次 |
| Stopped终态 / 末次观察样本 | 已收到 / 143040 |

本轮一个epoch、一个设备代次，语音应用EOS正常。最大年龄是根端保守新鲜度指标，不是声画同步偏差或物理显示/声音延迟。模型合成和完整端到端等待未因时钟接线而变成实时认证；无人工听感结论。

## 运行与后续

新组合独立构建在`E:/projects/nai/target-windows/v561-clock-build/debug`，包含匹配协议2的nai-v561-cosyvoice-stream、nai-v561-audition-control、nai-v561-stream-audio-output。没有拆开覆盖现有v561-owner-late运行组合，没有更新已打开GUI或旧安装、数据库、Key、音色和模型。

下一步将同源PCM交给受控常驻FlashHead worker，绑定任务/素材/帧pool身份，并让真实窗口消费该时钟及其新鲜度；随后做实际音画和OBS。当前PCM交付、生产帧认证、常驻公共执行门、实际画面、实时速度及长稳仍未完成，不能用此处合成像素测试替代。源分句流程仍限单次45秒和最多20秒音频。
