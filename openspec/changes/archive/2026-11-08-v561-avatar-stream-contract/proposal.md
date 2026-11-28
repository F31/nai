# Proposal

## Why
现有worker协议6只接受完整PCM路径，不能表达未知总长度的连续输入。前两轮已验证有界TTS块和连续滤波，但直接连入旧render会重置motion。需要独立定义下一执行入口的签名增量契约与消费状态机，先验证取消、乱序和EOS边界，再装配实际原始通道及GPU。

## What Changes
新增avatar_stream.proto并生成Rust/Python/TS/C++；实现消费端签名、任务身份、序号、单块在途、样本偏移/摘要及EOS校验。PCM仅走独立原始通道，控制仅有引用/长度/摘要。预备契约不加入协议6 action、不自动启用不存在的worker能力、不提供兼容层。

## Impact
新契约和独立消费状态机/跨语言测试；当前播放入口、worker6、模型与安装版不变。
