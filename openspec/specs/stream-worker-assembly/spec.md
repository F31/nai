# stream-worker-assembly Specification

## Purpose
规定真实常驻增量worker的加载预热、签名控制、独立PCM及共享帧释放生命周期，区分输入EOS与渲染完成，确保取消后清理状态才可复用模型；未装配Rust设备链路前不得以独立进程诊断授予公共播放或持续实时通过。

## Requirements

### Requirement: 真实增量worker生命周期
增量worker SHALL 在真实模型和编译预热完成后才Ready，使用独立控制和raw输入驱动同一GPU会话，完成前不接受重叠任务。

#### Scenario: EOS已验证但GPU未完成
- **WHEN** 输入EOS回执已发送而GPU仍在生成尾帧
- **THEN** 不发送RenderComplete，仍接受签名取消和帧释放。

#### Scenario: 取消后恢复
- **WHEN** GPU确认取消且映射已回收
- **THEN** 更高task/epoch的新Begin可复用同一模型，旧帧释放不得影响新任务。
