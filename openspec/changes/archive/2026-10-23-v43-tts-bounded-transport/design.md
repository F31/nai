# 设计
仅在既有adapter增加tts模块，不引入旧provider或plugin间依赖。请求持有Zeroizing Key，结果返回owned PCM16与固定采样率；失败不返回部分音频。WAV解析逐chunk边界校验RIFF长度、PCM编码、位宽、采样率、字节率、通道、data完整性，拒绝重复fmt/data与压缩。音频只在Rust内存，未来worker/媒体接线需另做契约。
字符费用公式为ceil(chars*分单价/百万)，未知不能等于零；估算不是供应商账单，不伪造usage。真正启动须后续由唯一writer原子预留、派发并结算，当前不提供UI调用。Windows/WSL本机HTTP验证协议、取消、期限、上限、坏WAV与敏感错误不泄漏；不调用付费服务，不自动播音。
