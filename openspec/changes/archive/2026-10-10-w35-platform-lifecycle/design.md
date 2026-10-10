# 设计：平台生命周期与旧房间播报抑制的判定边界

- 基线：V3.7 §3.3（进程拓扑：platform-bridge）、§9.2（桥接与能力快照）、§27.1（SDK 桥接）、§347（伴侣生命周期与七类需处理）、§349（能力快照最严格交集）、§119（平台事件中断三态）、§1075（SDK 加载与握手）、§1261（交付与 CI）、§30.1（测试分层）、§1328（P01 验收）
- 判据：`tests/registry.yaml` P01
- 承接：ADR 0034（写实准入 Gate）、ADR 0035（手机监控面）、ADR 0036（设备接入）

## 背景

P01（平台生命周期）的真值（官方 SDK、生产 composition root 平台宿主接线、真实启动/退出/端口冲突/网关）需要外部条件，本轮不可达。D13 已交付 `nai-kernel::platform::PlatformLink`，但那判的是**事件准入**（一个事件能不能进），不是**桥接进程的生命周期**。§27.1/§347/§349/§119 给了一组很密的判据，此前全库没有落点。要把它们落成代码，必须先回答：

1. 落进哪个 crate，与既有 `platform.rs`/`session.rs` 怎么划界？
2. 「桥接只做四项」怎么表达，越界怎么报？
3. 四类生命周期转换的「明确处置」怎么表达，「清授权」怎么判？
4. 「不产生旧房间播报」判到哪一层？
5. 「引擎转待机」与「重连不恢复旧自动权限」怎么判？
6. 能力快照三态怎么取最严格，「只关闭对应功能」怎么判到**两个方向**？
7. 平台中断的三态（撤自动/保手工/保回放）怎么判，非平台中断怎么办？
8. 七类情形怎么表达与检查？
9. SDK 信任检查与「无 SDK 时不冒充」怎么共存？
10. 未接入官方 SDK 时 readiness 是什么？
11. 审计会不会被判定函数架空？

## 决策

### D1 落 `crates/nai-kernel`（`bridge.rs`），与 `platform.rs`/`session.rs` 划界

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 并进既有 `platform.rs` | 与 `PlatformLink` 同文件 | **否决**：`PlatformLink` 判**事件准入**（来源/房间/authority/epoch/代次/时间/权限），本模块判**进程生命周期与播报抑制**；混在一起会让「准入」与「生命周期」互相污染，且 `platform.rs` 已有自己的 `Connection`/`fail` 词汇。 |
| B. 并进既有 `session.rs` | 与会话状态机同文件 | **否决**：`session.rs` 的 `SessionLifecycle`/`SessionMode`/duty lease 是**会话**面（引擎内的会话与值守租约），P01 是**伴侣桥接**面（跨进程生命周期）；两者是不同的实体与不同的不变量。 |
| C. 放进 `nai-plugin-hosts`（桥接是宿主） | 贴近宿主 | **否决**：plugin-hosts 判**IPC/ACL/边界**，且会把 kernel 的报告词汇（`Finding`）拖进宿主层；桥接生命周期的判定策略应与 kernel 其余判定骨架同住。 |
| D. 新增 `crates/nai-kernel` 的 `bridge.rs` | 与 `golden.rs`/`realistic.rs`/`monitoring.rs`/`device.rs` 同 crate | **采纳**：复用 `golden::Finding`（同一报告词汇），依赖方向不变（kernel 只依赖 contracts）。 |

> §119「platform-bridge（C++）」说的是**运行时的进程与语言**，与「判定策略放 kernel crate」不矛盾：kernel 判的是**声明与处置**，不启动伴侣、不加载 SDK、不建连接。

### D2 桥接职责是**封闭四项**，越界逐条指名

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 只记「职责清单」不判越界 | 一个列表 | **否决**：§27.1 的「只做」是一条**否定式**，不判越界等于没落。 |
| B. 只报「有越界」不指名 | 一个布尔 | **否决**：不知道越在哪，修法不同。 |
| C. `duties` 四项之内 + `extra_duties` 逐条指名报出 | 封闭集 + 逐条 | **采纳**：`duty_outside_bridge_scope`。 |

### D3 四类转换各清授权，但以「声明了桥接职责」为前提

`BridgeTransition::{Started, Exited, Reconnected, RoomSwitched}`（§1328 原文四类）；`TransitionDisposition{clears_broadcast_authority, logged, leaves_engine_in_standby, restores_previous_auto_authority}`。缺某类 ⇒ `transition_undisposed`；不清授权 ⇒ `transition_keeps_stale_broadcast`；不记录 ⇒ `transition_not_logged`。

