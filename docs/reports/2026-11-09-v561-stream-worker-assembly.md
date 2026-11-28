# 2026-11-09 常驻增量worker装配

## 实现

avatarstream协议2正式生成Rust/Python/TS/C++：新增真实Ready、共享Frame/mapping、Release、RenderComplete、RenderCancelled、Stopped与stats。旧增量版本1拒绝；既有完整源诊断worker6不隐式支持新协议。输入EOS、渲染完成、设备播放是三个独立阶段。

新增flashhead_stream_worker.py，真实模型加载/编译预热后Ready，私有密钥互证raw连接建立后接受签名任务。控制线程推进全进程序号与任务floor，GPU主线程消费有界队列/独立PCM并维持一个连续GPU会话。逐帧共享槽需要签名Release，任务完成后复用同一模型接受更高task/epoch。控制取消先置标记，GPU清理后才发RenderCancelled；业务45秒、维护240秒上限独立校验。

取消未消费完整的raw包时标记通道污染，即使GPU尚未开始读包也终止连接/worker，防止残留跨任务。完整raw已消费后的GPU取消可在同进程干净恢复。没有新增第二视觉引擎、并发TTS、数据库迁移或GUI兼容层。

## 失败与修复

real-01首次原生整链在最终一致性断言失败。real-02诊断启动脚本误改参考文件名，在模型启动前失败；修正路径后real-03保留了具体结果：92160样本/源摘要、96有效帧、96实际像素摘要校验、EOS均正确，但与上一进程参考的逐帧hash为0/96匹配。

源码确认LTX头像encode使用posterior.sample()；原代码在头像缓存生成之后才固定全局Torch随机源。补充在get_base_data之前按配置seed绑定全局RNG，并通过顺序测试防止回归。不更改768分辨率、49帧/9帧motion、四步、face_ratio或模型权重。重新建立seeded-reference后，独立进程real-04完整任务及恢复各96/96帧与参考完全相同，原失败和原参考没有覆盖。

## 验证

- Python状态9项（包括旧版本拒绝、EOS后Release不释放raw槽）、GPU会话CPU替身7项（含头像seed先于cache）、启动预算2项。
- Node描述符及相邻3项、Rust/Python签名往返1项、升级后真实Rust→Python原始通道3项。共25项定向检查通过；固定生成器315消息/19源无漂移，对应Rust契约测试clippy通过。未跑无关全工作区测试。
- 真实GPU：新seeded-reference在单模型内完成整段96→连续96→第10帧取消→恢复96，完整像素摘要一致。之后独立真实worker进程用签名protobuf stdin/stdout、随机key/nonce互证TCP raw和共享像素，Ready为65094ms（维护阶段），模型只加载一次，结果如下。

| 独立worker任务 | 读取帧/Release | EOS时已读帧 | 首帧 | 任务耗时 |
| --- | ---: | ---: | ---: | ---: |
| 完整生成 |96/96|40|1875ms|5734ms|
| GPU期间取消 |10/9|无EOS|1860ms|3563ms|
| 取消后恢复 |96/96|40|1687ms|5218ms|
| 半包取消 |0/0|无EOS|无帧|32ms渲染取消回执|

第10帧读取后取消，不发送其Release，因此不称10帧已确认消费。GPU取消的3563ms是任务总时长，不是停止延迟。半包32ms仅指无帧任务的worker取消回执，不代表硬件静音。完整任务和恢复分别核对实际共享像素、逐帧序号/epoch/样本时轴/槽代次、累计源摘要、EOS及渲染stats；均与独立新参考96帧完全相同。

证据：target-windows/v561-stream-worker-assembly/real-04.json、real-04.log、real-04.worker.log；seeded-reference.json/.log；real-01/02/03失败日志及real-03.failure.json。最后成功worker单次模型加载；本轮包含失败复核、参考重建和成功worker，共启动4个真实模型进程，没有把全轮说成一次加载。所有验收源为已有授权PCM，0模型API调用，无麦克风、播放、录制、推流或安装版更新。

## 边界与下一任务

本轮父进程是明确的Python原生诊断，不是Rust engine。Rust→Python真实raw已经单独验过，但Rust新Resident尚未装配协议2/Ready/frame ingress，也未接真实CosyVoice或音频设备时钟。不得把此结果称为Rust音画整链或直播完成。

源在诊断开始时已完整可用，EOS前40帧不等于真实网络首声提速；生成仍需约5.2–5.7秒/3.84秒，未达持续实时。新固定初始化参考尚未补人工画质评价，不自动继承旧随机初始化片段的人工接受结论。

下一纵向任务是Rust常驻owner：复核协议2/维护Ready、私有启动/失败回收、单槽raw消费与渲染取消后槽清退、独立只读frame ingress和Release，再接真实CosyVoice及音频设备时钟。损坏/半包通道只允许显式重新准备，不自动重放任务或旧帧。34ACC pending、M0 No-Go不变。
