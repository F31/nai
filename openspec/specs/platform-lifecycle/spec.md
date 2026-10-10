# platform-lifecycle Specification

## Purpose
把 §27.1「桥接只做」、§347 伴侣生命周期与七类需处理情形、§349 能力快照最严格交集与「缺失能力只关闭对应功能」、§119「平台事件中断，自动互动撤权；手工与回放仍可用」、§1328 P01 验收的**判定口径**落成在 portable 下可执行、可测试的骨架：桥接职责是**封闭四项**，伴侣**启动、退出、重连、切场次**四类转换各有处置并在每次转换**清掉播报授权**，**旧房间永不播报**，伴侣退出后**引擎转待机**，重连**不恢复旧自动权限**，平台能力快照取**最严格交集**且**只关闭对应功能、不自行合成事件**，平台事件中断**撤自动互动但保留手工与回放**，官方 SDK **只经固定路径与握手校验加载**，无官方 SDK 时**如实记 `UNVERIFIED_EXTERNAL_DEPENDENCY`、绝不用 fake connector 冒充**，SDK 回调**不同步等待 LLM 或磁盘**。本能力只判定**一份桥接生命周期描述是否越界**，不启动伴侣、不加载 SDK、不建网络连接、不推流；这些真值由外部条件提供，未提供时 readiness 保持 `Blocked`（P01 的诚实状态）。

## Requirements

### Requirement: Bridge duties are a closed set
桥接 SHALL 只做**四项**：SDK 初始化、官方事件与权限转换、与引擎的认证 IPC、退出（§27.1「桥接只做」）。声明四项之外的额外职责 MUST 被审计逐条报出。

#### Scenario: A duty outside the bridge's four
- **WHEN** 桥接声明承担「生成回复内容」这一额外职责
- **THEN** 审计报出 `duty_outside_bridge_scope` 并指名该职责

#### Scenario: Only the four declared duties
- **WHEN** 桥接职责恰为 SDK 初始化、事件与权限转换、认证 IPC 与退出
- **THEN** 审计不报出职责越界缺陷

### Requirement: Lifecycle transitions are disposed and clear broadcast authority
伴侣**启动、退出、重连、切场次**四类转换 SHALL 各有明确处置（§1328），且**每一次转换都清除播报授权**（§1328「不产生旧房间播报」）。任一转换类无处置 MUST 被审计报出并指名该类；某类转换的处置**不清除**播报授权 MUST 被审计报出。**未声明桥接职责时不存在生命周期转换**，本项 MUST NOT 产生缺陷。

#### Scenario: A transition class is undisposed
- **WHEN** 已声明桥接职责，但未声明对「切场次」的处置
- **THEN** 审计报出 `transition_undisposed` 并指名 `room_switched`

#### Scenario: A transition keeps the old broadcast authority
- **WHEN** 「重连」的处置不清除播报授权
- **THEN** 审计报出 `transition_keeps_stale_broadcast` 并指名 `reconnected`

#### Scenario: No bridge is declared
- **WHEN** 未声明任何桥接职责
- **THEN** 审计不报出任何生命周期转换缺陷

### Requirement: Lifecycle transitions are written to the decision log
每一次生命周期转换 SHALL 写入**决策日志**（§1497 载体、§27.1 桥接职责）。某类转换的处置不记录该转换 MUST 被审计报出并指名该类。

#### Scenario: A transition disposition does not log
- **WHEN** 「退出」转换的处置不写入决策日志
- **THEN** 审计报出 `transition_not_logged` 并指名 `exited`

### Requirement: A stale room never broadcasts
播报授权 SHALL 绑定到**房间与连接代次**；授权与其所覆盖的当前连接不一致时 MUST NOT 允许播报（§1328「不产生旧房间播报」）。存在与当前连接**不一致**的播报授权 MUST 被审计报出。

#### Scenario: A stale-room authority is still in force
- **WHEN** 当前连接为房间 B 与代次 2，而生效的播报授权仍绑定房间 A 与代次 1
- **THEN** 审计报出 `stale_room_broadcast_permitted`，且 `may_broadcast` 为假

#### Scenario: An authority matching the current connection
- **WHEN** 播报授权与当前连接的房间与代次一致
- **THEN** 不报出陈旧房间缺陷，且 `may_broadcast` 为真

