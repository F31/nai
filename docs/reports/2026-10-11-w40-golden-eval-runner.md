# W40 黄金集评估执行体与参考答案判据（B01）

- 日期：2026-10-11
- 变更：`openspec/changes/archive/2026-10-11-w40-golden-eval-runner/`
- 决策：[ADR 0040](../adr/0040-golden-eval-runner-boundary.md)
- 指南：[docs/golden-set.md](../golden-set.md)、[tests/rag-golden/README.md](../../tests/rag-golden/README.md)
- 相关：[ADR 0032](../adr/0032-knowledge-quality-gate-boundary.md)、[ADR 0033](../adr/0033-golden-set-payload-boundary.md)

## 一、为什么做这一项

registry 的 **B01「知识质量门槛」** 是 `blocked`，验收口径是规格 §655「知识质量：**100 题集达到**第 6 章门槛」＝**实测达标**。
W30 交付了判定机制（`crates/nai-kernel/src/golden.rs`），W31 交付了载荷载体（`xtask check --golden` 与题集格式与位置）。
**判决书与卷子格式都在，但没人能组织这场考试。**

逐条核对后，距达标缺四段，**性质完全不同**：

| # | 缺什么 | 性质 | 谁做 | 现状 |
| --- | --- | --- | --- | --- |
| ① | 100 题题集内容 | 数据 | 场景包产出方（外部） | 缺（ADR 0033 决策 6 明禁代造） |
| ② | 参考答案判据 | 口径 | 本仓库 | **本轮补齐** |
| ③ | 评估执行体 | 编排代码 | 本仓库 | **本轮补齐** |
| ④ | 真实知识库内容 | 数据 | 场景包产出方（外部） | 缺 |

### 发现 F1：题集没有参考答案，正确率只能靠外部注入

题集 schema（修订 1）**不含参考答案**，而 `Submission.answer: AnswerVerdict`
（`crates/nai-kernel/src/golden.rs:293`，现为 `:349` 附近）由调用方注入。因此即使 100 题到位，
`answer_accuracy` 依然是外部判定后填进去的——**达标链条上有一段没有可复现的执行体**。

### 发现 F2：全仓库没有评估执行体

检索确认：没有任何代码把「题集 → 真实检索 → 真实生成 → 逐题判定 → `QualityReport`」串起来。
判定在 kernel、载荷校验在 xtask，**两者之间没有执行者**。而两个接缝都已具备：
`nai-provider-core` 的 `TextGenerator`（`src/text.rs:90`，W36 起真实模型已通）与 `nai-storage` 的
`retrieve_local`/`retrieve_session_knowledge`（`src/knowledge.rs:518,549`）。

### 发现 F3：ADR 0033 把「编排代码」也算成了外部的

ADR 0033 的后果段把「真实题集内容、真实模型评估与真实检索质量」一并划给外部。对 ①④ 这是对的
（数据必须人工投入）；对 ③ 它把编排代码也说成外部的，代价是**真实模型已经可用，却没有任何一次运行能产生 B01 的结论**。
本轮补足这一条，并明确与 0033 的关系是**补足而非推翻**（其七条决策全部保持）。

## 二、交付

### 1. 题集修订 2：参考答案是题集的一部分

每题**必填**：

```json
"expected": { "kind": "reference", "must_include": ["七小时"], "must_exclude": ["终身保修"] }
```

`must_include` **至少一条**：一个没有任何要求的参考答案，会让判定在「自动正确」与「永远未判定」之间二选一，
两者都等于**把判定关掉**。`must_exclude` 可选，出现即判错。

**修订 1 不做双读**：同时接受两版会让一份漏写 `expected` 的旧题集读成「没有参考答案要求」而不是结构不合规——
静默改变用例含义，与 ADR 0033 决策 2 拒绝未知字段是同一条理由。夹具
`tests/fixtures/rag-golden/example-min.json` 同步升到修订 2（**仍是构造数据，不是真值**）。

### 2. 判定是 kernel 纯函数

`judge_answer(&ExpectedAnswer, &str) -> AnswerVerdict`（`crates/nai-kernel/src/golden.rs`）：

