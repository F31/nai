# 常驻FlashHead worker与同源PCM

## Why

当前FlashHead只有每次加载模型的诊断入口。新语音链路需要独立、可取消的常驻模型组件，按任务验证同源PCM，禁止重复生成另一段音频来冒充同步。

## What Changes

新增私有继承stdio的签名Protobuf控制协议、配置Schema、Rust进程owner和独立Python/PyTorch常驻worker。PCM走有界文件快照，帧走有界共享内存诊断接收器，不进控制总线。模型和参考形象只初始化一次，逐任务重置运动历史、音频缓存和随机状态。明确技术验收入口；生产pool身份、公共窗口、音频时钟汇合和OBS后续接线，不授34ACC或M0通过。无新常驻依赖，无vLLM/MuseTalk兼容层。
