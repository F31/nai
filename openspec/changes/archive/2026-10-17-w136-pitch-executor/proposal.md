# Proposal: W136 — B07 的执行体：把「讲品循环」从一次判定变成会跑的东西

## Why

B07「基础讲品循环」登记 `pending`，`notes` 自己写明理由：W68 交的是**判定面**
（`crates/nai-kernel/src/evaluate/selling.rs`，19 项测试），而「讲品循环跑起来了吗」**仍然不可判**。

本轮开工前现测确认了缺口的位置与形状：

| 环 | 状态（现测） |
| --- | --- |
| ① 判定面 | **已有**：`evaluate_pitch(&PitchPlan, &PitchPolicy) -> PitchVerdict`，四条 row 全判 |
| ② 执行体 | **没有**：全仓 `selling`／`PitchRunner` 只在 `nai-kernel` 内命中（`evaluate/mod.rs`、`assembly.rs` 的注释、自身、测试），`nai-runtime` **0 命中** |
| ③ 调用方 | **没有**：`Composition` 无相关方法 |
| ④ 真实产生者 | 段落内容与话术、TTS 时长与播放采样、§5.7 的恢复前重校验（守门/知识层）——**都不在本轮** |

判定面的形状本身也说明了缺什么：它判的是**一整份 plan 的快照**（`entrances` 是整条时间线），
而一个在跑循环是**一次进一段**的，每一步要回答的是「**现在**能进这一段吗」。快照判定**不能**
回答这个问题——它没有「上一次进入是什么时候」这个状态。

顺带现测到一条**既有判据比规格弱**的地方（不是本轮新增，是被本轮逼出来的）：

- 规格：`Segment cooldown … 同一段落的**重复进入** MUST 不早于段落级冷却`
- 实现（W68）：`entrances.windows(2)` + `pair[1].segment == pair[0].segment`
- ⇒ `selling@0 → spec@600 → selling@900` 在冷却 1000ms 下**被判为合规**，因为**没有一对相邻同段**。
  而它的语义明明是「卖点在 300ms 后就回来了」。

## What Changes

1. **冷却改为按「该段自己的上次进入」判，并把这条规则抽成单一定义**
   （`nai-kernel`）：新增 `PitchHistory`（最后一次进入 + 每段最后一次进入 + 计数）与
   `evaluate_transition(history, next, policy) -> Vec<PitchRejection>`。
   `evaluate_pitch` 的 row 3 改为**逐条重放**这条函数，于是批次判定与在跑循环不可能对同一条时间线
   给出不同答案（「同一件事 MUST NOT 两处定义」）。
2. **新增执行体** `crates/nai-runtime/src/pitch.rs`：
   - `PitchRunner::start(input)`：**先把整份 plan 判完**，不合规的 plan 不进入「跑了一半」的状态；
   - `enter(at_ms)`：**fail closed** —— 被拒的那一步不记录、不移动游标，忽略错误的调用方不会跑到规则前面；
   - `pause(completed)` / `resume(recheck)`：§5.7 的检查点与归位。**归位不是一次新的进入**，
     所以它不进冷却判定，但它 MUST 过 §5.7 的重校验。
3. **§5.7 的重校验是输入，不是自证**：`ResumeRecheck` 覆盖「产品、价格、活动、素材、授权」五个面，
   每面三态 `Fresh / Stale / Unavailable`。`Unavailable` 是**拒绝**，不是通过——
   仓库内**没有**这五面的真实产生者（守门与知识层），所以「假设它是新的」会把一个**没测过的事实**
   写成一次**授权**。为此专门提供 `ResumeRecheck::unavailable()`，让「我们没有这份重校验」
   有一个**可表示**的取值（否则调用方只能在「撒谎」与「不调用」之间选）。
4. **拒绝分成四族、四个错误码**：plan 不合规 `INVALID_ARGUMENT`、转移被拒 `CONFLICT`、
   重校验过期 `EVIDENCE_EXPIRED`、重校验不可测 `CAPABILITY_NOT_READY`。
   「太早了」和「我们测不了」不是同一个答案，合并会让调用方无法分派。
5. **调用方**：`Composition` 新增 `begin_pitch` / `enter_segment` / `pause_pitch` / `resume_pitch` / `pitch`。
6. **registry**：B07 `pending` → **`implemented`**（不是 `tested`：真实产生者仍不在仓库内）。
   `notes` 逐项写明本轮建了什么、以及**为什么还不升 tested**。

## 刻意不做（具名登记）

1. **`apps/engine` 不接**。现测：engine 的 `Script` 里没有讲品概念（只有 `ManualEvent` 的 comment/gift），
   要接就得先设计脚本层的讲品编排格式——那是**另一个形状的设计**，与「把执行体建起来并接上调用方」
   不是同一件事。本轮如实登记 `apps/engine` **0 命中**（与 W123/W128 对 R10 的处置一致）。
2. **不实现 §5.7 的五面重校验**。它属守门与知识层；本轮只把**接缝**建好并让它 fail closed。
3. **不产出段落文本、不做 TTS 时长、不做播放采样**。§53 明确那属于内容创作与运行时。
4. **不改 `evaluate_pitch` 的四条 row 的其余语义**（顺序、缺段、插问、事实绑定），只动 row 3
   的冷却读数来源，并用判别性样本钉住差异。
5. **不给 `PitchHistory` 加「被拒的进入也算历史」的选项**。记录是**无条件**的：
   历史记录的是「发生了什么」，让一次违规擦掉自己的证据会使冷却可被绕过。