- **归一化**：Unicode 小写折叠 + 连续空白折叠 + 去首尾，使换行与缩进不产生假不匹配；
- **否定式优先**：任一 `must_exclude` 命中即 `Incorrect`，无论 `must_include` 是否满足——
  否则「既说对又说错」的答案会通过；
- **空判据返 `Unknown`**（未判定，从分母排除），**不返 `Correct`**：没有判据不等于全部正确；
- 取**子串**而非相似度阈值：阈值需要一个数字，那个数字本身就成了判据，且不可复现。

判定产物是**既有的** `AnswerVerdict`，因此 `Submission`/`evaluate`/`QualityReport` **一个字段都没改**，
`Unknown` 从正确率分母排除的语义原样继承。放 kernel 而非执行体，因为它是**口径**不是编排——
放在执行体里会失去 portable 门控的覆盖。`GoldenCase` 另加 `answer_verdict(&self, answer)`，
对没有参考答案的用例返回 `Unknown`；`audit_coverage` 新增 `expected_answer_absent`（按用例 id），
使「从未判定」不会读成「全部正确」。

### 3. 评估执行体

`xtask/examples/golden_eval.rs`，对照既有先例 `crates/nai-provider-core/examples/live_provider_smoke.rs`：
真实链路一律放 `examples/`，不进测试、不进工程门控。

```bash
NAI_LLM_BASE_URL=... NAI_LLM_API_KEY=... NAI_LLM_MODEL=... \
  cargo run -p nai-xtask --example golden_eval [--dir <目录>] [--evidence <文件>]
```

它复用门禁**自己的**读取器（`read_case_set` 与 `case_set_files`，与审计共用同一个 `parse_case_set`），
所以「这里读得进」与「门禁放行」**永远是同一件事**。

| 情况 | 退出码 | 行为 |
| --- | --- | --- |
| 目录内无 `*.json` | 1 | `CAPABILITY_NOT_READY`，指出被搜索目录，**不发起调用** |
| 结构不合规 | 1 | 逐条报出缺陷，**不发起任何调用** |
| 缺少环境变量 | 1 | `CAPABILITY_NOT_READY` + 变量**名**；凭据值从不打印、从不入库 |
| 真实调用失败 | 2 | 与「前置未就绪」区分 |

**它只驱动生成侧**：检索召回、检索 P95 与行为类判据（澄清/转义、无副作用、取消）具名打印为**未驱动且不填零**，
并明示「不是 B01 达标」。

**否决「用答案文本猜 `outcome` 再喂 `evaluate`」**：那会得到一个字段齐全但含义失真的报告——
行为类 token（`no_answer_not_clarified` 等）**假失败**（把「执行体没驱动」读成「系统差」），
而 `MustNotAct` **假通过**（本路径没有副作用通道，恒为空）。**两种误导都比缺字段严重。**
（部分提交喂给 `evaluate` 本就会被拒——`every golden case needs exactly one submission`——这正是它该有的行为。）

## 三、实测

### 3.1 测试

```
cargo test -p nai-kernel --test golden     31 passed / 0 failed     （原 24 + 新 7）
cargo test -p nai-xtask --lib              39 passed / 0 failed     （原 34 + 新 5）
```

新增 7 项 kernel 判据测试：全部命中 ⇒ `Correct`、缺一条 ⇒ `Incorrect`、`must_exclude` 命中覆盖全部命中 ⇒ `Incorrect`、
空白/大小写不改变结论、空 `must_include` ⇒ `Unknown`、无参考答案的用例 ⇒ `Unknown`、审计具名列出每个缺参考答案的用例。
新增 5 项 xtask 载荷测试：修订 2 接受而 1/3 被拒、缺 `expected` 是缺陷、空 `must_include` 是缺陷、未知 `kind` 与未知字段被拒、
`must_exclude` 可缺省但不可为空数组。

### 3.2 就位门禁

```
cargo xtask check --golden --dir tests/fixtures/rag-golden
  files: 1；example-min.json [fixture-min] 8 case(s)
  below_minimum_cases: 8 of 100        ← finding，不是 defect（结构合规）
cargo xtask check --golden
  files: 0 → CAPABILITY_NOT_READY（如实反映）
```

