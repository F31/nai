# 多会话隔离与容量准入

本页说明 A11（V3.10 §30.4，§12.3）在代码里的落点：**Pro 多会话时每会话状态如何互不可见**，以及**资源不足时如何拒绝新会话而不伤在跑的**。实现位于 `crates/nai-kernel/src/multiplex.rs`（kernel 层，只依赖 contracts）。

```bash
cargo xtask test --suite kernel
cargo xtask check --profile portable
```

## 1. 每会话命名空间与独立 epoch

`Multiplexer` 持有以 `session_id` 为键的 `SessionNamespace { session_id, epoch, lanes }`：

- `open(id, epoch)`：重复打开即 `CONFLICT`；会话数上限 `MAX_SESSIONS = 64`。
- `advance_epoch(id)`：**只推进被点的那个会话**。共享一个全局计数器会让一个会话的重启作废另一个会话的工作。
- `bind(id, lane)`：把 lane 绑给它的会话；同一个 lane 在不同会话各绑一次**不冲突**（lane 是每会话的）。

六个 lane 取自 §16 A11 与 §12.3：`Task | Audio | Approval | Budget | Knowledge | Output`。

## 2. lane 访问以属主为键（无串场）

到达一个 lane 的**唯一**入口是：

```rust
access(requester, owner, lane) -> Result<LaneGrant>
    requester != owner  → CONFLICT("lane belongs to another session")
    会话未知 / lane 未绑定 → CAPABILITY_ABSENT
```

跨会话**没有**访问路径——「任务、音频、审批、预算、知识无串场」不是一句要记住的检查，而是签名的性质：lane 只以它的属主为键。**否决**「一张全局 lane 表 + 进入时检查 owner」：检查会被漏写，且漏写不会让测试变红。

## 3. 共享面：三类、只读、有界

可共享的资源是**封闭枚举**：

```rust
enum SharedKind { ImmutableModel, KnowledgeSnapshot, ConnectionPool }
```

活的会话状态（预算、审批、输出 ID）**没有**可共享的表示——没有函数能把一个 lane 变成 `SharedKind`，所以「共享只限三类」是 API 的形状而非评审纪律。

- **只读**：`SharedPool::write` 永远以 `PERMISSION_DENIED("shared resource is read-only")` 拒绝。规格说的是只读知识快照与不可变模型，没有写路径才是「只读」的实现。
- **受控**：`ConnectionPool` 的「受控」由持有者上限落实，满了即 `RESOURCE_EXHAUSTED("shared holder limit")`；上限为零在**构造时**即拒，避免第一次 acquire 才发现池不可用。
- 同一会话重复 acquire 同一 `(kind, key)` 幂等（持有者集合去重）。

## 4. 分族两阶段准入

四族**独立判定**：

```rust
enum AdmissionFamily { GpuThroughput, Vram, AudioTiming, ApiQuota }
```

`admit(session, requests)` 是**两阶段**的：

1. 按族汇总本次请求（同族多条求和）；
2. 逐族对照**全机总量**检查（全场合计用量 + 本次请求 ≤ 上限）；
3. **全部通过**才写入用量。

任一族不足即 `RESOURCE_EXHAUSTED("family capacity exceeded")`，且**不写任何用量**——拒绝不半提交。这直接对应 §12.3「不能让已开播会话 OOM」：一次失败的请求不会在机器上留下任何占用，已开播会话的每一个数字都原封不动。

- 某族有余量**不能**换另一族的余量（「分别准入」）。
- 未声明的族按 **0** 处理（保守读法）；把未声明预算当作不限制，会让一次配置遗漏变成一次静默放行。
- `release(session)` 只释放该会话的份额，其他会话不受影响。

## 5. 隔离审计标记

`audit_isolation(namespaces, shared)` 是对**公开快照值**的纯函数（与 W25 的 `audit_residency` 同形），两条规则：

| 标记 | 含义 |
| --- | --- |
| `shared lease without a holder` | 共享租约没有任何持有者——行为测试看不见的泄漏 |
| `two namespaces share a session id` | 两个命名空间共用一个 session id，会让两个会话共享一个 epoch |

两者都只能由错误产生，所以审计存在的意义是把「注册表不会那样」变成重构后可复查的东西。

## 6. 与 W25 驻留账本的维度区分

| | W25 `Residency`（`deployment-lifecycle`） | 本轮 `Admission`（`multi-session-isolation`） |
| --- | --- | --- |
| 回答 | 能力开不开、驻留占什么 | 会话能不能进、进来占多少配额 |
| 键 | 能力 × 资源族 | 会话 × 配额族 |
| 族 | `Process | Port | Vram | CloudRequest` | `GpuThroughput | Vram | AudioTiming | ApiQuota` |
| 语义 | 声明式持有（`enable` 时物化） | 请求-准入（两阶段检查） |

`Vram` 在两处重合是物理世界的重合（都是显存），不是模型重复：一个能力可以已经驻留（占着显存），而新会话仍因显存配额满被拒。两个层次的事实。

## 7. 未做边界

- 准入用的是**宿主给定的上限表**；本模块不探测真实 GPU/显存/配额。
- `automation: integration` 的真实多路会话（同机并发写实会话、真实音频时序）属设备里程碑，本轮未做。
- 不做跨节点会话路由与多会话 UI。
- kernel 只依赖 contracts，故自带 `MultiplexError{code, reason}`；它与 `nai-upgrade-core` 的 `UpgradeError` 同形，是分层约束下的一处可接受重复。
