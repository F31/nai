# Tasks: W136 — B07 的执行体

## 1. 勘定（先量，再动手）

- [x] 1.1 读 `tests/registry.yaml` 的 B07 `notes` 原文：W68 交的是**判定面**，`pending` 的原因是
      「段落内容与话术、TTS 时长与播放采样是内容创作与运行时的事」
- [x] 1.2 读基线 §53 原文：四行都是「确定性脚本与规则，不依赖工具规划」（INV-33 不被触碰）
- [x] 1.3 读基线 §5.7 原文：「检查点保存段落与已完成句，不保存未播出的生成文本；
      **恢复前重新校验产品、价格、活动、素材与授权**，重启默认待机」
- [x] 1.4 现测四环：判定面**已有**（`evaluate_pitch` + `tests/selling.rs` 19 项）；
      `nai-runtime` 对 `selling`/`PitchRunner` **0 命中**；`Composition` 无相关方法；
      `apps/engine` 的 `Script` 无讲品概念
- [x] 1.5 现测既有判据的强度：`entrances.windows(2)` + 同段 ⇒ `selling@0, spec@600, selling@900`
      在冷却 1000ms 下**放行**（规格要求拒绝）⇒ 这是本轮必须一起修的**弱判据**，不是新增功能

## 2. 先写会红的判据（kernel）

- [x] 2.1 `PitchHistory`：最后一次进入 + 每段最后一次进入 + `count()`；`record` **无条件**记录
      （让违规擦掉自己的证据会使冷却可绕过）
- [x] 2.2 `evaluate_transition(history, next, policy)`：单调前提先判（递减 ⇒ **只**报单调，区间规则不评估，
      避免 `u32` 下溢被当作规则违规）
- [x] 2.3 `evaluate_pitch` 的 row 3 改为逐条重放 2.2（**单一定义**），冷却读数改成「该段自己的上次进入」
- [x] 2.4 判别性样本：`a_repeat_with_another_segment_in_between_still_hits_the_cooldown`
      （相邻对全不同段 ⇒ 旧实现放行）；边界另一侧 `a_repeat_exactly_at_the_cooldown_boundary_is_allowed`
- [x] 2.5 一致性样本：`the_incremental_transition_agrees_with_the_batch_verdict`；
      空历史 `the_first_entrance_has_no_interval_rule_to_break`；递减 `a_decreasing_transition_reports_only_the_monotonic_refusal`

## 3. 执行体（runtime）

- [x] 3.1 `PitchRunner::start`：先整份判定，`!allowed` ⇒ 具名拒绝，**不产生半个循环**
- [x] 3.2 `enter`：fail closed —— 拒绝时不 `record`、不移动游标
- [x] 3.3 `pause` / `resume`：检查点不存未播出文本；**归位不是新进入**（不记入时间线、不进冷却）
- [x] 3.4 `ResumeRecheck`：五面 × 三态；`fresh()` / `unavailable()` / `with()`；`Unavailable` 是拒绝
- [x] 3.5 `PitchRefusal` 四族四码：`INVALID_ARGUMENT` / `CONFLICT` / `EVIDENCE_EXPIRED` / `CAPABILITY_NOT_READY`；
      `reason()` 逐条点名规则与面

## 4. 调用方

- [x] 4.1 `Composition`：`begin_pitch` / `enter_segment` / `pause_pitch` / `resume_pitch` / `pitch`
- [x] 4.2 「没开始」与「进展为 0」不折叠：`pitch()` 返回 `Option`，未启动时 `enter_segment` 具名失败
- [x] 4.3 `apps/engine` **不接**，并在报告与 registry `notes` 里如实登记 0 命中与理由

## 5. 非空洞注入验证（七组，逐组真红）

- [x] 5.1 I1 冷却退回「相邻同段」⇒ kernel 1 红（判别性样本）+ runtime 1 红（孪生样本）
- [x] 5.2 I2 拒绝后仍推进时间线 ⇒ 3 红（fail-closed 三条）
- [x] 5.3 I3 不可测的重校验当作通过 ⇒ 2 红
- [x] 5.4 I4 归位被当成一次新进入 ⇒ 2 红
- [x] 5.5 I5 计划被拒时仍然启动 ⇒ 2 红
- [x] 5.6 I6 五个面退化成同一个名字 ⇒ 3 红
- [x] 5.7 I7 两类重校验拒绝合并成同一错误码 ⇒ 1 红
- [x] 5.8 逐字节还原：五个文件 `sha256sum -c` 全部 OK（注入基线取 fmt-clean 的最终字节）

## 6. 文档与收口

- [x] 6.1 `registry.yaml`：B07 `pending` → `implemented`，`notes` 写明建了什么、为何不升 `tested`
- [x] 6.2 ADR 0110 + 报告 + `docs/development-plan.md`（W136 小节）+ `docs/knowledge.md`
- [x] 6.3 全量门控 `cargo xtask check --profile full`
- [x] 6.4 归档 + 两条 validate
- [x] 6.5 一次 commit + push + CI 观测
