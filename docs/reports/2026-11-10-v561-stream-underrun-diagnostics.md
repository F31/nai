# 播放欠载诊断：工程与原生演练通过，真实性能修复未完成

## 当前事实

更新后的实际工作台session-1794319111278629700已准备并进入play-2。该次业务播放失败：sentence-1/failure.json记录shown_frames=50、audio_code=WORKBENCH_ERROR_IO_FAILED，执行器报告NAI_STREAM_DEVICE_UNDERRUN。script.json的completed_sentences=0/results为空，只表示没有完成的句子，不可据此称从未显示画面或没有播放样本；现有失败数据也不能证明实际可听声或同步通过。共1次本地模型API调用来自操作者本次讲解，不是本轮新增调用。

Buffer::take同时受队列和play_until授权上界限制，旧标签不足以区分PCM供应慢与GPU帧门控慢，因此未直接更换TTS或增大缓存。此前三句原生完整播放仍有2.7–3.7秒句间等待，性能门槛保持失败。

## 实现

媒体核心新增纯诊断分类，按同次请求区间、received和play_until分PCM_SUPPLY、VIDEO_GATE、PCM_AND_VIDEO_GATE、NO_SOURCE_SHORTFALL及无效范围。适配器在同一次拉取使用单一授权快照，失败日志包含请求/实际样本、源起点、已收到、授权上界、提交/播放计数；不包含正文、原始媒体或凭据。

不改变validate_device_pull决策、句尾校验、租约、PCM容量、预缓冲、模型画质或截止期限。诊断不授予输出，不补静音掩盖真实句内短读，不据旧UNDERRUN归因唯一瓶颈。无协议字段变化。

## 验证

- 媒体核心stream_pcm相关7项通过，覆盖新的分类/溢出以及真实句尾等待、未封口短读拒绝、样本hash/容量/时轴。首次cargo test命令因已有目标exe的LNK1104失败，随后独立具名lib测试产物直接运行7项通过，未绕过失败断言。
- Windows适配器clocked-experiment lib clippy通过，执行器候选构建通过。首次误传不存在的应用feature构建失败；使用应用已声明的依赖feature正常构建。
- 无声Windows原生故障演练通过：使用全零PCM，故意供给9600样本后不EOS，约603ms结束，观察到8645样本设备进度；日志明确reason=PCM_AND_VIDEO_GATE/requested=240/actual=0/source_start=9600/received=9600/play_until=9600。证明诊断接线和已有失败退出，不证明业务缺陷修复。
- 产物target-windows/v561-underrun-diagnostics/underrun-native.json及underrun-native.log。候选音频程序同目录，Root/协议未改。0次新增LLM/TTS调用、无GPU加载、麦克风采集或推流；设备仅提交全零样本。

## 下一步

工作台仍可能持有模型会话，已请求操作者保存退出。退出后更新音频执行器，保留原错误及维护备份；用单一GPU实例和本地TTS最多一次短业务复验，收集真实欠载计数。若视频门控不足优先测GPU块持续吞吐，若PCM不足优先测供应商首包/分块间隔；共同不足不作单因结论。U03及整体连续输出性能仍未完成，OpenSpec活动，34ACC/M0不变。

## 真实单句续验

操作者确认退出，健康检查CosyVoice status=ok/busy=false，无其他工作台模型进程。新增正常生产单句诊断测试，最多一次模型调用，不重试。使用候选音频程序及正常Root，real-one-sentence-01通过（总82.00秒含准备），实际1次本地TTS、62400样本全播、65帧校验/64次画面提交、同设备PID11332/worker52344、最大时钟年龄28ms。首声6223ms，完整源6013ms、GPU结束7517ms、设备结束8820ms。无新增欠载，之前失败未被重现，因此不能宣称故障根因已修复或稳定性能通过。

单句视频生成开始1194ms至结束7517ms，包含供应商等待及媒体传输，不可将6323ms全部计为GPU纯计算。设备原始PCM和视频覆盖共用门控，诊断中的共同不足是观测事实，不是TTS与GPU因果独立定位；后续结合源/GPU阶段数据再优化。当前首声仍约6.2秒，三句句间空窗的原验收失败保留。无付费供应商、无推流/麦克风采集。

本轮实际一次本地模型调用，0次自动重试。维护更新开发运行目录音频程序，备份及SHA256核对通过，journal target-windows/maintenance/underrun-diagnostics-20261110-221821/journal.json为activated。工作台PID29832重开；Root、配置、素材和旧开始菜单安装不变。原生单句和维护交付完成，U03b/三句持续性能仍未完成，不归档。
