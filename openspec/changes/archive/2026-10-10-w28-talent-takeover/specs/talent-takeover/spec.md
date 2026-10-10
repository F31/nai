## Purpose

把 B05 从声明变成可执行判据：真人接管 MUST 同时暂停自动、静音数字人、清空队列并可选接入真人轨；TTS 轨与真人轨 MUST 是两条独立通道，任一时刻 MUST 至多一条可听；系统 MUST 只允许**一条**有效收音路径，麦克风 MUST 以稳定设备标识表示，默认设备变化 MUST 被报告而非静默跟随；接管 MUST 要求新鲜通过的回声测试；恢复自动 MUST 是显式操作并复核授权、事实版本与值守租约，真人讲话结束 MUST NOT 触发恢复。本 capability 不授予真实声卡、真实回声环境或真实 OS 混音器的认证。

## ADDED Requirements

### Requirement: Takeover pauses auto, mutes the digital human and clears the queue

接管 MUST 同时做四件事：暂停自动、把数字人（TTS 轨）置为静音、清空自动播放队列、（可选）接入真人音轨。接管 MUST 报告实际清空的条数，MUST NOT 只断言「已清空」。已处于接管时再次接管 MUST 被拒绝为冲突。接管 MUST 要求一个非空的事实版本，因为它将在恢复时被复核。

#### Scenario: Takeover clears the queue and mutes TTS
- **WHEN** 自动队列中有若干条待播，且回声测试通过，执行接管
- **THEN** 报告显示自动已暂停、TTS 已静音、清空条数与队列实长一致，且接管后队列为空、TTS 轨增益为零

#### Scenario: A second takeover is refused
- **WHEN** 已处于接管状态时再次执行接管
- **THEN** 接管被拒绝为冲突，模式与两条轨的增益都不变

#### Scenario: An empty fact version is refused
- **WHEN** 以空的事实版本执行接管
- **THEN** 接管被拒绝为参数非法

### Requirement: The TTS track and the talent track are never summed

混音器 MUST 把 TTS 轨与真人轨当作**两条独立的通道**，任一时刻 MUST 至多一条可听；处于接管时 TTS 轨 MUST 静音。被静音的那条轨 MUST 由混音器从当前模式推导，MUST NOT 依赖调用方在调用前把增益摆好。

#### Scenario: Auto renders only the TTS track
- **WHEN** 混音器处于自动模式
- **THEN** 渲染出的 TTS 增益非零，真人轨增益为零

#### Scenario: Takeover renders only the talent track
- **WHEN** 混音器已接管并接入真人轨
- **THEN** 渲染出的 TTS 增益为零，真人轨增益为接入值

### Requirement: Only one live capture path is allowed

系统 MUST 只允许一条有效收音路径。已有活跃路径时，新的申请 MUST 被拒绝，且拒绝 MUST 指明当前已被占用——不同持有者或不同设备 MUST NOT 成为放行理由。收音路径的释放 MUST 只接受**当前代次**的租约，旧租约 MUST NOT 释放新的持有者。麦克风 MUST 以**稳定设备标识**表示；系统默认设备的变化 MUST 被报告（含变化前后），MUST NOT 被静默跟随。

#### Scenario: A second capture path is refused
- **WHEN** 已有一条活跃收音路径，另一持有者在另一设备上申请
- **THEN** 申请被拒绝为冲突，活跃路径保持原样，活跃路径数仍为一

#### Scenario: A stale lease cannot free the current holder
- **WHEN** 释放旧代次的租约，而路径已被新代次的持有者取得
- **THEN** 释放被拒绝为陈旧代次，新持有者仍然活跃

#### Scenario: A default device change is observed, not followed
- **WHEN** 上报第一次默认设备、相同的设备、以及一个不同的设备
- **THEN** 依次得到「首次」「未变化」「变化（含前后设备）」，且当前默认设备被记住为新值

### Requirement: Takeover requires a fresh passing echo test

接管 MUST 要求一次**通过且未过期**的回声测试。未运行、运行中或失败的测试 MUST 以能力未就绪拒绝；已过期的通过 MUST 以证据过期拒绝；通过时刻晚于当前时刻 MUST NOT 被接受。回声测试 MUST 有有效期，旧的通过 MUST NOT 被当作当前无回声的证据。

#### Scenario: An unrun or failed test blocks takeover
- **WHEN** 回声测试未运行、运行中或失败时执行接管
- **THEN** 接管被拒绝为能力未就绪，模式保持自动

#### Scenario: An expired pass is refused
- **WHEN** 回声测试曾在有效期之前通过，现已超出有效期
- **THEN** 接管被拒绝为证据过期

#### Scenario: A pass in the future is not accepted
- **WHEN** 回声测试的通过时刻晚于当前时刻
- **THEN** 该测试不被视为有效

### Requirement: Resuming auto is an explicit, re-checked decision

恢复自动 MUST 只能由显式操作触发。恢复 MUST 复核三件：调用方已获授权、事实版本与**接管时快照的事实版本**一致、且存在**仍未到期**的值守租约。任一不满足 MUST 被拒绝为权限不足，且模式 MUST 保持接管。真人讲话结束 MUST NOT 触发恢复。

#### Scenario: Talent silence does not resume
- **WHEN** 处于接管时上报真人讲话结束
- **THEN** 返回当前模式为接管，模式不发生任何改变

#### Scenario: Resume re-checks authorization, fact version and duty lease
- **WHEN** 依次以未授权、事实版本不一致/为空、值守租约已到期/缺失发起恢复
- **THEN** 每次都被拒绝为权限不足，且模式保持接管

#### Scenario: Resume restores TTS and mutes talent
- **WHEN** 以已授权、事实版本一致、值守租约仍在有效期内的凭据恢复
- **THEN** 模式回到自动，TTS 轨恢复，真人轨静音

### Requirement: A mix audit over reported state

审计 MUST 是对**上报快照值**的纯函数，MUST 检出：两条轨同时可听、接管时 TTS 未静音、自动时真人有声、接管而无收音路径、接管而无有效回声测试、多于一条活跃收音路径。审计 MUST 报出**全部**违规而非第一条，并以稳定标记标识每一类，使报告无需 debug 构建即可分类。

#### Scenario: A clean snapshot has no violation
- **WHEN** 对一份接管中、TTS 静音、真人有声、恰有一条活跃收音路径且回声有效的快照执行审计
- **THEN** 不报出任何违规

#### Scenario: Each violation is reported by a stable token
- **WHEN** 分别构造只触发一类违规的快照
- **THEN** 每次报出且仅报出该类对应的标记；同时触发多类时，全部标记按固定顺序报出
