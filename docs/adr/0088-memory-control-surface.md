# ADR 0088：记忆读取面与遗忘落点接上控制面（H03/H04 控制面接线）

- 状态：已接受
- 日期：2026-10-15
- 轮次：W110
- 关联：[ADR 0087](0087-memory-read-surface.md)（H04 runtime 接线，本轮的直接前驱）、
  [ADR 0085](0085-memory-write-gate-wiring.md)（H03 写入面接线）、
  [ADR 0067](0067-viewer-memory-single-answering-point.md)（观众记忆开关的单一应答点）、
  [ADR 0027](0027-trace-id-single-definition.md)（一处定义）

## 背景

W109 闭合了「`revoked` 没有承载列」这个具名开放问题，`Composition::read_memory` 与
`Composition::forget_memory` 由此成为 `read_records` / `forget` 在生产代码里的**第一处**调用方。
但**接缝之上仍零调用方**——这一点是实测的，不是推断（从 `HEAD` 现状查，排除测试）：

```
$ git grep -n "read_memory\|forget_memory\|write_memory" --include=*.rs crates apps | grep -v /tests/
（零命中：只有测试在调）
```

也就是说，今天**无法被判**的命题：

1. 一个本地操作员发来的记忆读取请求，到底有没有真的走到装配根（而不是一个「看起来成功」的空壳）？
2. 一次遗忘请求经控制面走一遍后，读取侧是否**真的**被阻断了（而不是只回了一个 200）？
3. 作用域维度是**控制面自己的绑定**，还是可以被请求自己指定（后者等于把隔离边界交给被隔离者填）？

这与 W103–W109 **连续第七轮同型**（判定面存在、执行体不存在）。**不同的一点**：本轮执行体**已存在**
（W109 建的），缺的是**调用方** ⇒ 本轮是纯接线轮，**零迁移、零契约字段改动**。

## 决策

### 决策 1：接缝定义在 `nai-control-api`，实现在 `nai-runtime`，由 app 注入

`xtask/src/deps.rs` **禁止 shared 依赖 runtime**（`nai-runtime` 反过来依赖 `nai-control-api`）
⇒ 「控制面能调到装配根」这一句在类型上只有一种写法：**接缝在 shared 定义、实现在 runtime、由 app 装配**。

```rust
pub trait MemorySurface: Send + Sync {
    fn read(&self, query: &MemoryQuery) -> MemoryControlResult<MemoryReadOutcome>;
    fn forget(&self, request: &MemoryForgetRequest) -> MemoryControlResult<i64>;
}
```

后端未注入 ⇒ `503 CAPABILITY_NOT_READY`，**MUST NOT** 以空结果假成功（与既有 `approvals` 同形：
「没这能力」与「有这能力但结果为空」不得不可区分）。

### 决策 2：签名**不带** `now_ms` 与 `MemoryPolicy`——两者各只在一处决定

- 时间源只在一处：`utc_ms()` 由**实现端**取。接缝若收 `now_ms`，控制面就成了第二处时间源。
- 策略由**装配点**给定、`MemoryAdapter` 持有。若让路由挑策略，§42.3 的 `viewer_memory_enabled`
  就会被**答两次**（W89 的教训：一条开关答两次就会分家）。

⇒ 实现落在 `MemoryAdapter{root: Arc<Mutex<Composition>>, policy}` 而不是直接 `impl … for Composition`。
适配器**只转发**：判定只在内核一处，控制面与适配器都 MUST NOT 重跑 §42.3／§42.4。

### 决策 3：三态**不丢维度**——`Ok` / `Rejected` / `Fault` 各有自己的 HTTP 形状

| 态 | 载体 | HTTP |
|---|---|---|
| 判定放行 | `Ok(T)` | `200` |
| §42.3／§42.4 拒绝 | `Rejected(MemoryRejection)` | `422` + **判定自己的**稳定名字 |
| 路径故障 | `Fault(ErrorCode)` | 按码映射（忙 ⇒ `409`、未就绪 ⇒ `503`、…） |

⚠️ **请求级拒绝 MUST NOT 折成 `200` 加空 `records`**——否则「本次读取不成立」与「读出来零条」
不可区分，而后者是被**允许**的（ADR 0087 决策 4 的同一条，只是载体从 Rust 错误变成 HTTP 状态）。
⚠️ **故障 MUST NOT 报成判定拒绝**：装配根锁竞争是**冲突**，把它记在「不允许」名下就是把宿主瞬态
写成安全结论（铁律二）。

故障载体**复用契约的 `ErrorCode`**，不新造一套故障分类。

### 决策 4：作用域是**控制面自己的绑定**，请求 MUST NOT 指定

`workspace` / `session` 取自 `ControlApi` 自身；请求体里出现 `workspace_id` / `session_id` 一律**忽略**。
否则一次读取的作用域由被隔离者自己填写，隔离边界形同虚设。

观众维度是**查询**维度（取数边界只按 workspace），故 `viewer_scoped_id` 可由请求给出，缺失即空串。

### 决策 5：读取端点也走 `POST` + body——因为本控制面**一律禁 query**

设计稿原写 `GET /api/v1/memory` + query 参数，实现时改为 `POST /api/v1/memory/read` + JSON body：
本控制面有一条既有测试钉住「**一律禁止 query 参数**」。⇒ **规格已就地同步改 `POST`**（设计服从既有不变量，
不是反过来）。

