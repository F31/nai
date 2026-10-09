# Spec Delta

## Purpose

建立 composition root（`crates/nai-runtime`）：把存储在 owner、配置、会话与值守租约、审批后端、决策日志、媒体门控与媒体子进程监督装配成一个受监督整体，并在子进程故障时执行「先静音、再撤权、再记录、转待机」。kernel MUST NOT 依赖本装配根。

## ADDED Requirements

### Requirement: Composition root assembly and dependency direction

装配根 SHALL 在 `runtime` 层注册并只依赖既有实现；kernel MUST NOT 依赖 runtime 或具体插件（由 `cargo xtask check-deps` 强制）。装配 SHALL 保持存储在单 owner 与短 `Operation` 约定下访问，不得重开 Store 绕过恢复撤权。

#### Scenario: Dependency violation
- **WHEN** kernel 或 sdk 依赖 runtime、或引入 plugin→plugin 直接依赖
- **THEN** 依赖检查失败，装配不得合入

#### Scenario: Second storage owner
- **WHEN** 已在运行的装配再次打开同一数据根
- **THEN** 拒绝（Conflict），不产生第二个写 owner

### Requirement: Composition lifecycle logging and control ordering

装配 SHALL 把会话控制按「先媒体侧静音/撤权，再持久化与记录」的顺序执行，并把关键事件写决策日志：critical 类别写入失败 MUST 上报错误，telemetry 类别 MAY 进入有界缓冲。值守租约的授予、到期与撤权 MUST 记为 critical。

#### Scenario: Mute or takeover
- **WHEN** 控制面请求静音或接管
- **THEN** 媒体侧先失权、会话 epoch 递增，随后写入 critical 决策日志；控制 ACK 不代表设备已停止

#### Scenario: Critical logging failure
- **WHEN** 关键事件写入决策日志失败
- **THEN** 调用方收到错误，不静默丢弃关键记录

### Requirement: Supervised child fault handling

装配 SHALL 监督媒体子进程（keeper、render-worker）的存活与状态；检测到退出、被强杀或状态读取失败 MUST 依次执行：媒体侧立即静音 → 撤销输出授权（epoch 递增）→ 写 critical 决策日志 → 置待机且拒绝继续投递。MUST NOT 因日志或数据库故障而回退静音。

#### Scenario: Keeper killed
- **WHEN** 被监督的 keeper 子进程被强杀
- **THEN** 装配撤销输出授权、记录 critical 日志并转待机；决策日志链校验通过

#### Scenario: Publish failure while halting
- **WHEN** 故障处置期间决策日志写入失败
- **THEN** 静音与撤权仍然生效，错误如实上报，不假装已记录

### Requirement: Honest supervision boundary

装配与文档 MUST 声明：本装配没有真实设备、窗口、音频输出、PlaybackAck 或 loopback HTTP 服务启动；`--frame-drill`/`--render-drill` 之外的路径不得被表述为生产就绪或设备验收通过。

#### Scenario: Status reporting
- **WHEN** 装配对外报告状态
- **THEN** 输出可用性保持不可用，并给出明确的 `CapabilityNotReady` 或待机原因
