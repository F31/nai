# Proposal: W132 — H17 的 runtime 接线：把「层失败」变成界面上可见的降级

## Why
`tests/registry.yaml` 的 `H17` notes 逐字写着「W66 已落判定面……**仍未接线**——渲染进 CapabilityView
与界面属呈现层工作」。本轮开工前**现测**，这句话是真的：`grep -rn evaluate_degradation` 在
`crates/` 与 `apps/` 里**只命中测试**——`crates/nai-kernel/tests/harness.rs`。生产代码里**零调用点**，
所以「一次降级有没有真的被送到界面」是一个只有读代码的人才能回答的问题。

更值得钉的一点是：**判定面自己答不了这个问题**。`DegradeVerdict::visible` 是**恒为 `true` 的构造常量**
（`evaluate_degradation` 无条件写 `visible: true`），它没有「不可见」这一格。也就是说「无静默降级」
在 kernel 侧是**构造性事实**，判它永远是绿；真正的风险不在这里，而在**判定到界面之间的 transport**。
本轮交付的就是这段 transport。

四环模型下的落点：①判定面（W66 已建）→ ②**执行体（本轮新建）** → ③调用方（呈现层，未闭合）
→ ④真实产生者（真实界面与真实模型，未接线）。

## What Changes
1. **呈现层承载物**（`crates/nai-kernel/src/evaluate/harness.rs`）：`DegradationNotice` +
   `render_degradation_notices`。一条判定渲染成**恰好一条**通知，**不去重也不合并**（同一层失败两次
   就是两个降级事件，合并会吞掉一次）；每个字段**照抄判定输出**，渲染器不重算 code / level / 可行动说明。
   六层的可行动说明 MUST 互异——一个「已降级」的通用串是「可见但什么也没说」。
2. **承载物**：`DecisionKind::Degradation` + 迁移 `0011-decision-log-degradation-kind.sql`
   （SQLite 改不了 CHECK ⇒ 逐列重建表、逐字复制行含四个哈希列、重建两个只追加触发器），
   版本号成套**六处**同步（`migration::VERSION` 10→11 / `SQL` 数组 + `include_str!` /
   `backup.schema.json` const / Rust 契约样本断言 / `schema.test.mjs` 备份样本含「上一版 MUST 被拒」+1 /
   TS 生成物）。
3. **`degradation` 判为 critical，不是 telemetry**：telemetry 写失败会落进**遥测缓冲**且调用方只看到
   `persisted == false`；一条能悄悄消失的降级，正是 INV-32 要禁的那种「静默降级」。W104 已经踩过一次
   这个缝（只改枚举不改 CHECK ⇒ 插入被 schema 拒绝并**静默落入缓冲**），本轮把两处一起改。
4. **接线**（`crates/nai-runtime/src/degradation.rs`）：`Composition::record_degradation` 在生产路径上
   **真调用** `evaluate_degradation` 与 `assert_no_free_form`——判定先于写入，违规 ⇒ 具名拒绝且
   **零落盘**；`Composition::read_degradation_view` **只从链上解码**，不可解析 ⇒ 具名失败
   （MUST NOT 静默跳过），空链 ⇒ 报 `0`，域 = `degradation` 那一种。
5. **刻意不做**：
   - **不改 `CapabilityView` 的契约**。`CapabilityDescriptor` 描述的是**插件能力**
     （`capability_id`/`plugin_id`/`state`），把 harness 的层失败塞进去是语义错配——插件没失败，
     是某一层失败了；而且改 proto 要 protoc 再生，本机没有（工具链前提）。§46.5 也写明本轮不新增契约。
     ⇒ 「渲染进 CapabilityView」**仍未接线**，具名登记。
   - **不替 `assert_no_free_form` 增加新的违规类**。INV-32 后半段的两个类已由 W66 定义，本轮只是把它
     放到生产路径上执行；再造一类就是同一件事的第二处定义。
   - **不给 `record_degradation` 传判定结果**。它的入参是**事实**（哪些层失败了、还剩什么可用、
     上下文还留着什么），不是「落到哪一级」——后者由判定给出。传判定就是「注入判定」。

## 具名登记（本轮**发现但不修**的弱化，MUST NOT 顺手处理）
1. **真实界面仍未接线**：本轮交付的是「降级项清单**存在且可被界面读到**」（有渲染形状、有落盘、
   有读回面）。真正的 CapabilityView / UI 渲染需要契约扩展 + 壳体 IPC，两者都未建 ⇒ H17 的
   `status` **仍 `pending`**（与 H15/H01/H02 接线轮的惯例一致：接线到 runtime ≠ 验收达标）。
2. **`critical_failures_surface_and_never_enter_the_buffer` 的关键种类清单是手写的**
   （`crates/nai-storage/src/tests/decision_log.rs` 里四个 `DecisionKind` 逐字列出）。本轮新增了第五个
   critical 种类而该用例**没有变红**——它不声明关键种类的闭世界，所以「新增一个 critical 种类」
   在那里是无信号的。本轮不改（改它会动到既有判据的形状），如实登记。
3. **`required_str` 在 `assembly.rs` 与 `degradation.rs` 各有一份**。合并要改 `assembly.rs` 的 10 处
   调用点，属本轮范围外；两份的消息各自指名自己的行形状，不是同一个字符串。
4. **期限敏感断言在 CPU 争抢下误报**：`nai-storage --lib` 默认并行度下
   `corrupted_database_artifact_and_traversal_never_activate_destination` 报
   `DeadlineExceeded / operation stopped`，`RUST_TEST_THREADS=4` 下 **82/82 全绿**。与 W84 同族，
   根因未修（本轮亦未修），处置仍是「限并发 + 四条替代判据」，不改代码。
5. **R2 注入首跑假绿**：注入写成 `if !violations.is_empty() && !cfg!(test)` 时 9/9 全绿——
   因为 `cfg!(test)` 在**生产 lib 里恒为 false**，`degradation.rs` 不是测试 crate。改成
   `&& verdicts.is_empty()` 后才红。**注入不变红的第三种成因**：不是被短路、也不是无覆盖，
   而是**注入自身没有改到行为**。⇒ 注入后全绿 MUST 先怀疑注入，MUST NOT 直接记作「判据无判别力」。

## Modified Capabilities
- `agent-harness`：**ADDED** 三条 Requirement（渲染 / 传输与读回 / INV-32 在生产路径）。
  刻意**不 MODIFIED** 既有的 `Every harness layer failure lands on a named, ordered and visible degradation`
  ——那条声明的是**落到哪一级**（W66 的判定面），本轮新增的三条声明的是**那条结论怎么到界面**，
  是另一件事；「一条 Requirement 只声明一条行为」。
