# Design: W132 — H17 的 runtime 接线

## 缺口（逐项现测，MUST NOT 由「同族」推断）

| 现测项 | 结果 | 证据 |
|---|---|---|
| `evaluate_degradation` 的生产调用点 | **零** | `grep -rn evaluate_degradation` 只命中 `crates/nai-kernel/tests/harness.rs` |
| `assert_no_free_form` 的生产调用点 | **零** | 同上，只被测试引用 |
| `DegradeVerdict::visible` 的取值 | **恒 `true`** | `evaluate_degradation` 无条件写 `visible: true` |
| `CapabilityView` 有没有承载降级的地方 | **没有** | proto 三条字段：`session_id`/`revision`/`capabilities`；`CapabilityDescriptor` 描述插件能力 |
| `decision_log` 的 kind CHECK | 13 个名字，**无 `degradation`** | `migrations/sqlite/0007-...sql` |
| 本机能否改 proto | **不能** | `cargo xtask schema --check` 需 protoc 3.21.12（工具链前提） |

⇒ 缺口不是「算不出落到哪一级」（W66 已建且有 **24 条**覆盖），而是**那条结论没有 transport**。

## 关键设计决定

1. **`visible` 是构造常量，所以「无静默降级」不能钉在判定面**。判定面没有「不可见」这一格，
   断言它恒真 ⇒ 永远绿。真正会坏的是 transport：判定说可见，但没人把它送到界面 ⇒ 实际不可见。
   ⇒ 本轮把判据放在 transport 的三跳上：渲染（不吞、不改）／落盘（一次、恰为渲染出的内容）／
   读回（只从链上解、解不出就报错）。
2. **`degradation` 判为 critical**。telemetry 写失败会落进遥测缓冲，调用方只看到 `persisted == false`；
   一条能悄悄消失的降级 = 静默降级。这是本轮唯一一处改既有枚举语义的决定，理由写在枚举注释与迁移注释里。
3. **迁移逐列重建，不 `ALTER`**。SQLite 改不了 CHECK。行**逐字复制**（含四个哈希列），
   链不变——`verify_decision_chain` 的前像从不提表名。迁移用例钉三件事：既有行四个哈希列一字未动／
   升级**前** `degradation` 被 CHECK 拒绝（判别性前提，否则「升级后可插入」恒真）／升级**后**可插入。
4. **入参是事实，不是判定**。`record_degradation(failures, available, retained)`：
   `failures` 是「哪些层失败了」，`available` 是「还剩什么可用」，`retained` 是「上下文还留着什么」。
   「落到哪一级」由 `evaluate_degradation` 在**生产路径上**算出。若改成接收 `DegradeVerdict`，
   判定就由调用方给出——那是「注入判定」，runtime 侧将不再有任何东西保证这个映射。
5. **判定先于写入，违规零落盘**。INV-32 后半段（`assert_no_free_form`）在本入口里执行：降级后失去
   策略或人设 ⇒ 具名拒绝，**一个字节都不落**。命名了错误码却继续生成，正是这条要关的缝。
   拒绝侧携带「本来会显示什么」，便于调用方检查，但明确**不落盘**。
6. **读回只从链上解码**。`read_degradation_view` 不接受调用方的答案。不可解析 ⇒ 具名失败
   （MUST NOT 静默跳过：写路径只写过一种形状，其它都是损坏）；空链 ⇒ 报 `0`，不是跳过。
7. **渲染不去重、不合并**。同一层失败两次是两个降级事件。合并会吞掉一次，而「渲染不吞输入」是一条
   可判的事实；界面怎么排版是呈现层的事，不是这里该替它决定的。
8. **无失败 ⇒ 不落盘**。零降级时没有可显示的东西，落一条 `notices: []` 只是噪音。判定**照跑**
   （这正是「未检查」与「已检查为空」的区别），只是不写。
9. **不扩展 `CapabilityView`**。把 harness 层失败映射成 `CapabilityDescriptor` 需要编造
   `capability_id`/`plugin_id`，且 `PluginState` 里没有对应取值——那是在说「一个插件失败了」，
   而事实是「某一层失败了」。不实陈述比缺口更贵 ⇒ 具名登记，等契约与壳体 IPC 到位再接。

## 判据与判据的判别力（先测后设计）

判据首跑捕获 **19 个错误**（`E0432` ×3 / `E0425` ×2 / `E0599` ×14）⇒ 判据真的在测待建之物。

| 守卫 | 坏实现会怎样 | 注入验证 |
|---|---|---|
| 一条判定 ⇒ 一条通知 | 丢最后一条 / 只渲染第一条 | K2（`.take(1)`）红 3 条 |
| 通知字段照抄判定输出 | 把 `actionable` 换成通用串 | K1 红 2 条 |
| 同上（`drops` 分支） | `drops` 恒 `None` | K3 红 1 条 |
| 六层说明互异 | 六层共用一句「已降级」 | K1 同上 |
| 渲染不吞输入 | 按层去重 | K2 同上 |
| 回退后的级，不是首选级 | 写 `preferred_level()` | **K4**：kernel 29/29 **全绿**，runtime 既有 8 条**全绿**，**只有新补的回退用例红** |
| 不可解析 ⇒ 失败 | `unwrap_or_default()` 跳过 | R1 红 1 条 |
| INV-32 在生产路径 | 不跑 `assert_no_free_form` | R2 红 2 条（policy / persona 互不顶替） |
| 域 = `degradation` 那一种 | 去掉 kind 过滤 | R3 红 1 条 |
| 读取形状收紧 | 载荷键写成 `error_code` | R4 红 4 条 |
| `refuses_call` 是派生的 | 恒 `false` | R5 红 1 条 |

**K4 是这一轮的方法论落点**。既有 8 条 runtime 用例**全部**用 `DegradeContext::all_available()`，
在该输入下「首选级」与「回退后的级」**构造性相等** ⇒ 写 `preferred_level()` 的坏实现一条都不会红。
这正是 W131 记的「注入不变红的**第二个方向**：判据根本没有覆盖那一层」——不是被更早的闸门短路，
而是**没有任何既有输入能走到那一层**。处置是**补一条能走到那一层的输入**
（`template_available: false, approved_cache_available: false, human_available: true`），
MUST NOT 改判据去迁就。

**R2 首跑假绿**给出第三种成因：`cfg!(test)` 在生产 lib 里恒为 false，注入没改到任何行为。
⇒ 「注入后全绿」MUST 先怀疑注入自身。