### 决策 6：「没给输入」≠「空集」；缺省一律取**安全方向**

- `kinds` 缺失 / `null` ⇒ **全部三类**；显式 `[]` ⇒ 空集（零类）。
- `budget` 缺省 `16`、上限 `256`，越界具名失败。
- `trigger` 缺 ⇒ `UserRequest`（本控制面正是它的载体）；`purge_state` 缺 ⇒ `NotStarted`
  （**不**宣称已清除）；`claims_fully_erased` 缺 ⇒ `false`（**不**宣称已擦除）。

### 决策 7：接线成立 MUST 由「调用真的发生了」证明

控制面测试用 `RecordingSurface`——它既记 `reads`/`forgets` 次数，也留下路由构造出的**入参**
（作用域 / 类别 / 配额）。用恒真桩（一律返回空 outcome 且不记账）的话，「接线」可以是装饰：
路由返回 `200` 而**从未**调到后端，测试照样绿。

「装配根忙」用**另一条线程持锁**触发：同线程 `try_lock` 在 Windows 上是未定义形状，MUST NOT 依赖它
（否则判据在一个平台上测的是另一件事）。

### 决策 8：`PurgeState` 补 `ALL`，与 `ForgetTrigger` / `MemoryKind` 对称

路由要遍历在册标签来解析字符串 ⇒ 需要一个可遍历的全集。补上后测试对 `PurgeState::ALL` 与
`ForgetTrigger::ALL` 逐个**回环**（每个在册标签 MUST 被路由认得），加变体而路由没跟上 ⇒ 红。

### 决策 9：H03/H04 的 `status` **仍保持 `pending`**

本轮闭合的是「执行体 → 调用方」，**不是验收达标**。§42.4 的**清除副本**一侧仍未建
（被遗忘的记录**仍在库里**，只是读不出来），Pro 跨会话（§42.5）未覆盖。

## 后果

- **闭合**：`read_memory` / `forget_memory` 从「只有测试在调」变为有两个真实端点的调用方；
  「一次记忆读取到底有没有走到装配根」从「只能读源码回答」变成**可测**。
- **零迁移、零契约字段改动、零 schema 改动**：本轮是纯接线轮（与 W109 那轮必须付一次迁移形成对照）。
- **权限面新增两项**：`memory.read` / `memory.forget`（**单独成权**，为了可以只吊销记忆访问而不连带
  会话读取；复用既有宽权限会让「最小权限」无从表达）。
- **明确留下的缝（如实登记）**：
  - §42.4 的**清除副本**一侧未建 ⇒ 一条被遗忘的记录仍在库里；
  - `MemoryPolicy` 本轮**无配置项**，来源是装配点（默认值：观众长期记忆**默认关闭**）；
  - 端点只到 loopback 控制面，**不含**桌面 UI、不含跨会话（§42.5）。
- **不变**：`read_records` / `forget` / `admit_write` 及全部 kernel 判定规则**一行未改**；
  `read_memory` / `forget_memory` 的语义一行未改；H03/H04 的 registry `status` **保持 `pending`**。

## 替代方案

1. **接缝定义在 runtime、由 control-api 反向依赖** —— 违反分层（`xtask/src/deps.rs` 禁止）。
2. **在控制面里再判一次 §42.3** —— 一条门槛答两次会分家（W88/W89）；判定只在一处。
3. **接缝带 `now_ms`** —— 引入第二处时间源（铁律一：断言顺序 MUST 由构造保证，不得靠墙钟）。
4. **接缝带 `MemoryPolicy`（由路由挑）** —— `viewer_memory_enabled` 被答两次（决策 2）。
5. **`GET` + query 参数** —— 本控制面有测试钉住「一律禁 query」（决策 5）。
6. **请求级拒绝折成 `200` + 空 `records`** —— 与「读出来零条」不可区分（决策 3）。
7. **故障折成 `403`** —— 把锁竞争记在「没有权限」名下，宿主瞬态写成安全结论（决策 3）。
8. **作用域由请求指定** —— 隔离边界交给被隔离者填（决策 4）。
9. **用恒真桩做测试实现** —— 接线可以是装饰（决策 7）。
10. **顺手做 §42.4 的清除副本** —— 需要副本清单与清理执行体，混进来会让「阻断已完成」与
    「清除已完成」重新不可区分。

## 关联

- 实现：`crates/nai-control-api/src/lib.rs`（`MemorySurface` / `MemoryControlResult` /
  `MemoryForgetRequest` / 两个路由）、`crates/nai-runtime/src/memory.rs`（`MemoryAdapter`）、
  `crates/nai-kernel/src/evaluate/memory.rs`（`PurgeState::ALL`）、`apps/engine/src/lib.rs`（注入）
- 测试：`crates/nai-control-api/src/tests.rs`（**26** 项，W110 新增 **11**）、
  `crates/nai-runtime/tests/memory.rs`（**23** 项，W110 新增 **5**）
- 规格：`openspec/specs/local-control-surface/spec.md`（W110 新增 4 条 Requirement）
- 报告：`docs/reports/2026-10-15-w110-memory-control-surface.md`
- 活文档：`docs/runtime.md`、`docs/kernel.md`、`docs/development-plan.md`
- 登记：`tests/registry.yaml`（H03 / H04 追加 W110 段；**两条 `status` 未改**）
