# 2026-11-09 Rust常驻增量主控与真实TTS输入

## 实现

新增Rust v561_avatar_stream::Resident，装配协议2、私有spawn密钥、独立raw Sender和既有只读Frame Ingress。与完整源诊断共用单一进程环境构造，清空继承环境后只放行原生工具链，缓存仍为E盘。独立维护Ready<=240秒；真实任务仍<=45秒，不用维护预算放宽业务。Ready复检真实预热样本/图/kernel/块几何，事件复检HMAC、全进程序号、portrait、task/epoch和类型闭包。

帧上限先由max_samples限制，再按实际交付PCM复检源样本起点；只有EOS后允许最后不足960样本的一帧。逐帧通过独立只读helper读取并复核摘要、池/槽代次，然后才发签名Release。成功EOS与RenderComplete分别复检累计实际样本/hash、Ingress计数和共享工作量/补尾/耗时校验器。中途输入不是完成源或播放许可。

取消先撤Task Operation，再以独立4秒清理等待签名渲染取消；迟到帧只认证而不读取、呈现或释放。只有CANCELLED且完整raw仍可用时清退在途槽，全连接payload ID不回退；RAW_RESTART_REQUIRED或未知终止关闭worker，不复用损坏通道。更高task/epoch可复用同一模型。

新增v561_stream_synthesis应用装配：一次CosyVoice请求逐规范PCM块交owner，适配器完成回执验证成功后才提交同源EOS；失败撤销临时任务、不重试或保留部分成功。该应用层负责供应商编排，GPU owner不直接依赖供应商实现。

## 定向验证

- Rust新增3项：Ready实际证据与独立维护边界；旧版/序号/身份/负载类型拒绝；真实PCM前沿与EOS短帧。
- 相邻完整源owner6项、媒体工作量3项及显式Windows维护取消/超时故障1项，共13项Rust测试通过。Python增量状态9项通过，共22项定向检查。
- 两个诊断二进制最终构建及clippy -D warnings通过。初次维护故障测试误用了另一个测试的环境变量，纠正NAI_AVATAR_TEST_PYTHON后通过；clippy提示的布尔表达式与诊断闭包作用域已修正。仅余既有E盘增量缓存硬链接复制回退工具链警告。
- 协议源未变，不重复无关全量生成/工作区测试。不以历史V48注册表作为门槛。

## 真实Rust→GPU验收

一次真实模型加载，维护Ready61265ms；原有授权PCM92160样本/3.84秒，不调用TTS。实际走Rust签名控制→独立raw→Python GPU→共享帧→独立只读helper→Rust摘要/时轴/Release：

| 任务 | 实际读取帧 | EOS前帧 | 首验证帧 | 总时长 |
| --- | ---: | ---: | ---: | ---: |
| 完整 |96|40|1797ms|5393ms|
| 第10帧取消 |10|不作完整证据|1751ms|3333ms|
| 取消后恢复 |96|40|1745ms|5219ms|

两个成功任务96帧均与独立固定seed参考逐帧完全一致。第10帧读取后取消，不把其未释放状态冒充完整回执；取消后owner退槽并使用不回退的payload ID，复用同一模型恢复。3333ms是任务总时长，不是停止延迟。

证据：target-windows/v561-rust-stream-owner/real-01/report.json、worker.log与real-01.log。

## 一次真实本地CosyVoice→Rust→GPU

服务健康且空闲后，只调用一次已保存本地配置，固定短句“你好，这是数字人直播。”，无自动重试。维护Ready62171ms独立记录，不算下列业务时长：

- 供应商完成回执通过，65280实际源样本（2.72秒）。
- Root实际读取68帧、worker完整68帧；80候选/12尾帧裁弃，43520有效特征/7680补零，工作量方程通过。
- 首PCM33647ms；供应商适配器完成35930ms；首验证帧36017ms；业务完成37868ms。
- GPU compute3297ms、transfer64ms、delivery639ms、overlap375ms。供应商complete计时包含同步消费回调的背压，不能当纯供应商推理耗时；首PCM发生在回调之前，因此其长等待不能归因于本次帧消费回调。

这证明真实供应商完成证据与Rust/GPU完成贯通，不证明低延迟，尚无提前播出收益。更正：首帧与适配器回执使用不同计时起点，原先据36017/35930推断首帧晚于供应商回执并不严谨，撤回该时序结论，原始数字保留。共同常驻条件下首PCM显著慢，本轮未采集同一请求的阶段/显存峰值，不能唯一归因于显存、GPU争用或服务排队。后续阶段证据见2026-11-09-v561-tts-first-pcm-stages.md。

证据：target-windows/v561-rust-stream-owner/tts-01/report.json、worker.log及tts-01.log。本轮2个模型进程（原有PCM验收、真实TTS验收各一次加载），1次本地TTS API调用、0 LLM调用，费用unknown（本地）；无设备播放、麦克风、录制、推流或安装版更新。

## 后续优先级

1. 首PCM阶段定位：同一请求核对服务准入/首包时间、共享GPU显存/RAM与推理阶段，先解决33.65秒等待；保持单TTS和单视觉模型，不引入第二实例或自动重试。
2. 将已验证的Rust输入/帧接入动态音频设备时钟，依据实际生成水位开放PCM，保证同epoch、P0、短租约和有界缓冲；输入EOS与设备播放完成保持分离。
3. 真实可听音画及取消后恢复通过后，再接GUI/安装版与OBS验收。

仍未完成设备音画、新参考人工画质、持续实时、OS PID/ACL及长稳发布组合。当前约5.2秒生成3.84秒源和真实TTS长首包都不满足直播体验；34ACC pending、M0 No-Go不变。
