# 平台生命周期与旧房间播报抑制的判定骨架（P01）

代码：`crates/nai-kernel/src/bridge.rs`；测试：`crates/nai-kernel/tests/bridge.rs`；
决策：[ADR 0037](adr/0037-platform-lifecycle-boundary.md)。

本模块把 §27.1「桥接只做」、§347 伴侣生命周期与七类需处理情形、§349 能力快照最严格交集与「缺失能力只关闭对应功能」、§119「平台事件中断，自动互动撤权；手工与回放仍可用」、§1328 P01 验收的**判定口径**落成在 portable 下可执行、可测试的骨架。它**不启动**伴侣、**不加载**官方 SDK、**不建立**任何连接、**不推流**——那些由外部条件（官方 SDK 认证、账户/用途批准、Windows 直播伴侣环境）提供；它负责的是「给定一份**声明的桥接生命周期**，它是否越过职责、转换、授权、能力与降级的边界」。

因此 `tests/registry.yaml` 的 **P01 保持 `blocked`**：§1328 的验收原文是「伴侣启动、退出、重连、切场次不产生旧房间播报；权限撤销停止相关功能」，这些是外部真值。本文档描述的是**判定机制**，不是交付结论。

> **与 D13 已交付部分划界**：D13（`reports/2026-10-08-d13-platform-ingress.md`）交付的是**平台事件准入**——`nai-kernel::platform` 的 `PlatformLink`：可信 root 认证、真实房间来源、完整有效权限快照、单调权限期限、连接代次递增、断线清除权限、重连要求新 revision，以及事件来源/房间/authority/epoch/代次/时间/权限与 TTL 校验。**那条线判的是「一个事件能不能进」**。本模块判的是**桥接进程的生命周期**——同一份规格里另一组判据，此前**全库没有落点**（`grep bridge` 在 `crates/nai-kernel/src/` 内为空）。
>
> 同理与 `nai-kernel::session` 划界：`session.rs` 的 `SessionLifecycle`/`SessionMode`/duty lease/`revoke` 判的是**引擎内的会话与值守租约**；本模块判的是**跨进程的伴侣桥接**。两者互补、不重叠。

## 桥接职责是封闭四项

§27.1：「桥接**只做**：SDK 初始化、官方事件与权限转换、与引擎的认证 IPC、退出。」

| 声明 | 报出 |
| --- | --- |
| 四项之内 | **不报** |
| `extra_duties` 非空（如「生成回复内容」） | `duty_outside_bridge_scope`（**逐条指名**） |

「只做」是一条否定式：不判越界等于没落；不指名则不知道越在哪。

## 四类生命周期转换：各有处置、各清授权、各记录

§1328 原文四类：**伴侣启动、退出、重连、切场次**。

| 缺失 / 违规 | 出处 | 报出（均指名该类） |
| --- | --- | --- |
| 某类转换无处置 | §1328 各有处置 | `transition_undisposed` |
| 处置**不清除**播报授权 | §1328 不产生旧房间播报 | `transition_keeps_stale_broadcast` |
| 处置不写决策日志 | §27.1 桥接职责 | `transition_not_logged` |
| `Exited` 处置**未声明**引擎转待机 | §347 退出时引擎转待机 | `exit_without_standby` |
| `Reconnected` 处置**声明恢复旧自动权限** | §347、§119 重连不恢复旧自动权限 | `reconnect_restores_previous_auto_authority` |

**条件化**：未声明任何桥接职责时**不存在生命周期转换**（一个尚未接入官方 SDK 的桥接不该被读成四个缺陷）。判据以一个**不受门控的独立面**兜住——见下文「陈旧房间广播抑制」。

## 陈旧房间广播抑制：判到授权与连接的绑定

§1328「不产生旧房间播报」不能只判「处置声明了清授权」——清授权之后仍可能**存在**一份与当前连接不符的授权（恢复路径漏挂就是典型）。因此两条都判：

