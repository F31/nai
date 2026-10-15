## Why

§42.5 跨会话边界（V3.11 §42.5）在 memory 面**从未建判据**：只靠设计意图断言「授权、审批、租约、设备身份与自动权限不跨场继承」，没有任何可测守卫把它变成事实。

现测（不是推断）：

- `MemoryQuery.scope.session_id` 取自控制面自身绑定（`crates/nai-control-api/src/lib.rs:708-712`），请求同名字段被忽略——这是事实，但**没有测试钉死**「用别的 session 查询别的 session 的记忆被拒」。
- 跨场读取靠 `scope_matches`（`crates/nai-kernel/src/evaluate/memory.rs:619`）的 `record.session_id == scope.session_id`（非观众类）与 `viewer_scoped_id`（观众类）：行为已存在，但无显式判据，回归即可悄悄改坏。
- `memory_read` / `memory_forget` 鉴权走 `self.access`（`Access` 绑定 `session_id` + `device`），但无测试证明「跨场的 token / 别的设备」被拒。
- 场次记忆跨会话保留（按保留期）：`memory_records` 无「会话结束即清」逻辑，记录按 `expires_at_ms` 保留——但无守卫钉死「会话结束不清空场次记忆」。

W107/W109/W110/W111 四次都是「闭合判定面」：本次把 §42.5 在 memory 面的判定面也闭合。

## What Changes

- **钉死跨场读取守卫**：`scope_matches` 的 session / viewer_scoped_id 隔离**显式**成为可测判据——跨 session 查询非观众记录 ⇒ `OutOfScope`；跨 viewer_scoped_id 查询观众记录 ⇒ `OutOfScope`。
- **钉死跨场授权被拒**：`memory_read` / `memory_forget` 路由的 `self.access` 绑定当前场（session + device），补判据——用别的 session / 别的设备的 token 访问本场控制面 ⇒ `401` / `403`。
- **钉死场次记忆跨会话保留**：记录按 `expires_at_ms` 保留、不因会话结束被清空；旧场的记录对旧场仍按 session 隔离可读，对新场 `OutOfScope`，但**物理保留**（tombstone 之外的保留语义）。
- **作用域是事实，不是请求参数**（沿用 W109/W111 的纪律）：`scope.session_id` 来自控制面绑定，请求伪造同名字段被忽略——回归保护。

## Modified Capabilities

- `runtime-composition`：scope_matches 跨场隔离成为显式判据；场次记忆跨会话保留成为显式判据。
- `local-control-surface`：memory 路由的跨场 token / 设备被拒成为显式判据。
- `local-persistence`：记忆记录按保留期保留、会话结束不清空成为显式判据。

## Impact

- `crates/nai-runtime/tests/memory.rs`：补跨 session / 跨 viewer 读取守卫用例。
- `crates/nai-kernel/tests/memory.rs`：补 `scope_matches` 跨场隔离用例（若 runtime 层未覆盖）。
- `crates/nai-control-api/src/tests.rs`：补跨场 token / 设备被拒用例（memory 路由）。
- 生产代码**零改动**（判据钉死既有行为）；除非测试发现缺口，否则不引入新执行体。