**条件化**：未声明任何桥接职责时**不存在生命周期转换**，本项 MUST NOT 产生缺陷——否则 P01 尚未实现官方 SDK 的诚实状态会被读成四个缺陷，与 D12「无声明应是 `Blocked`」冲突。这是把「不适用」读成「不合规」的又一例（同 W33 预览未启用、W34 纯 `Sense` 不要求联锁）。

### D4 「不产生旧房间播报」判到**授权与连接的绑定**，不只判「处置了」

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 只判「每类转换都清授权」 | 单一布尔 | **否决**：清授权之后仍可能**存在**一份与当前连接不符的授权（例如恢复路径漏挂）；只判处置声明会漏掉真正会播报的那种状态。 |
| B. 只判描述符级授权是否一致 | 单一状态 | **否决**：会漏掉「某类转换根本没清授权」这类**处置缺陷**。 |
| C. 两条都判：处置面 `transition_keeps_stale_broadcast` + 状态面 `stale_room_broadcast_permitted` | 双角度 | **采纳**：与 W34 的 `device_event_undisposed`（处置面）与 `duplicate_event_opens_second_capture_path`（结果面）同形。`BroadcastAuthority::covers(connection)` 比对**房间与连接代次**；`may_broadcast()` 是运行时判据。 |

### D5 退出转待机、重连不恢复旧自动权限

§347「伴侣退出时桥接退出，引擎转待机；重连不恢复旧自动权限」合成两条独立判据，且**两条都落在转换处置的声明上**（审计是描述符的纯函数，判的是「处置是否声明做到」而非另设一个全局状态字段）：`Exited` 处置 `leaves_engine_in_standby` 为假 ⇒ `exit_without_standby`；`Reconnected` 处置 `restores_previous_auto_authority` 为真 ⇒ `reconnect_restores_previous_auto_authority`（§119 同义重申）。

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 描述符级另设「当前引擎状态」+「最近一次转换」 | 判实际状态 | **否决**：把「运行中的桥接也带着退出处置声明」误读成「已退出却仍在运行」；要修得再引入一个「最近转换」字段，得不偿失。 |
| B. 只判「退出处置存在」 | 过松 | **否决**：判不出「退出时转没转待机」。 |
| C. 在**转换处置**里声明引擎结果与是否恢复旧自动权限 | 局部声明 | **采纳**：与 `clears_broadcast_authority` 同构，两点都可直接构造。 |

### D6 能力快照取**最严格交集**，功能开关判**两个方向**

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 只判「功能不该开着却开着」 | 单向 | **否决**：§349「**只**关闭对应功能」的另一半是**不得**越界关闭；只判单向等于允许「撤销 A 权限顺手停了 B 功能」。 |
| B. 让 `unknown` 参与「已授予」判定 | 宽松 | **否决**：§349 明说取**最严格交集**，`unknown` 必须视为**未授予**——这正是「不知道」不能被读成「可以」。 |
| C. `unknown` 视为未授予；`unknown` 被读作已授予 ⇒ 报出；功能开着而所需能力非 granted ⇒ 报出；功能**因能力变更**被停止而所需能力仍 granted ⇒ 报出 | 三态 + 双向 | **采纳**。第三条的**条件**（`stopped_by_capability_change`）是为了不误报「用户主动关闭的功能」——只有**因能力变更**发生的停止才可能与「只关闭对应功能」冲突。 |

### D7 平台中断三态（撤自动 / 保手工 / 保回放），条件化于「确为平台中断」

§119「平台事件中断，自动互动撤权；手工与回放仍可用」是一条**三态否定式**：撤**自动**、保**手工**、保**回放**。`Degradation{bridge_lost, auto_reply_revoked, manual_available, replay_available, logged}`；仅当 `bridge_lost` 为真时判四条（三条状态 + 记录）。**非平台中断的降级**（如 Provider 不可用）MUST NOT 因此报出——否则「降级」二字会被读成一律要撤自动，与 §21 的 Provider 处置冲突。这是**条件化受检**的又一例（同 D3、D9）。

### D8 七类情形各有处置且记录

`BridgeCondition` 恰取 §347 原文七类：**多次启动、端口占用、SDK 失联、消息重复、旧房间事件、时间漂移、权限撤销**。`ConditionDisposition{condition, handled, logged}`。缺某类 ⇒ `condition_undisposed`；不记录 ⇒ `condition_not_logged`。取**原文七类**而非自拟清单，是为了「规格说的是哪几类，代码判的就是哪几类」，避免悄悄漏掉一类。门控同 D3。

