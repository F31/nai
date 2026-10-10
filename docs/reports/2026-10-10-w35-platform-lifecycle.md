# W35 实测报告：平台生命周期判定骨架（P01 前置解耦）

- 日期：2026-10-10
- 变更：`openspec/changes/w35-platform-lifecycle/`
- 决策：[ADR 0037](../adr/0037-platform-lifecycle-boundary.md)；指南：[docs/platform-lifecycle.md](../platform-lifecycle.md)
- 判据：`tests/registry.yaml` P01

## 结论

P01（平台生命周期）的**真值**（官方 SDK 认证、生产 composition root 平台宿主接线、真实启动/退出/端口冲突与网关）需要外部条件，
本轮不可达，P01 **保持 `blocked`**——其 registry notes 此前只有一句「官方 SDK/生产宿主未实现（D13）」。

D13 已交付的是**平台事件准入**——`nai-kernel::platform` 的 `PlatformLink`：可信 root 认证、真实房间来源、完整有效权限快照、
单调权限期限、连接代次递增、断线清除权限、重连要求新 revision，以及事件来源/房间/authority/epoch/代次/时间/权限与 TTL 校验。
**那条线判的是「一个事件能不能进」**。本轮补的是同一份规格里**另一个面**——**桥接进程的生命周期**，此前**全库没有落点**
（`grep bridge` 在 `crates/nai-kernel/src/` 内为空）。§27.1/§347/§349/§119/§1328 给了一组很密的判据，于是「平台生命周期」在执行层
等于一句**无从判定**的要求。本轮把口径落进 `crates/nai-kernel/src/bridge.rs`，几处最容易含糊的地方被钉死：

1. **桥接职责是封闭四项**（§27.1「桥接只做：SDK 初始化、官方事件与权限转换、与引擎的认证 IPC、退出」），越界职责**逐条指名**
   （`duty_outside_bridge_scope`）——「只做」是一条否定式，不判越界等于没落。
2. **四类生命周期转换各有处置、各清播报授权、各写决策日志**（§1328）——**未声明桥接职责时不存在转换**，故条件化。
3. **「不产生旧房间播报」判两个面**（§1328）：处置面 `transition_keeps_stale_broadcast` + 状态面 `stale_room_broadcast_permitted`。
   授权绑定**房间与连接代次**（`covers` 两项都比），`may_broadcast()` 是运行时判据。**状态面不受门控**——尚未实现的桥接若仍挂着
   已离开房间的播报授权，那是实打实的缺陷。
4. **退出转待机与重连不恢复旧自动权限都落在转换处置的声明上**（§347、§119）。**否决**描述符级另设「当前引擎状态」+「最近转换」——
   那会把「运行中的桥接也带着退出处置声明」误读成「已退出却仍在运行」。
5. **能力快照取最严格交集，功能开关判两个方向**（§349）：`unknown` 视为未授予、读作已授予即报出；欠关闭
   `feature_active_without_capability` 与**因能力变更**过关闭 `feature_stopped_despite_capability`（后者带 `stopped_by_capability_change`
   前置，避免误报用户主动关闭的功能）。这正是「权限撤销停止**相关**功能」的精确口径：不是全停，也不是不停。
6. **平台中断三态**（§119）：撤**自动**、保**手工**、保**回放**，四条判据条件化于 `bridge_lost`——Provider 不可用等其他降级不欠此规则。
7. **七类情形取 §347 原文七类**（多次启动/端口占用/SDK 失联/消息重复/旧房间事件/时间漂移/权限撤销）各有处置与记录，避免自拟清单悄悄漏类。
8. **官方 SDK 只经固定路径与握手校验加载**（§27.1、§1075），条件化于「声明已核验」；**无官方 SDK 时如实记
   `UNVERIFIED_EXTERNAL_DEPENDENCY`、绝不用 fake connector 冒充**（§27.1 末句；该错误码此前在 workspace 内**从未被使用**）；
   回调不同步等待 LLM 或磁盘。
9. **`readiness` 三态且先取审计**（`Defective` 优先）；让「优先」可被构造与验证的支点，是三条**不受门控**的独立判据：陈旧房间授权、
   伪造替代、功能开关双向。

**本轮不产生任何真实交付**：伴侣进程、官方 SDK 加载、握手、网络连接、网关与推流都不在此仓库生成。
**不新增 xtask 命令与证据目录**：§30.1 为升级指定 `tests/upgrade`、为 RAG 黄金集指定 `tests/rag-golden`、为 UI 指定 `tests/ui`；
平台生命周期**没有**证据目录约定，其真值来自官方 SDK 与真实账号，不是仓库载荷；臆造 `tests/lifecycle` 属过度绑定。

## 目标行为

