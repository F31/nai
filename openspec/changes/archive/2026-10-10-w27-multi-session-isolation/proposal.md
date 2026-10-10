## Why

`tests/registry.yaml` 里 A11 是 `pending`，notes 写着「多会话资源治理未建」。V3.6 把它写在两处，一处是验收口径，一处是设计口径：

| 出处 | 原文 |
| --- | --- |
| §30.4 A11 | Pro 多会话：任务、音频、审批、预算、知识无串场；资源不足时拒绝新增会话 |
| §12.3 | 所有状态以 `session_id` 隔离，Lite 默认一个活动会话。Pro 多会话时，每会话独立 epoch、授权、队列、预算、知识绑定、审批与输出 ID；**共享只限不可变模型、只读知识快照与受控连接池** |
| §12.3 | GPU 吞吐、显存、音频时序、API 配额**分别准入**。新会话容量不足时拒绝或等待，**不能让已开播会话 OOM** |

这三句话合起来是一个**结构性**要求，不是一个功能：要同时做到「每会话私有」「共享面封闭」「准入按族独立且拒绝时不伤在跑的」。

W25 已经在 shared 层建了「能力驻留」的账本（`Residency`/`ResourceKind`），但那回答的是**能力开不开、占不占资源**；A11 回答的是**会话能不能进、进来占多少配额**。两者是不同维度：一个能力可以已经驻留（占着显存），而新会话仍因显存配额满被拒。因此本轮把 A11 落在**内核**（`crates/nai-kernel`）——它只依赖 contracts，而多会话正是内核会话语义（`session.rs` 已经定义了单会话的 epoch 与撤销）。

## What Changes

- **每会话私有命名空间（`Multiplexer` + `SessionNamespace`，A11）**：`open` 给每个会话一个**独立 epoch**，`advance_epoch` 只动被点的那个；`bind` 把六个 lane（`Task | Audio | Approval | Budget | Knowledge | Output`）绑给它的会话。到达一个 lane 的**唯一**入口是 `access(requester, owner, lane)`：requester 不等于 owner 即 `CONFLICT("lane belongs to another session")`。所以「无串场」不是要记住检查的规则，而是**签名的性质**——lane 只以它的属主为键。会话数上限 64。

- **共享白名单是封闭枚举（`SharedKind` + `SharedPool`）**：只有 `ImmutableModel | KnowledgeSnapshot | ConnectionPool` 三类可以被共享。活的 lane 状态（预算、审批、输出 ID）**没有**任何函数可以把它变成可共享资源——「共享只限…」是 API 的形状。共享是**读**：`SharedPool::write` 永远以 `PERMISSION_DENIED("shared resource is read-only")` 拒绝，让「只读快照」可测而不是承诺。`ConnectionPool` 的「受控」由**持有者上限**落实：满了以 `RESOURCE_EXHAUSTED("shared holder limit")` 拒绝。

- **分族准入（`Admission` + `AdmissionFamily`，A11）**：GPU 吞吐、显存、音频时序、API 配额**四族独立判定**。`admit` 是**两阶段**的——先把本次请求按族汇总、逐族对照全机总量检查，**全部通过才写入任何用量**。因此一次拒绝不会半提交：被拒会话的用量为 0，且**已开播会话的每一个数字都原封不动**（「不能让已开播会话 OOM」）。未声明的族按 0 处理（保守读法），`release(session)` 只释放该会话的份额。

- **隔离审计（`audit_isolation`）**：对**公开快照值**的纯函数，检出两类只能由错误产生的状态——「共享租约没有任何持有者」（行为测试看不见的泄漏）与「两个命名空间共用一个 session id」（会让两个会话共享一个 epoch 的合并错误）。

- **不越权声明**：本轮证明的是**隔离与准入的架构判据**，不是「在真机上同时跑过 N 个写实会话」（需要真实 GPU 与多路音频时序，属设备里程碑）。registry 只补证据并如实留痕，`pending → implemented` 且**不标 `tested`**。

## Capabilities
### New Capabilities
- `multi-session-isolation`：每会话私有命名空间与独立 epoch、以属主为键的 lane 访问（无串场）、封闭三类的只读共享白名单与受控连接池、按族独立的两阶段容量准入、隔离审计。覆盖 registry **A11**。

### Modified Capabilities
- 无。既有 `session-leases-and-cancellation`（单会话生命周期、租约、取消）**不改**：本变更在其之上回答「多个会话共存时，状态如何不互相看见、配额如何不被抢空」。

## Impact

- 代码：`crates/nai-kernel/src/` 新增 `multiplex.rs`；`tests/` 新增 `multiplex.rs`（24 项）；`lib.rs` 增 `pub mod multiplex;`。**不新增 crate，不改 workspace 依赖**（维持 18 包与依赖方向；kernel 仍只依赖 contracts）。
- 规格：新增 capability `multi-session-isolation`；新增 `docs/multi-session.md`、ADR 0029、报告 `docs/reports/2026-10-10-w27-multi-session-isolation.md`。
- 依赖：**零新增**。仅用 `nai_contracts::v1::ErrorCode` 与 `std`。
- registry：A11 补证据并按实现 `pending → implemented`（`automation: integration`，真实多路 GPU/音频时序未做，**不标 `tested`**）。
- 明确不做：不接真实 GPU/显存/配额探针（准入用的是宿主给定的上限表）；不做跨节点会话路由（那属轻云与平台接入）；不做多会话的 UI。

## Non-Goals

- 不用隔离断言替代真实并发：本轮证明的是「跨会话访问在类型上不可达、拒绝不半提交」这条**结构性判据**，不宣称任何真实硬件上的并发会话数。
- 不让共享面扩大：活状态（预算/审批/输出 ID）没有可共享的表示，不提供「强制共享」的逃生口。
- 不让拒绝产生副作用：准入是两阶段，拒绝时**不写**任何用量。
