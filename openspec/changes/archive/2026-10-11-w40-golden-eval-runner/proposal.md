## Why

`tests/registry.yaml` 的 **B01「知识质量门槛」** 状态是 `blocked`，验收口径是规格 §655「100 题集**达到**第 6 章门槛」＝**实测达标**。
W30 交付了判定机制（`nai-kernel::golden`），W31 交付了载荷载体（`xtask check --golden` + 题集格式与位置）。
判决书与卷子格式都在，**但没人能组织这场考试**。逐条核对后缺的是四段，性质完全不同：

| # | 缺什么 | 性质 | 谁做 |
| --- | --- | --- | --- |
| ① | 100 题题集内容 | **数据**；ADR 0033 决策 6 明禁代造 | 场景包产出方（外部） |
| ② | 参考答案判据 | **口径** | 本仓库（本轮） |
| ③ | 评估执行体 | **编排代码** | 本仓库（本轮） |
| ④ | 真实知识库内容 | 外部（§5.9「知识来源」） | 场景包产出方 |

②是本轮最容易被忽略的一段。题集 schema **不含参考答案**，而 `Submission.answer: AnswerVerdict`
（`crates/nai-kernel/src/golden.rs:293`）由调用方注入——所以即使 100 题到位，`answer_accuracy`
依然是外部判定后填进去的，**达标链条上有一段没有可复现的执行体**。

③是核对后确认的真缺口。全仓库检索不到任何把「题集 → 真实检索 → 真实生成 → 逐题判定 → `QualityReport`」
串起来的代码：判定在 kernel、载荷校验在 xtask，**两者之间没有执行者**。而两个接缝都已具备——
`TextGenerator`（`crates/nai-provider-core/src/text.rs:90`，W36 起真实模型已通）与
`retrieve_local`/`retrieve_session_knowledge`（`crates/nai-storage/src/knowledge.rs:518,549`）。

ADR 0033 的后果段把「真实题集内容、真实模型评估与真实检索质量」一并划给外部。对①④这是对的（数据必须人工投入）；
对③它把**编排代码**也说成外部的，代价是：真实模型已经可用，却没有任何一次运行能产生 B01 的结论。

## What Changes

- 题集的每题**必须**声明参考答案（题集 schema 修订升到 **2**）：`expected { kind: "reference", must_include: [...], must_exclude: [...] }`。
- `nai-kernel::golden` 新增 `ExpectedAnswer` 与纯函数 `judge_answer()`：归一化后按**必须包含 / 不得包含**判定，
  产出既有的 `AnswerVerdict`。判定是**口径**不是编排，故落在 kernel 并受 portable 门控覆盖。
- 新增 `xtask/examples/golden_eval.rs`：**评估执行体**，照 `crates/nai-provider-core/examples/live_provider_smoke.rs`
  的既有惯例——复用 xtask 自己的解析器、真实 `TextGenerator` 驱动、缺前置一律 `CAPABILITY_NOT_READY` 且**拒绝用桩替代**。
- 夹具 `tests/fixtures/rag-golden/example-min.json` 同步升到修订 2；**仍明确标注不是真值**。
- `docs/development-plan.md` / registry B01 notes 记录缺口从 4 段降到 2 段；**B01 保持 `blocked`**（①④未达，达标不可能达成）。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `knowledge-quality-gate`：新增三条需求——每个用例 SHALL 声明参考答案；参考答案判定 SHALL 是纯函数且**未判定不等于通过**；
  评估 SHALL 由真实接缝驱动，缺失前置时 MUST 报 `CAPABILITY_NOT_READY` 而 MUST NOT 用桩替代。

## Impact

- 新增：`xtask/examples/golden_eval.rs`。
- 修改：`crates/nai-kernel/src/golden.rs`（类型、字段、纯函数）、`crates/nai-kernel/tests/golden.rs`、
  `xtask/src/golden.rs`（修订 2 与 `expected` 解析及其测试）、`tests/fixtures/rag-golden/example-min.json`、
  `schemas/rag-golden/case-set.schema.json`、`docs/golden-set.md`、`tests/rag-golden/README.md`、`docs/adr/0040-*.md`、
  `tests/registry.yaml`（B01 notes）、`docs/development-plan.md`、`AGENTS.md`。
- 依赖：`xtask` 的 `[dev-dependencies]` 新增 `nai-provider-core`、`nai-storage`、`tokio`（tool 层无依赖限制；
  且 **example 不进 lib**，故 `cargo test -p nai-xtask --lib` 与 portable 门控的覆盖范围不变）。
- **不新增 workspace 包**（仍 18 个），`check-deps` 的方向规则不被触碰。
- **不代造任何题**：夹具仍是构造数据并如实标注；门禁与执行体都不在 `check --profile portable` 内。
