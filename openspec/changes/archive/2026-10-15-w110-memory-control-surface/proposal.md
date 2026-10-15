## Why

W109 给 §42.3 的读取判定与 §42.4 的遗忘阻断建了**执行体**——`Composition::read_memory` 与
`Composition::forget_memory`。但它们**没有调用方**：

```
$ git grep -n "read_memory\|forget_memory" -- crates apps | grep -v /tests/
crates/nai-runtime/src/memory.rs:164:    pub fn read_memory(
crates/nai-runtime/src/memory.rs:208:    pub fn forget_memory(
```

两处命中，**都是定义处**。也就是说：判定面存在（W57）、承载物存在（W109 的 `revoked_at_ms`）、
执行体存在（W109 的两个方法），而**在真实运行路径上，记忆读取判定与遗忘阻断一次都没有发生过**。

`write_memory` 同样如此（W107 交付、同样是零生产调用方），故本轮是 W103–W109 同母题的**第七轮**，
形状又变了一格：前六轮是「判定面存在、执行体不存在」，本轮是**「执行体存在、接线不存在」**——
代码写出来了，但系统运行时不会走到它。

**为什么必须接控制面**：§42.4 的遗忘链由**用户请求删除**触发（`ForgetTrigger::UserRequest`），
§42.3 的读取是操作员/合规核查时发生的行为。二者在基线里的触发者都是**外部请求**，
而本系统的外部请求入口就是 loopback 控制面。不接控制面，则「用户要求删除一条记忆」在仓库里
仍然**不可表达**——这正是 H04 的 notes 里登记的第一条遗留。

## What Changes

- `nai-control-api` 新增**记忆端点**：`GET /api/v1/memory`（读取）、`POST /api/v1/memory/forget`（遗忘阻断）。
- 新增**接缝** `MemorySurface`：由 `nai-control-api`（shared 层）定义、`Composition`（runtime 层）实现、
  `apps/engine` 装配时注入。分层实测：`xtask/src/deps.rs` 禁止 shared 依赖 runtime
  ⇒ 接缝 MUST 定义在 shared 一侧（见 design 决策 1）。
- 新增两条控制面权限名 `memory.read` / `memory.forget`（**是选择，不是本来如此**）。
- 引擎装配时把共享的 `Composition` 作为记忆后端注入控制面。

## Modified Capabilities

- `local-control-surface`：新增记忆端点的接线要求。

## Impact

- `crates/nai-control-api/src/lib.rs`：接缝 trait、结果枚举、两个路由、权限表。
- `crates/nai-runtime/src/memory.rs`：`impl MemorySurface for Composition`（映射，不重判）。
- `apps/engine/src/lib.rs`：装配时注入；`CONTROL_PERMISSIONS` 增两条。
- 测试：`crates/nai-control-api/src/tests.rs`（真实 loopback + reqwest）、
  `crates/nai-runtime/tests/memory.rs`（接缝实现的行为）。
- 不改契约生成物、不新增迁移、不新增依赖、不动 kernel 判定规则的任何一行。

## 明确不做

- **不接写入面**：`write_memory` 的真实触发者是「记忆的产生者」，§42.1 四类记忆各有产生者，
  属领域决策，不在本轮 ⇒ 如实披露：真实运行下读取面目前**只会读到零条**。
- **不做 §42.4 的清除副本一侧**。
- **不做 Pro 跨会话（§42.5）**。
- **不引入配置项**（`MemoryPolicy` 由装配点给定）。
- **不改 H03/H04 的 registry `status`**（仍 `pending`）。