| 面 | 判据 | 报出 |
| --- | --- | --- |
| 处置面 | 某类转换的处置不清除授权 | `transition_keeps_stale_broadcast` |
| 状态面 | 生效授权与当前连接的**房间或连接代次**不一致 | `stale_room_broadcast_permitted` |

`BroadcastAuthority::covers(connection)` 同时比对**房间**与**连接代次**：换房间、或同房间但代次已推进（重连产生新代次），旧授权都不再成立。`BridgeDescriptor::may_broadcast()` 是运行时判据（授权存在且覆盖当前连接）。

**状态面不受「是否声明了桥接」门控**——一个尚未实现的桥接若仍挂着一份已经离开的房间的播报授权，那是实打实的缺陷，不能被「还没做」冲淡。这也是 `Defective` 优先于 `Blocked` 的第一根支点。

## 能力快照取最严格交集，功能开关判两个方向

§349：「平台能力快照取值 granted、denied、unknown，**取最严格交集**。**缺失能力只关闭对应功能**，不自行合成事件。」——这正是「权限撤销停止**相关**功能」的精确口径：不是全停，也不是不停。

| 方向 | 判据 | 报出（均指名） |
| --- | --- | --- |
| 快照读法 | 记录 `unknown` 却**读作已授予** | `unknown_capability_treated_as_granted` |
| 欠关闭 | 功能仍启用，而所需能力**非 granted**（`unknown` 视为未授予） | `feature_active_without_capability` |
| 过关闭 | 功能**因能力变更**被停止，而所需能力**仍 granted** | `feature_stopped_despite_capability` |

「过关闭」的判据带一个**前置条件** `stopped_by_capability_change`：只有**因能力变更**发生的停止才可能与「只关闭对应功能」冲突；用户主动关闭的功能不算。没有这个前置，任何被手动关掉的功能都会被误报。

## 平台中断三态：撤自动、保手工、保回放

§119：「平台事件中断，**自动互动撤权**；**手工与回放仍可用**；重连后不恢复旧自动权限。」

| 处置 | 报出（指名 cause） |
| --- | --- |
| 未撤销自动互动 | `platform_interruption_keeps_auto_reply` |
| 使手工不可用 | `platform_interruption_disables_manual` |
| 使回放不可用 | `platform_interruption_disables_replay` |
| 不写决策日志 | `degradation_not_logged` |

**条件化于 `bridge_lost`**：只有**平台事件中断**的降级才欠这条三段式。Provider 不可用等其他降级不因此报出——否则「降级」二字会被读成「一律撤自动」，与 §21 的 Provider 处置冲突。

## 七类情形各有处置且记录

§347 原文七类：**伴侣多次启动、端口占用、SDK 失联、消息重复、旧房间事件、时间漂移、权限撤销**。

| 缺失 / 违规 | 报出（均指名该类） |
| --- | --- |
| 某类无处置或处置未生效 | `condition_undisposed` |
| 处置不写决策日志 | `condition_not_logged` |

取**原文七类**而非自拟清单——规格说的是哪几类，代码判的就是哪几类，避免悄悄漏掉一类。门控同四类转换。

## 官方 SDK：只经固定路径与握手校验加载

§27.1：「握手检查**路径、签名、nonce 与有效令牌**；官方 SDK 库**不能从任意路径加载**。」§1075 同义重申。

| `SdkAvailability` | 缺任一项握手校验 |
| --- | --- |
| `Verified` | `sdk_loaded_without_trust_check`（**逐项指名** `path_pinned`/`signature`/`nonce`/`token`） |
| `UnverifiedExternalDependency` | **不判**——如实声明没有官方 SDK 时，要求它校验一个并不存在的签名是在惩罚诚实答案 |

## 无官方 SDK 时如实记录，绝不用 fake connector 冒充

§27.1：「没有官方接口文档或 SDK 时返回 `UNVERIFIED_EXTERNAL_DEPENDENCY`；演练走 **manual 与 replay**，**不用 fake connector 冒充真实能力**。」

