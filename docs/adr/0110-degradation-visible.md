# ADR 0110：H17 的 runtime 接线——把「层失败」送到界面

- 状态：已接受（W132，2026-10-17）
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`
- 相关：ADR 0109（W131）、ADR 0105（W127）、ADR 0087（W109）、INV-32、§12.2、§47.4、§48.5

## 背景

`tests/registry.yaml` 的 `H17` notes 逐字写着「W66 已落判定面……**仍未接线**」。本轮开工前现测，
这句话是真的：`evaluate_degradation` 与 `assert_no_free_form` 在 `crates/` 与 `apps/` 里
**只被测试引用**，生产代码零调用点。

更关键的一点是**判定面自己答不了「无静默降级」**：`DegradeVerdict::visible` 是
`evaluate_degradation` 无条件写入的 `true`——判定面没有「不可见」这一格，所以断言它恒真、永远绿。
真正会坏的是**判定到界面之间的 transport**。本轮交付这段 transport。

## 决定

1. **「无静默降级」的判据放在 transport 上，不放在判定面**。判定面的 `visible` 是构造性事实，
   钉它是装饰。本轮钉三跳：渲染（不吞、不改）／落盘（一次、恰为渲染出的内容）／读回（只从链上解）。
2. **新增呈现层承载物 `DegradationNotice` + `render_degradation_notices`**（kernel）。
   一条判定 ⇒ 恰好一条通知；字段**照抄**判定输出；**不去重、不合并**；六层的可行动说明互异。
3. **新增 `DecisionKind::Degradation`，并判为 critical**。telemetry 写失败会落进遥测缓冲
   （调用方只看到 `persisted == false`），一条能悄悄消失的降级正是 INV-32 要禁的静默降级。
4. **新增迁移 `0011-decision-log-degradation-kind.sql`**，逐列重建 `decision_log`、逐字复制行
   （含四个哈希列）、重建两个只追加触发器。SQLite 改不了 CHECK；链不变，因为前像从不提表名。
   枚举与 CHECK 是同一闭集的两处，MUST 同态（W104 已踩过一次只改一处的缝）。
5. **`record_degradation` 的入参是事实，不是判定**。事实 = 哪些层失败了 / 还剩什么可用 /
   上下文还留着什么；「落到哪一级」由 `evaluate_degradation` 在**生产路径上**算出。
   接收判定结果 ⇒ runtime 侧不再有任何东西保证这个映射（那是「注入判定」）。
6. **判定先于写入，违规零落盘**。INV-32 后半段在本入口执行：降级后失去策略或人设 ⇒ 具名拒绝，
   **一个字节都不落**。拒绝侧携带「本来会显示什么」便于检查，但明确不落盘。
7. **读回只从链上解码**。不可解析 ⇒ 具名失败（MUST NOT 静默跳过）；空链 ⇒ 报 `0`；
   域 = `degradation` 那一种。
8. **渲染不去重、不合并**。同一层失败两次是两个降级事件；合并会吞掉一次。
   界面怎么排版是呈现层的事，不是这里该替它决定的。
9. **无失败 ⇒ 不落盘**。零降级没有可显示的东西；判定照跑（这是「未检查」与「检查过为空」的区别），只是不写。
10. **不扩展 `CapabilityView` 契约**。`CapabilityDescriptor` 描述**插件能力**
    （`capability_id`/`plugin_id`/`PluginState`），把 harness 的层失败映射进去需要编造这些字段，
    且 `PluginState` 没有对应取值——那是在说「一个插件失败了」，而事实是「某一层失败了」。
    不实陈述比缺口更贵。另：改 proto 需 protoc 3.21.12 再生，本机无（工具链前提）；§46.5 也写明本轮不新增契约。
11. **不新增 INV-32 的违规类**。两类的定义已由 W66 给出，本轮只把它们放到生产路径上执行。
12. **`status` 仍 `pending`**。四环：①判定面（W66 已建）②执行体（本轮新建）③调用方（呈现层，未闭合）
    ④真实产生者（真实界面与真实模型，未接线）。与 H15/H01/H02 接线轮惯例一致：接线到 runtime ≠ 验收达标。
13. **版本号成套六处同步 10 → 11**：`migration::VERSION` / `SQL` 数组 + `include_str!` /
    `backup.schema.json` const / Rust 契约样本断言 / `schema.test.mjs` 备份样本（含「上一版 MUST 被拒」+1）/
    TS 生成物 `packages/contracts-ts/src/generated/config/backup.ts`。

## 后果

- 正：一次降级是否可见，现在可以由**链上读回**证明，而不是靠读代码。
- 正：INV-32 后半段第一次在生产路径上执行；此前它只在测试里被引用。
- 负：`degradation` 是 critical ⇒ 写入失败会变成**硬错误**而不是缓冲。频繁的工具失败会把
  写失败暴露在热路径上。这个代价是刻意付的：能悄悄消失的降级比一次响亮的失败更贵。
- 负：真实界面仍未接线（第 10 条），H17 的验收差距只缩小了**两环**，没有闭合。

## 具名登记（发现但不修）

1. **真实界面仍未接线**：本轮交付的是「清单存在且可被界面读到」。CapabilityView / UI 渲染需要
   契约扩展 + 壳体 IPC，两者都未建。
2. **关键种类的清单是手写的**：`crates/nai-storage/src/tests/decision_log.rs` 的
   `critical_failures_surface_and_never_enter_the_buffer` 逐字列出四个 `DecisionKind`。本轮新增第五个
   critical 种类而该用例**没有变红**——它不声明关键种类的闭世界，所以「新增一个 critical 种类」在那里无信号。
   本轮不改（会动到既有判据的形状）。
3. **`required_str` 在 `assembly.rs` 与 `degradation.rs` 各有一份**：合并要改 `assembly.rs` 的
   10 处调用点，属本轮范围外；两份的消息各自指名自己的行形状。
4. **期限敏感断言在 CPU 争抢下误报**：`nai-storage --lib` 默认并行度下
   `corrupted_database_artifact_and_traversal_never_activate_destination` 报
   `DeadlineExceeded / operation stopped`，`RUST_TEST_THREADS=4` 下 **82/82 全绿**。W84 同族，根因未修。
5. **R2 注入首跑假绿**：写成 `!cfg!(test)` 时 9/9 全绿，因为 `cfg!(test)` 在**生产 lib**里恒 false。
   ⇒ 「注入不变红」的**第三种成因**：不是被短路、不是无覆盖，而是**注入自身没改到行为**。
   「注入后全绿」MUST 先怀疑注入，MUST NOT 直接记作「判据无判别力」。

## 与既有 ADR 的关系

- ADR 0109（W131）：「注入不变红的第二个方向 = 判据根本没有覆盖那一层」。本轮 **K4 再实证**：
  既有 8 条 runtime 用例全用 `all_available()`，在该输入下「首选级」与「回退后的级」构造性相等
  ⇒ 写 `preferred_level()` 的坏实现让 kernel 29/29 **全绿**、runtime 既有 8 条**全绿**，
  **只有本轮新补的回退用例红**。处置是补输入，不是改判据。
- ADR 0087（W109）：迁移不改 0001（会让既有库校验和漂移）。本轮同此。