### D9 SDK 信任检查条件化于「声明 SDK 已核验」

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 无条件要求全部握手校验 | 一刀切 | **否决**：会与 D10 冲突——如实声明**没有**官方 SDK 时，不该同时被要求「校验签名」。 |
| B. 都不要求 | 宽松 | **否决**：§27.1「官方 SDK 库不能从任意路径加载」是硬要求。 |
| C. `SdkAvailability::Verified` 时逐项检查路径固定/签名/nonce/有效令牌；`UnverifiedExternalDependency` 时不判 | 条件化 | **采纳**：`sdk_loaded_without_trust_check` 逐项指名。 |

### D10 `UNVERIFIED_EXTERNAL_DEPENDENCY` 是**诚实状态**，冒充才是缺陷

§27.1「没有官方接口文档或 SDK 时返回 `UNVERIFIED_EXTERNAL_DEPENDENCY`；演练走 manual 与 replay，**不用 fake connector 冒充真实能力**」。落成：`SdkAvailability::{Verified, UnverifiedExternalDependency}` + `uses_fake_connector_claimed_real`。该状态下冒充 ⇒ `fake_connector_claimed_as_real_capability`；**如实记录则不报**。这与 W32「不以 CUDA EP 注册宣称全 GPU」、W33「如实披露中继可读而不宣称端到端加密」同形：**判的是「宣称」，不是「能力」**。

### D11 SDK 回调不同步等待 LLM 或磁盘

§27.1「SDK 回调快速入队，不同步等待 LLM 或磁盘大事务」。`CallbackPolicy{enqueues_without_blocking, awaits_llm_or_disk}`；`awaits_llm_or_disk` ⇒ `callback_blocks_on_llm_or_disk`。门控同 D3（回调是桥接的行为）。

### D12 readiness 三态，`Defective` **优先**于 `Blocked`

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 无声明即缺陷 | 一律 Defective | **否决**：P01 尚未实现官方 SDK 是**计划内**的诚实状态（§1261「无法执行就标 Blocked，不写成 CI Passed」）。 |
| B. 无声明即 Ready | 乐观 | **否决**：会把「尚未接入」读成「已就绪」。 |
| C. `Blocked`/`Defective`/`Ready`，**先取审计**（非空即 `Defective`），再按是否声明了桥接职责（无 ⇒ `Blocked`）否则 `Ready` | 审计优先 | **采纳**。让 `Defective` 优先于 `Blocked` 的支点是**不受门控的独立判据**：陈旧房间播报（D4 状态面）、伪造替代（D10）、功能开关双向（D6）在空职责描述上仍可构造。 |

### D13 审计是描述符的纯函数

与 W30/W32/W33/W34 一致：对描述符自身字段做纯函数，报出**全部**命中项，**十九条**标记逐条可在测试里直接构造；干净描述符审计为空；最大违规描述符的审计并集**恰为**这十九条（既证每条可构造，也防未登记标记）。

### D14 不新增 xtask 命令与证据目录

§30.1 未为平台生命周期指定证据目录（升级为 `tests/upgrade`、RAG 为 `tests/rag-golden`、UI 为 `tests/ui`）。P01 的真值来自**官方 SDK 认证、账户/用途批准与 Windows 直播伴侣环境**，不是仓库载荷。**臆造** `tests/lifecycle` 属过度绑定。待真实平台轨立项后再补载体，届时**无需改本模块判定**。

## 验证

- `cargo test -p nai-kernel --lib` 与 `cargo test -p nai-kernel --test bridge`（新增用例）。
- `cargo xtask check --profile portable`（含 fmt、workspace clippy、全量测试）通过。
- `cargo xtask check-deps` 仍 18 包、方向不变。
- OpenSpec `validate --all --strict --archived` 全通过。

## 环境边界

判定骨架在 portable（Linux/WSL 逻辑层）可测，测的是**口径**：一份桥接生命周期描述如何判职责越界、四类转换如何判处置与清授权、旧房间如何被抑制播报、退出如何转待机、重连如何不恢复旧自动权限、能力快照如何取最严格交集、功能开关如何双向判定、平台中断如何撤自动而保手工与回放、七类情形如何判处置、SDK 如何判信任与不冒充、尚未接入如何表达。**不含**任何伴侣进程、官方 SDK 加载、网络连接、握手、网关、推流与真实账号；因此 P01 保持 `blocked`，本变更不升级其状态。