### Requirement: Partner exit leaves the engine in standby
伴侣退出时，引擎 SHALL 转待机（§347）。「退出」转换的处置**未声明引擎转待机** MUST 被审计报出并指名该类。未声明「退出」处置时，本项 MUST NOT 另行报出（同一处缺陷不重复计）。

#### Scenario: Exit without standby
- **WHEN** 「退出」转换的处置未声明引擎转待机
- **THEN** 审计报出 `exit_without_standby` 并指名 `exited`

#### Scenario: Exit declares the engine in standby
- **WHEN** 「退出」转换的处置声明引擎转待机
- **THEN** 不报出退出相关缺陷

### Requirement: Reconnect never restores previous automatic authority
重连 MUST NOT 恢复旧的自动权限（§347「重连不恢复旧自动权限」；§119「重连后不恢复旧自动权限」）。「重连」的处置若声明**恢复旧自动权限** MUST 被审计报出。

#### Scenario: Reconnect restores the old automatic authority
- **WHEN** 「重连」的处置声明恢复先前的自动权限
- **THEN** 审计报出 `reconnect_restores_previous_auto_authority`

#### Scenario: Reconnect keeps automatic authority closed
- **WHEN** 「重连」的处置不恢复任何自动权限
- **THEN** 不报出重连相关缺陷

### Requirement: Capability snapshots use the strictest intersection and only close the matching feature
平台能力快照取值 **granted / denied / unknown**，取**最严格交集**（§349）：`unknown` MUST 视为**未授予**，MUST NOT 被读作已授予。**缺失能力只关闭对应功能**，MUST NOT 自行合成事件。将 `unknown` 读作已授予 MUST 被审计报出；某功能仍启用而其所需能力**非 granted** MUST 被审计报出；某功能**因能力变更被停止**而其所依赖能力**仍为 granted** MUST 被审计报出（过度关闭）。

#### Scenario: An unknown capability read as granted
- **WHEN** 一份能力快照把 `unknown` 记录为已授予
- **THEN** 审计报出 `unknown_capability_treated_as_granted` 并指名该能力

#### Scenario: A feature stays active without its capability
- **WHEN** 某功能仍处于启用状态，而其所依赖的能力为 `denied` 或 `unknown`
- **THEN** 审计报出 `feature_active_without_capability` 并指名该功能

#### Scenario: A feature is closed despite an available capability
- **WHEN** 某功能因能力变更被停止，而其所依赖的能力仍为 `granted`
- **THEN** 审计报出 `feature_stopped_despite_capability` 并指名该功能

### Requirement: Platform interruption revokes automatic replies but keeps manual and replay
平台事件中断时 SHALL **撤销自动互动**，而**手工与回放仍可用**（§119）。降级不撤销自动互动 MUST 被审计报出；使手工不可用 MUST 被审计报出；使回放不可用 MUST 被审计报出。**非平台中断**的降级 MUST NOT 因此产生缺陷。

#### Scenario: An interruption keeps automatic replies running
- **WHEN** 平台事件中断的处置未撤销自动互动
- **THEN** 审计报出 `platform_interruption_keeps_auto_reply`

#### Scenario: An interruption takes manual or replay down with it
- **WHEN** 平台事件中断使手工或回放不可用
- **THEN** 审计分别报出 `platform_interruption_disables_manual` / `platform_interruption_disables_replay`

#### Scenario: A degradation that is not a platform interruption
- **WHEN** 一份非平台中断的降级记录未撤销自动互动
- **THEN** 审计不报出平台中断相关缺陷

### Requirement: Platform interruptions are written to the decision log
平台事件中断的处置 SHALL 写入**决策日志**（§27.1 桥接职责）。不记录 MUST 被审计报出。

#### Scenario: An interruption disposition does not log
- **WHEN** 平台事件中断的处置不写入决策日志
- **THEN** 审计报出 `degradation_not_logged`

### Requirement: Every declared condition class has a disposition and is logged
桥接 SHALL 处置**伴侣多次启动、端口占用、SDK 失联、消息重复、旧房间事件、时间漂移、权限撤销**七类情形（§347）。任一情形类无处置 MUST 被审计报出并指名该类；处置不记录该情形 MUST 被审计报出并指名该类。

