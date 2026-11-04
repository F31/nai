# 可靠性与故障

## ADDED Requirements

### Requirement: Worker 生命周期与看门狗

系统 SHALL 对视觉 worker 设置心跳、超时、重启上限（每场 ≤2 次）与失败降级；SHALL 在 worker 失联时切安全帧，不得继续旧帧。

#### Scenario: Worker 崩溃
- **WHEN** worker 进程退出或 3s 无心跳
- **THEN** 输出安全帧，尝试重启；失败则降级/停止，记录原因

#### Scenario: Worker 卡死
- **WHEN** worker 不响应但进程存在
- **THEN** 超时后终止并重启，不无限等待；不把旧帧当直播

### Requirement: 依赖失联

系统 SHALL 分别处理 Qwen 不可达/超时、OBS 退出、音频设备丢失、engine 崩溃；SHALL 明确降级/停止，不伪造音频/画面/连接。

#### Scenario: Qwen 超时
- **WHEN** TTS 请求超时
- **THEN** 当前句失败并提示；可继续文字模式，不播放替代音频

#### Scenario: 音频设备丢失
- **WHEN** 输出设备断开
- **THEN** 立即静音并提示，不自动切到默认设备

#### Scenario: Engine 崩溃
- **WHEN** engine 异常退出
- **THEN** launcher 可重启；不自动恢复直播；旧帧/旧音频/旧许可不复活

### Requirement: 资源与散热保护

系统 SHALL 监控 VRAM、系统 RAM、温度与功耗；SHALL 在温度 ≥87°C、RAM 接近 28GB、VRAM 不足或持续掉帧时告警/降级/停止；SHALL NOT 靠静默降质或无限缓存维持。

#### Scenario: 高温
- **WHEN** 温度达到/超过 87°C
- **THEN** UI 告警并建议降级/切 Lite/停止；记录温度曲线

#### Scenario: 内存紧张
- **WHEN** 系统内存接近告警线
- **THEN** 停止后台/预览/非必要缓存，仍不足则安全停止

### Requirement: 旧 epoch 与未知结果

系统 SHALL 用 epoch/seq 拒绝旧帧、旧音频、旧动作与旧回答；SHALL 将未知结果标为未知，不自动重放。

#### Scenario: 停止后旧结果
- **WHEN** 停止后 LLM/TTS/worker 返回旧结果
- **THEN** 系统丢弃，不显示/播放

#### Scenario: 未知提交
- **WHEN** 外部副作用结果未知
- **THEN** 不使用“已成功”或“已失败”，保持未知并记录
