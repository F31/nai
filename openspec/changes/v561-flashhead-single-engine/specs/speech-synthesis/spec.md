# Qwen3-TTS 语音合成

## ADDED Requirements

### Requirement: 本地 Qwen3-TTS 适配

系统 SHALL 通过可替换的本地 TTS 适配器调用 Qwen3-TTS，默认 0.6B；适配器 SHALL 负责健康检查、声线/语言/参数、超时、错误与取消，不得把 TTS 实现细节泄漏到会话或 UI。

#### Scenario: Qwen 正常
- **WHEN** Qwen3-TTS 端点可达且模型已加载
- **THEN** 系统可合成中文语音、试听、播放，并上报模型/声线/采样率/耗时

#### Scenario: Qwen 不可达
- **WHEN** 健康检查失败或合成超时
- **THEN** 当前句失败并显示原因；系统可进入文字模式，不播放替代/伪造音频

#### Scenario: 模型身份未知
- **WHEN** 无法确认模型版本、采样率或流式能力
- **THEN** UI 显示未知，不把未确认参数写成默认能力

### Requirement: 流式优先、句级降级

系统 SHALL 优先消费 Qwen3-TTS 的增量 PCM；当服务只能返回完整 WAV 时，系统 SHALL 退化为句级合成 + 预取 1–2 句，并保持有界队列。

#### Scenario: 真流式
- **WHEN** Qwen 返回增量 PCM
- **THEN** 系统边收边送入播放与口型分支，记录首包/首音

#### Scenario: 非流式
- **WHEN** Qwen 只返回完整 WAV
- **THEN** 系统按句请求、预取下一句，首句优先短句；不声称“流式”

#### Scenario: 预取串句
- **WHEN** 预取队列存在多句
- **THEN** 每句带序号/epoch，按顺序播放，不跨句拼接或错序

### Requirement: 双路采样与音频主时钟

系统 SHALL 将 TTS PCM 分为播放分支（设备采样率，通常 48k）与口型分支（16k 特征），两者 SHALL 使用同一会话时间线与 epoch；SHALL NOT 混用采样率或时钟。

#### Scenario: 播放与口型
- **WHEN** 一句语音开始播放
- **THEN** 口型按同一句的 16k 特征驱动，帧选择按已播放样本位置

#### Scenario: 采样率未知
- **WHEN** Qwen 实际采样率未知或与配置不符
- **THEN** 系统先探测/重采样；无法确认时拒绝开始，不使用猜测值

### Requirement: 取消与旧音频阻断

系统 SHALL 在停止、静音、接管、切段或新 epoch 时清空待播队列并拒绝旧 epoch 音频；当服务端不支持取消时 SHALL 明确记录客户端丢弃而非服务端停止。

#### Scenario: 用户停止
- **WHEN** 用户按下停止
- **THEN** 系统立即停止播放、清空队列、标记新 epoch，旧音频不得续播

#### Scenario: 服务端无取消
- **WHEN** Qwen 不支持服务端中止
- **THEN** 客户端丢弃旧结果并记录“服务端可能仍在推理”，不声称推理已停止

### Requirement: 缓存与可观测

系统 SHALL 以文本、声线、语言、参数、模型版本为键缓存合成结果，并设置容量/过期上限；SHALL 记录首包、首音、失败、取消、队列深度与缓存命中。

#### Scenario: 重复文本
- **WHEN** 同文本/声线/参数/模型再次请求
- **THEN** 命中缓存并跳过合成；参数不同不得复用

#### Scenario: 缓存超限
- **WHEN** 缓存达到上限
- **THEN** 按 LRU/过期清理，不无界增长

### Requirement: 声线与许可

系统 SHALL 支持选择/试听声线、导入参考音频（如 Qwen 支持克隆），并要求用户确认声音使用权；未确认时 SHALL 拒绝用于直播。

#### Scenario: 未确认声音权利
- **WHEN** 用户选择参考音频但未确认授权
- **THEN** 系统拒绝开始直播并提示确认，不静默使用

#### Scenario: 声线试听
- **WHEN** 用户点击试听
- **THEN** 系统合成短句并播放，不启动直播会话