#### Scenario: A condition class is undisposed
- **WHEN** 已声明桥接职责，但未声明对「时间漂移」的处置
- **THEN** 审计报出 `condition_undisposed` 并指名 `time_drift`

#### Scenario: A condition disposition does not log
- **WHEN** 「权限撤销」的处置不写入决策日志
- **THEN** 审计报出 `condition_not_logged` 并指名 `permission_revoked`

### Requirement: The official SDK is loaded only through a pinned, trusted path
官方 SDK 库 MUST NOT 从任意路径加载；加载 SHALL 校验**固定路径、签名、nonce 与有效令牌**（§27.1、§1075）。声明 SDK 已核验而任一项握手校验缺失 MUST 被审计逐项报出。

#### Scenario: An SDK loaded without all trust checks
- **WHEN** 桥接声明官方 SDK 已核验，但未校验签名
- **THEN** 审计报出 `sdk_loaded_without_trust_check` 并指名该检查项

#### Scenario: An unverified SDK is not judged for trust checks
- **WHEN** 桥接如实声明没有官方 SDK（`UNVERIFIED_EXTERNAL_DEPENDENCY`）
- **THEN** 审计不报出 SDK 信任检查缺陷

### Requirement: No official SDK means UNVERIFIED_EXTERNAL_DEPENDENCY, never a fake connector
没有官方接口文档或 SDK 时 SHALL 返回 `UNVERIFIED_EXTERNAL_DEPENDENCY`，演练走 **manual 与 replay**，MUST NOT 用 **fake connector** 冒充真实能力（§27.1）。在该状态下把 fake connector 冒充真实能力 MUST 被审计报出。

#### Scenario: A fake connector claimed as a real capability
- **WHEN** 无官方 SDK 且把 fake connector 冒充为真实平台能力
- **THEN** 审计报出 `fake_connector_claimed_as_real_capability`

#### Scenario: An honest unverified external dependency
- **WHEN** 无官方 SDK 且如实记录为 `UNVERIFIED_EXTERNAL_DEPENDENCY`，未冒充真实能力
- **THEN** 审计不报出冒充相关缺陷

### Requirement: SDK callbacks never block on the LLM or a disk transaction
SDK 回调 SHALL 快速入队，MUST NOT 同步等待 LLM 或磁盘大事务（§27.1）。同步等待 MUST 被审计报出。

#### Scenario: A callback blocks on the LLM or disk
- **WHEN** 桥接的回调策略声明同步等待 LLM 或磁盘事务
- **THEN** 审计报出 `callback_blocks_on_llm_or_disk`

#### Scenario: A callback that only enqueues
- **WHEN** 桥接的回调策略只快速入队，不同步等待
- **THEN** 不报出回调相关缺陷

### Requirement: Readiness is Blocked until a bridge is declared, and Defective outranks it
审计非空时 readiness MUST 为 `Defective`，且该判定 MUST 优先。审计为空且未声明任何桥接职责时 readiness MUST 为 `Blocked`（P01 诚实状态：官方 SDK 与生产宿主尚未实现），MUST NOT 记为缺陷。审计为空且已声明桥接职责时 readiness 为 `Ready`。

#### Scenario: No bridge declared
- **WHEN** 未声明任何桥接职责，且无任何违规
- **THEN** readiness 为 `Blocked` 且审计为空

#### Scenario: Defect outranks the undeclared bridge
- **WHEN** 未声明桥接职责，但存在与当前连接不一致的播报授权
- **THEN** readiness 为 `Defective`，不得因「尚未接入官方 SDK」而掩盖该缺陷

#### Scenario: A declared bridge with no violations
- **WHEN** 桥接职责与四类转换处置齐备且无任何违规
- **THEN** readiness 为 `Ready`

### Requirement: Bridge audit is a pure function of the descriptor
审计 SHALL 是对所声明桥接字段的**纯函数**，报出全部命中项，使每条规则都能被直接构造与验证。

#### Scenario: A clean descriptor audits empty
- **WHEN** 直接构造一份满足全部约束的桥接描述
- **THEN** 审计为空

#### Scenario: Every finding is constructible
- **WHEN** 直接构造一份含某违规的描述
- **THEN** 审计报出对应的稳定标记，无需启动伴侣、加载 SDK 或建立任何真实连接