| 面 | 判据 | 稳定标记 |
| --- | --- | --- |
| 职责封闭 | 四种之内不报；`extra_duties` 逐条指名 | `duty_outside_bridge_scope` |
| SDK 握手 | `Verified` 时缺路径固定/签名/nonce/令牌逐项报出（门控） | `sdk_loaded_without_trust_check` |
| SDK 诚实性 | 无官方 SDK 而冒充真实能力 | `fake_connector_claimed_as_real_capability` |
| 回调 | 同步等待 LLM 或磁盘 | `callback_blocks_on_llm_or_disk` |
| 转换处置 | 四类缺一类即报 / 不清授权 / 不记录（门控） | `transition_undisposed`、`transition_keeps_stale_broadcast`、`transition_not_logged` |
| 退出 | 退出处置未声明引擎转待机 | `exit_without_standby` |
| 重连 | 重连处置声明恢复旧自动权限 | `reconnect_restores_previous_auto_authority` |
| 陈旧房间 | 生效授权与当前连接的房间或代次不一致（**不受门控**） | `stale_room_broadcast_permitted` |
| 能力快照 | `unknown` 被读作已授予（**不受门控**） | `unknown_capability_treated_as_granted` |
| 功能开关 | 功能开启而所需能力非 granted / 因能力变更停止而能力仍 granted（**不受门控**） | `feature_active_without_capability`、`feature_stopped_despite_capability` |
| 平台中断 | 未撤自动 / 停手工 / 停回放 / 不记录（`bridge_lost` 时，门控） | `platform_interruption_keeps_auto_reply`、`platform_interruption_disables_manual`、`platform_interruption_disables_replay`、`degradation_not_logged` |
| 七类情形 | 缺一类处置 / 不记录（门控） | `condition_undisposed`、`condition_not_logged` |

共**十九条**稳定标记，全部可在测试里直接构造（无需启动伴侣、加载 SDK 或建立连接）。

## 测试

`crates/nai-kernel/tests/bridge.rs`（**44 项**）：

- **职责**：四项之内不报；越界逐条指名。
- **SDK 握手**：`Verified` 缺四项逐项报出（`nonce`/`path_pinned`/`signature`/`token`）；齐备不报；`Unverified` 不判握手检查。
- **SDK 诚实性**：无官方 SDK 而冒充报出；如实记录不报。
- **回调**：同步等待报出；只入队不报。
- **转换**：四类齐备不报；缺 `room_switched` 指名报出；不清授权（`reconnected`）报出；不记录（`exited`）报出；**未声明桥接不报转换缺陷**；
  重连恢复旧自动权限报出。
- **陈旧房间**：换房间报出且 `may_broadcast` 为假；同房间旧代次报出且不可播报；一致时不报且可播报；无授权不报且不可播报。
- **退出**：退出处置未声明待机报出并指名；声明待机不报；缺退出处置只报 `transition_undisposed`（**同一处缺陷不重复计**）。
- **能力与功能**：`unknown` 读作已授予报出；严格读法不报；功能开启而能力 `denied`/`unknown` 报出；因能力变更停止而能力仍 granted 报出；
  开启且 granted 不报；用户主动关闭（非能力变更）不报。
- **平台中断**：未撤自动报出；停手工与停回放报出；不记录报出；三态齐备不报；**非 `bridge_lost` 降级不报**。
- **七类情形**：齐备不报；缺 `time_drift` 指名报出；`handled=false` 报出；不记录（`permission_revoked`）报出；未声明桥接不报。
- **readiness**：空描述符 `Blocked` 且审计为空；空描述符 + 陈旧房间授权 ⇒ `Defective`（**优先于 `Blocked`**）；空描述符 + 伪造替代 ⇒ `Defective`；
  齐备描述符 `Ready`。
- **审计纯函数**：干净描述符审计为空；构造描述（verified 路径 + unverified 路径，两者互斥）的审计并集**恰为**十九条。

## 实跑

```
cargo test -p nai-kernel --offline --test bridge
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

cargo xtask check --registry
admission registry check passed (V3.6 acceptance + D01-D14 mapping)

cargo xtask check-deps
dependency boundaries passed: 18 workspace packages (normal/build/dev/target declarations)

cargo clippy -p nai-kernel --all-targets -- -D warnings
Finished `dev` profile ... (clean)

cargo xtask check --profile portable
EXIT=0；82 个测试目标（65 Running + 17 Doc-tests）；667 passed / 0 failed；LNK=0
portable: engineering foundation checks passed; device/SDK/GPU/install certification not performed
```

目标数 81→82（新增 `tests/bridge.rs` 一个目标）；通过项 623→667（+44）。

## 依赖边界

- 落 `crates/nai-kernel/src/bridge.rs`：kernel 只依赖 contracts，**18 包与依赖方向不变**。
- **零新增依赖**：仅用 `std`。复用 `golden::Finding`（`Finding::new` 已是 `pub`），**不新增令牌类型、不复制实现**。
- `lib.rs` 仅 +1 行（`pub mod bridge;`，字母序插在 `device` 之前）。
- **不改** `platform.rs`（事件准入，另一面）、**不改** `session.rs`（会话状态机与值守租约，另一实体）。
- `Cargo.lock` 无改动。

## 未覆盖边界

- **不启动伴侣、不加载官方 SDK、不建网络连接、不推流、不做真实握手、不做网关与回调服务**。本模块判的是**声明与处置**。
- **不重复 `PlatformLink` 的事件准入判据**：来源/房间/authority/epoch/代次/时间/权限仍在 `platform.rs`。
- **不替真实平台做验收**：官方 SDK 认证、账户/用途批准、Windows 直播伴侣环境、端口冲突的真实处置无法执行就标 `Blocked`（§1261）。
- **不新增 xtask 命令与证据目录**：§30.1 未为平台生命周期指定证据目录；P01 的真值来自官方 SDK 与真实账号，不是仓库载荷。
  待真实平台轨立项后再补载体，**无需改本模块判定**。
- registry **P01 保持 `blocked`**；notes 已补记 W35 交付的判定机制、**与 D13 已交付的事件准入**的边界，以及两条验收口径。
