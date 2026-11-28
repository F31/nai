# 2026-11-08 连续形象输入契约与消费状态

## 实现

新增schemas/v561/avatar_stream.proto及Rust/Python/TS/C++正式生成产物、Rust模块导出。新入口avatarstream.v1仅为增量契约；既有worker6仍保持原能力，不把增加DTO当作生产Ready。控制消息没有PCM，Chunk仅传原始payload ID、源起点、样本量和SHA256。Begin绑定portrait、已验证块几何、24000Hz和最多144000源样本。每块最多38400样本，控制最大16384字节。

scripts/avatar_stream_state.py消费状态机检查确定性protobuf HMAC-SHA256、version1、严格逐条序号、task/epoch及floor、portrait/格式、raw身份/实际长度/hash和累计偏移。只允许一个在途块；只有本地consumed可释放并生成签名回执，取消不等待消费。任何校验/父取消错误终结并invalidate；EOS要求无在途、实际累计样本/hash及Root已验证provider_eos，返回EOS_VERIFIED而非播放完成。模块本身不认证原始OS管道或供应商权重。

## 验证

- Python7项：完整消费与签名ack；在途取消；签名/序号/epoch/portrait/几何/格式/floor错误；块偏移/长度/摘要/raw ID/单槽/task/限额/错误release；EOS单槽/供应商/计数/hash；父取消；签名块→单连续重采样实例数值对照。
- Rust1项真实子进程互通：Rust生成3条带HMAC的protobuf Begin/Chunk/EOS→Python解析及消费→Python签名EOS→Rust反序列化及HMAC、身份、计数、摘要复检。公开固定测试key仅用于fixture，不是用户密钥。原始PCM由fixture明确提供，不能冒充原始IPC或GPU通过。
- Node3项描述符/相邻检查：控制只带引用、Begin/Chunk/EOS结构；既有worker版本6和维护/业务范围不变。
- cargo check -p nai-workbench-engine --lib --offline通过；固定生成器315条protobuf消息、19源Rust/Python/TS/C++生成无漂移。C++只生成，未编译或执行C++运行时。
- 首次沙箱protoc启动因Windows DLL重定位错误失败；原生同一固定protoc生成及无漂移检查通过。不将首次失败当契约问题，不删除旧证据。

共11项定向检查，无全workspace测试，无API/GPU调用、播放、录制或推流。证据target-windows/v561-avatar-stream-contract/rust-requests.pb及python-eos.pb（公开fixture）。没有更新安装版、数据库、Key或用户配置。

## 后续

下一任务实现实际独立raw PCM传输与消费ack（OS身份/生命周期/背压/取消），将当前状态机接入worker单任务循环，持续使用同一重采样器和motion/history；补未知总长度下的帧索引及EOS尾帧裁剪。再验证真实CosyVoice满块→GPU首帧→设备时钟音画。当前仍无该入口的生产握手、raw IPC或GPU证据，不授新能力Ready。34ACC pending、M0 No-Go保持。
