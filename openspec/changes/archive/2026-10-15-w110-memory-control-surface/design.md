## 决策 1 — 接缝定义在 `nai-control-api`（shared），实现在 `nai-runtime`

**实测依据**：`xtask/src/deps.rs` 的 forbidden 表
`"shared" | "provider" => matches!(target.layer, "plugin" | "runtime" | "app" | "tool")`
⇒ shared **不得**依赖 runtime。而 `crates/nai-runtime/Cargo.toml` 实测含
`nai-control-api = { path = "../nai-control-api" }`（runtime → shared 允许）。

所以唯一合法形状是：shared 定义 trait，runtime 写 `impl Trait for Composition`
（impl 与类型同 crate ⇒ 孤儿规则允许），app 注入。

**替代方案（否决）**：把接缝放进 `nai-kernel`。kernel 是**判定层**，接缝是**接口**职责；
且动 kernel 等于动 W57 的判定面领地，违反「不动判定规则」的边界。

**形状**：
```rust
pub trait MemorySurface: Send + Sync {
    fn read(&self, query: &MemoryQuery) -> MemoryControlResult<MemoryReadOutcome>;
    fn forget(&self, request: &MemoryForgetRequest) -> MemoryControlResult<i64>;
}
```

## 决策 2 — 判定一次：路由只转发，MUST NOT 重跑 §42.3/§42.4

控制面路由的三件事：鉴权、把 HTTP 请求变成 `MemoryQuery`／`MemoryForgetRequest`、
把结果变成 HTTP 响应。**判定本身只在 kernel 一处**（`read_records` / `forget`）。

请求级拒绝 ⇒ HTTP **具名** 4xx。MUST NOT 折成 `200 + {records: []}`——
否则「本次读取不成立」与「读出来零条」不可区分（与 W109 中
`MemoryReadError::Rejected` MUST NOT 折成空读取同一条理由）。

## 决策 3 — 结果三态，故障载体复用 contracts 的 `ErrorCode`

```rust
pub enum MemoryControlResult<T> {
    Ok(T),
    Rejected(MemoryRejection), // §42.3/§42.4 的结论
    Fault(ErrorCode),          // 路径自身的故障
}
```

**不新造故障分类**：`ErrorCode` 已是契约类型，控制面既有 `error(status, code, msg)` 可直接消费。

⚠️ **与 runtime 的 `MemoryReadError` / `MemoryForgetError` 是映射关系，不是第二处定义**：
判定规则只在 kernel 一处，这里是**接口层的传输形状**。映射 MUST 由 `From` 实现，
且 MUST 有测试钉「三态各自可达」（映射不丢维度）——否则 `Fault` 被静默折成 `Rejected` 也没人看见。

## 决策 4 — scope 由控制面填，MUST NOT 由客户端指定

`MemoryScope.workspace_id` / `session_id` 取 `ControlApi` **自身**的绑定（它已持有二者）。
客户端只给 `kinds` / `budget` / `authorization_ref` / `purpose` / `allow_approximate`。

**理由**：作用域是**事实**，不是请求参数。若客户端可指定 `workspace_id`，则「越作用域」
这条判据变成客户端可玩弄的旋钮。

⚠️ **但越作用域判定 MUST 仍可达**：取数 `Store::memory_scoped` **只按 workspace 过滤**
（W109 刻意如此）⇒ 同一工作区里**其他会话**的记录会被取到并进 `refused(out_of_scope)`。
⇒ 必须有用例钉住「能从控制面读到一条属于别会话的记录被具名拒绝」；
否则本轮接线会**静默地让一条判据失去对象**（这是本轮 I2 的注入点，同 W109 的形状）。

## 决策 5 — 权限名新增 `memory.read` / `memory.forget`

⚠️ **这是选择，不是「本来如此」**：复用 `session.read` 也站得住。不复用是因为记忆是敏感数据，
应可**单独**授予与吊销，否则吊销记忆访问就必须连带吊销会话读取。

两层权限**不是同一件事**，故不是「门槛答两次」：
控制面 permission 答「这个控制端身份能不能调记忆端点」（`Access.permissions`，内存+TTL）；
存储层 `session.manage` 答「这个 actor 能不能读这个工作区的记忆」（`permission_grants` 表）。
二者**不同源** ⇒ 各自可达，都不是装饰。

## 决策 6 — `now_ms` 由实现端取

接缝签名**不带**时间。`Composition::read_memory` 内部用 runtime 的 `utc_ms()`。
⇒ 时间源仍只有一处（铁律：MUST NOT 另立第二处时间源）。

## 决策 7 — `MemoryPolicy` 由装配点持有，MUST NOT 由路由挑

否则 §42.3 的 `viewer_memory_enabled` 会被**答两次**（W89 的教训：一条开关答两次就会分家）。
本轮：engine 装配时给定 `MemoryPolicy::default()`，并在文档披露「策略来源是装配点，
本轮不引入配置项」。

## 决策 8 — 只接 read + forget，不接 write

`write_memory` 同样零生产调用方，但它的真实触发者是**记忆的产生者**（§42.1 四类记忆各有产生者），
属领域决策 ⇒ 不在本轮。

⚠️ **如实披露**：因此真实运行下读取面**目前只会读到零条**（库里没有产生者写入的记录）。
本轮验证的是**路径与判定**，不是流量。

## Risks

- **R1 接缝变成「可以什么都不做」的假契约**：若 `MemorySurface` 的实现可以返回 `Ok(空)` 而不触碰
  `Composition`，则接线是装饰。⇒ 缓释：runtime 侧 MUST 有测试证明 `impl` 真的调用了
  `read_memory` / `forget_memory`；控制面侧 MUST 有测试证明路由真的调用了接缝
  （接缝用一个**会记录调用的测试实现**，MUST NOT 用一个恒真的桩）。
- **R2 越作用域判据失去对象**：见决策 4 ⇒ 用例 + 注入 I2 双向钉。
- **R3 故障被折成拒绝**：见决策 3 ⇒ 三态各一例。
- **R4 控制面锁竞争把「忙」说成「不允许」**：`Composition` 在 engine 里是
  `Arc<Mutex<Composition>>` ⇒ 取不到锁必须报**冲突**（与既有 `status()` 的 `try_lock` 同形），
  MUST NOT 报成权限不足。
