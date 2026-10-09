# avatar-2d-rendering Specification

## Purpose
提供不依赖 GPU/SDK 的授权 2D 像素生产核心，建立样本时轴驱动的有界 RMS 嘴型合成前置，供独立 worker 后续装配。

## Requirements

### Requirement: Explicit bounded layered asset admission
2D 核心 SHALL 从 JSON Schema 配置与通用形象清单校验精确三层 RGBA8 闭包、SHA256、尺寸/角色/位置和独立 root 许可授权；缺失、额外、重复、篡改或授权缺失 MUST 显式拒绝，不猜默认配置或授予输出 Ready。

#### Scenario: Corrupted or unlicensed layer
- **WHEN** 任一层内容哈希不一致或许可引用未授权
- **THEN** 不创建可渲染形象，返回明确失败

### Requirement: Sample-clock driven RMS rendering
渲染 SHALL 在有界 mono f32 PCM 队列上按对应音频样本窗口计算 RMS、噪声门限与 attack/release 平滑，用该强度合成真实 RGBA 像素；每调用最多一帧，非整数 fps 映射不得累积舍入漂移，EOS 尾块可形成短尾帧。不得以到达时间或最近网络块音量替代样本时间，也不得宣称精确音素口型。

#### Scenario: Different packet boundaries
- **WHEN** 相同 PCM 按不同合法块边界输入
- **THEN** 对应帧的样本窗口、嘴型与像素相同

#### Scenario: Bounded input overflow
- **WHEN** 输入超过配置缓冲或包含非有限值、格式变化、跳号、样本缺口
- **THEN** 拒绝整块，不覆盖或消费既有 PCM

### Requirement: Fenced cancellable frame production
输入/渲染 SHALL 复检 session/epoch/utterance、许可、deadline/cancellation，失败 MUST 不推进帧/PCM/平滑状态。输出 FrameMetadata 绑定实际样本区间与显式 buffer_ref；像素为二进制数据面所有权，不走 JSON/Base64，不表示 keeper 提交或实际播放 ACK。

#### Scenario: Revocation or cancellation before frame
- **WHEN** epoch 不符、许可撤销、context 取消或期限届满
- **THEN** 不返回帧，不继续消费输入，不能恢复旧授权

#### Scenario: Portable evidence boundary
- **WHEN** 软件渲染测试通过
- **THEN** 只登记像素生产核心证据，worker/共享内存/Windows设备/实时帧率验收仍明确未完成