| 状态 | 冒充真实能力 | 报出 |
| --- | --- | --- |
| `UnverifiedExternalDependency` | 是 | `fake_connector_claimed_as_real_capability` |
| `UnverifiedExternalDependency` | 否（如实记录） | **不报** |

这与 W32「不以 CUDA EP 注册宣称全 GPU」、W33「如实披露中继可读而不宣称端到端加密」同形：**判的是「宣称」，不是「能力」**。`UNVERIFIED_EXTERNAL_DEPENDENCY` 在整个 workspace 此前**从未被使用**；本模块给出它的第一处**可判**落点。

## SDK 回调不同步等待 LLM 或磁盘

§27.1：「SDK 回调**快速入队**，**不同步等待** LLM 或磁盘大事务。」`CallbackPolicy.awaits_llm_or_disk` 为真 ⇒ `callback_blocks_on_llm_or_disk`。门控同四类转换（回调是桥接的行为）。

## readiness 三态，`Defective` 优先

| 条件 | readiness |
| --- | --- |
| 审计非空 | `Defective`（**优先**） |
| 审计为空且未声明桥接职责 | `Blocked`（P01 诚实状态：官方 SDK 与生产宿主尚未实现） |
| 审计为空且已声明桥接职责 | `Ready` |

「尚未接入」不掩盖越界，越界也不被「尚未接入」冲淡。让 `Defective` 优先于 `Blocked` 可被**构造与验证**的支点，是三条**不受门控的独立判据**：陈旧房间授权（状态面）、伪造替代、功能开关双向。

## 审计是描述符的纯函数

`audit_bridge(&BridgeDescriptor) -> Vec<Finding>` 是对描述符自身字段的纯函数，报出**全部**命中项。**十九条**稳定标记：

| 面 | 标记 |
| --- | --- |
| 职责 | `duty_outside_bridge_scope` |
| SDK 与回调 | `sdk_loaded_without_trust_check`、`fake_connector_claimed_as_real_capability`、`callback_blocks_on_llm_or_disk` |
| 生命周期转换 | `transition_undisposed`、`transition_keeps_stale_broadcast`、`transition_not_logged`、`exit_without_standby`、`reconnect_restores_previous_auto_authority` |
| 情形处置 | `condition_undisposed`、`condition_not_logged` |
| 平台中断 | `platform_interruption_keeps_auto_reply`、`platform_interruption_disables_manual`、`platform_interruption_disables_replay`、`degradation_not_logged` |
| 陈旧房间 | `stale_room_broadcast_permitted` |
| 能力与功能 | `unknown_capability_treated_as_granted`、`feature_active_without_capability`、`feature_stopped_despite_capability` |

每条都可在测试里**直接构造**（无需启动伴侣、加载 SDK 或建立连接）；干净描述符审计为空；构造描述的审计并集**恰为**这十九条——既证每条可构造，也防未登记的额外标记。

## 未做边界

- **不启动伴侣、不加载官方 SDK、不建网络连接、不推流、不做真实握手、不做网关与回调服务**。本模块判的是**声明与处置**。
- **不重复 `PlatformLink` 的事件准入判据**（来源/房间/authority/epoch/代次/时间/权限仍在 `platform.rs`），**不改** `platform.rs`/`session.rs`。
- **不替真实平台做验收**：官方 SDK 认证、账户/用途批准、Windows 直播伴侣环境、端口冲突的真实处置无法执行就标 `Blocked`（§1261）。
- **不新增 xtask 命令与证据目录**：§30.1 未为平台生命周期指定证据目录（升级为 `tests/upgrade`、RAG 为 `tests/rag-golden`、UI 为 `tests/ui`）；P01 的真值来自官方 SDK 与真实账号，不是仓库载荷。臆造 `tests/lifecycle` 属过度绑定。待真实平台轨立项后再补载体，**无需改本模块判定**。
