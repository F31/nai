# 常驻形象 worker：工程接入与启动失败记录

状态：工程组件已实现；真实模型两任务复用未通过；OpenSpec 保持活动，不归档。

新增 avatar_worker.proto、avatar-worker JSON Schema，并用正式生成器生成 Rust/TS/C++/Python 契约。Rust owner 通过私有 stdio 和 HMAC 校验事件、序号、任务/epoch、PCM 摘要及参考形象摘要；控制消息上限16KiB，写队列2、事件队列8。提供独立 nai-v561-avatar-probe，接收已有24kHz单声道 int16 PCM，明确不授公共播放。

Python backend 实现模型和参考形象一次加载，每任务重置运动/音频历史和随机生成器，实际 PCM 摘要/长度校验后制作不可变快照；帧块有界、尾部零填充并裁到原始样本时长。采用现有三槽诊断帧接收器，尚非正式认证媒体池。取消标记与 GPU 推理解耦，GPU 返回后检查取消；Rust 不接受过期完成结果，并提供等待实际清理的方法。真实 GPU 取消、相邻任务状态隔离与公共输出撤权尚未实测，不能据实现宣称通过。

验证：3项 Python 控制/PCM 测试、2项 Rust 截断/超大控制消息测试通过；Rust probe 构建、定向 clippy、OpenSpec strict 校验通过。正式契约生成307消息/18个protobuf源完成；首次沙箱调用 protoc 系统错误，沙箱外重跑成功。未运行全量测试。

真实验证使用 docs/jjf.jpg（已有授权）、768×768、49帧块、现有1.92秒PCM；不调用TTS或付费API，不开麦、不播放、不推流。real-01 和增加阶段日志后的 real-02 均在45秒就绪期限到期退出，未收到Ready或完成任务；real-02日志只到 importing runtime。当前只能定位为运行环境导入阶段未完成，不能归因GPU推理速度、网络、显存或确定某个依赖。失败工件位于 target-windows/v561-resident-avatar，未自动重启或延长业务期限。

下一步：分阶段定位 numpy/torch/upstream 导入耗时，恢复有界启动后验证同一进程两任务及GPU取消，再将同源PCM与真实音频时钟接入正式帧池、公共窗口。此次不更新现有GUI运行组合，不升级34项ACC或M0。
