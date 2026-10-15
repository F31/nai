## ADDED Requirements

### Requirement: Memory endpoints forward to the runtime through a seam

控制面 SHALL 提供 `POST /api/v1/memory/read` 与 `POST /api/v1/memory/forget`，经接缝 `MemorySurface`
调到装配根的读取与遗忘落点。接缝 SHALL 由 `nai-control-api` 定义、由装配根实现、由引擎注入。
控制面 MUST NOT 自己重跑 §42.3／§42.4 的任何一条判定。
后端未注入 SHALL 报 `CAPABILITY_NOT_READY`，MUST NOT 假成功。

#### Scenario: A wiring that exists is a wiring that runs

- **WHEN** 以持 `memory.read` 的 token 请求 `POST /api/v1/memory/read`
- **THEN** 接缝**真的被调用**（调用可被观测），MUST NOT 是一个恒真桩

#### Scenario: No backend is not an empty backend

- **WHEN** 控制面未注入记忆后端而收到记忆请求
- **THEN** 报 `CAPABILITY_NOT_READY`，MUST NOT 以空结果报成功

### Requirement: A refused read is reported by name, not as an empty read

请求级拒绝 SHALL 以具名 HTTP 错误返回，MUST NOT 折成 `200` 加空 `records`——
「本次读取不成立」与「读出来零条」不得不可区分。路径自身的故障 SHALL 与判定拒绝分开，
MUST NOT 把存储故障或锁竞争记在判定名下。

#### Scenario: A read the judgement refuses is not an empty read

- **WHEN** 查询本身被 §42.3 拒绝（例如请求了近似顶替，或观众记忆未启用）
- **THEN** 响应是具名的请求级错误，而不是 `200` 加空 `records`

#### Scenario: Storage faults are not reported as refusals

- **WHEN** 记忆后端返回故障（例如装配根正忙）
- **THEN** 响应把它报成冲突，MUST NOT 报成权限不足或判定拒绝

### Requirement: Memory reads are scoped by the control surface and out-of-scope stays decidable

记忆读取的作用域维度（workspace 与 session）SHALL 由控制面自身的绑定给出，
MUST NOT 由请求指定。越作用域这一条判定 MUST 仍然可达：属于同一工作区但**其他会话**的记录
SHALL 逐条进 `refused` 并具名 `out_of_scope`，MUST NOT 被取数层预先筛掉。

#### Scenario: An out-of-scope record is named, not dropped

- **WHEN** 同一工作区里存在属于其他会话的记忆记录，且经控制面发起一次读取
- **THEN** 该记录逐条出现在 `refused` 中且拒因为 `out_of_scope`

#### Scenario: A request cannot choose its own scope

- **WHEN** 请求里带上 `workspace_id` 或 `session_id`
- **THEN** 这两个字段被忽略，取数仍按控制面自身的绑定

### Requirement: Forget blocks the read side first and fails by name

遗忘端点 SHALL **先**阻断读取、再判定这次遗忘的陈述是否如实，并返回**生效中的**撤销时刻
（已撤销则返回原有时刻，MUST NOT 覆盖）。记录不存在 SHALL 具名失败，MUST NOT 静默成功。

#### Scenario: Forget blocks the read side first

- **WHEN** 以持 `memory.forget` 的 token 对一条存在的记录请求遗忘
- **THEN** 该行被标为已撤销，随后读取该记录时它进 `refused(revoked)`；
  重复遗忘返回**同一个**撤销时刻

#### Scenario: A missing record is a named failure

- **WHEN** 遗忘请求给的 `record_id` 在本工作区不存在
- **THEN** 响应是具名的失败，MUST NOT 报成功