### 3.3 真实链路演练（夹具 + 真实模型）

夹具（占位问题，与夹具参考答案不匹配，**正确率无意义，只证链路通**）：

```
example-min.json [fixture-min] 8/8 judged, 0 correct
answer_accuracy: 0 permille over 8 judged
unmeasured: evidence_recall_at_5 — no served knowledge base is attached to this runner
unmeasured: retrieval_p95 — no retrieval samples exist without a served knowledge base
unmeasured: behaviour_expectations — …the guard, side-effect and cancellation channels, which this runner does not drive
not a B01 pass: §655 asks for the §6 thresholds, which include the unmeasured fields above
```

临时探针（1 例中文问题 + `must_include: ["。"]`，**跑完即删、未入库**）证明判定**双向往返**可用、不是恒 `Incorrect`：

```
probe.json [probe] 1/1 judged, 1 correct
answer_accuracy: 1000 permille over 1 judged
```

三条未就绪路径均已验证：默认目录空 ⇒ `CAPABILITY_NOT_READY`（不发起调用）；夹具 + 无凭据 ⇒
`CAPABILITY_NOT_READY: missing NAI_LLM_BASE_URL, NAI_LLM_API_KEY, NAI_LLM_MODEL`（只报变量名，不发起调用）。

### 3.4 工程门控

```
cargo xtask check-deps        18 workspace packages（方向不变）
cargo xtask check --secrets   credential scan passed: 702 files, no prefixed credential material
cargo xtask check --registry  通过
cargo fmt --all --check       干净
两处 tsc --noEmit EXIT=0；vite build 30 模块；desktop 测试 5/5；test:ui 15/15
cargo xtask check --profile portable
  EXIT=0；82 个测试目标（65 Running + 17 Doc-tests）；694 passed / 0 failed；LNK=0
openspec validate --all --strict --archived
```

（首轮 portable 被 clippy 的 `manual_checked_division` 拦下——`total_correct * 1000 / total_judged`
应写成 `checked_div`，它与「没有判定就没有比率」本就是同一条规则。改为 `checked_div` 后通过。
其后几轮 `cargo test` 出现**与本轮改动无关**的间歇性 `LNK1104`：失败的是 `nai-media-core`、`monitoring`、
`frame_slots` 等 W40 未触碰的目标，且**每次失败的文件不同**——是主机对刚链接出的可执行文件的瞬时占用，
非磁盘（E: 余 69G）、非并发（降到 `CARGO_BUILD_JOBS=2` 仍复现）。预热式重试一次即全部链接成功，
随后门控 `LNK=0` 通过。）

## 四、未覆盖边界

- **行为侧未驱动**：守门判定、副作用通道、取消通道的观测未接入。执行体目前只在生成侧产出结论，
  且**不**对行为类判据报结论（具名打印为未驱动）。
- **真实检索未接**：真实知识库内容属外部投入（④），执行体不伪造 `retrieved`，召回与 P95 一律记未驱动。
- **题集内容未提供**：`tests/rag-golden` 仍为 `files: 0`；夹具是构造数据，不是真值，不被默认命令读取。
- **不是 B01 达标**：§655 要求 §6 门槛全部达到，其中包含本执行体未驱动的字段。registry B01 **保持 `blocked`**。
- **未新增包**：仍 18 个；真实接线只进 `[dev-dependencies]`，example 不进 lib，故 portable 的覆盖范围与耗时不变。
- **不做评估 UI**；不改 `knowledge-pipeline`（它管怎么检索，本模块管怎么判定检索质量）。

## 五、结论

B01 达标链条上**仓库内可控的两段已齐**（判据、执行体）。遗留的两段是纯外部投入——100 题内容与真实知识库内容——
两者到位后**零代码改动**即可出完整结论。因此本轮**不**把 B01 升级：**「执行体就位」不等于「达标」**，
一次可运行的评估不是质量证据（`live-provider-evidence` 规格对此已有明条：一次连通冒烟不得用来把 B01 标为达标）。
