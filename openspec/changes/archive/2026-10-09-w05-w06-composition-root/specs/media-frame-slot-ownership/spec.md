# Spec Delta

## Purpose

补充 render-worker 侧的**生产装配形态**（非设备验收）：以真实 2D 插件生成像素、按 root 预留的 ticket 写入继承的池 FD，并通过事件帧上报写开始与提交结果。

## ADDED Requirements

### Requirement: Render worker produces real pixels into the reserved slot

render-worker SHALL 在显式 `--render-drill` 下工作：池 FD 经继承的标准流交付、PCM 以原始字节输入（MUST NOT 经 JSON/Base64 控制总线）、ticket 由 root 预留并校验；像素 MUST 由 2D 插件真实生成并按 ticket 的槽位偏移写入。MUST NOT 产生 `PlaybackAck`、声明 Ready 或授予输出授权。

#### Scenario: Real frame committed
- **WHEN** render-worker 依据合法 ticket 生成并写入像素
- **THEN** 输出 `write_started` 与 `committed` 事件，owner 按其账本接受提交

#### Scenario: Frame rejection
- **WHEN** ticket 过期、epoch 不符或像素长度/hash 与声明不符
- **THEN** 输出 error 事件并非零退出，owner 不接受提交

### Requirement: Producer rechecks epoch, cancel and budget

render-worker SHALL 在渲染与写入前复检 epoch、取消与期限预算；任一不满足 MUST 停止写入并显式失败，不得写入陈旧或超额数据。

#### Scenario: Stale epoch
- **WHEN** ticket 的 epoch 低于当前会话 epoch
- **THEN** 不写入池，报告失败
