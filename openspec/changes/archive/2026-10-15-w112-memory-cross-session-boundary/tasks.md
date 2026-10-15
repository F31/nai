# W112 任务：§42.5 跨会话边界在 memory 面的判定面闭合

## 1 判据（先测，钉死既有行为）

- [x] 1.1 现测 `scope_matches` 跨场隔离：非观众记录按 session_id、观众记录按 viewer_scoped_id；跨场 ⇒ OutOfScope
- [x] 1.2 现测 `self.access` 的场绑定：memory 路由鉴权走 session + device；跨场 token ⇒ 401
- [x] 1.3 现测场次记忆跨会话保留：记录按 `expires_at_ms` 保留、无 session-end 清除

## 2 runtime 判据

- [x] 2.1 `crates/nai-runtime/tests/memory.rs`：跨 session 查询非观众记录 ⇒ OutOfScope（具名，归属既有测试）
- [x] 2.2 跨 viewer_scoped_id 查询观众记录 ⇒ OutOfScope（具名，新增）
- [x] 2.3 场次记忆跨会话保留：旧场记录物理保留、对新场 OutOfScope、不删行（新增）

## 3 ControlApi 判据

- [x] 3.1 `crates/nai-control-api/src/tests.rs`：用别的场的 token 访问本场控制面 memory_read/forget ⇒ 401（新增）
- [x] 3.2 用别的 device 的 token 访问 ⇒ 401（与 3.1 同一条 `self.access` 绑定，决策 5，不单列测试）
- [x] 3.3 作用域是事实：请求伪造 `session_id` 字段被忽略，仍用控制面绑定（归属既有测试）

## 4 kernel 判据（如需）

- [x] 4.1 `scope_matches` 跨场隔离已由 runtime 2.2 经 `read_memory` 端到端覆盖；kernel 单测冗余，不做

## 5 文档与闭环

- [x] 5.1 ADR 0090（§42.5 memory 面判定面闭合）+ 报告 + `development-plan.md` W112 段
- [x] 5.2 `kernel.md` / `runtime.md` 就地订正（§42.5 状态：memory 面判定面已建，观众 authorization_ref 当场绑定仍遗留）
- [x] 5.3 registry `H03` / `H04` notes 追加 W112 段（`status` 仍 `pending`）
- [x] 5.4 全量门控 `cargo xtask check --profile full` + 两条 validate

## 6 流程项（非 checkbox）

提交 / 推送 / CI 观测 / 回填在归档那一刻结构上不可能完成 ⇒ 不写成任务项。
