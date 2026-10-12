# W67：harness 爬坡门控（V3.9 §5.9、§47.2、§47.3；验收 H18）

日期：2026-10-12 · 变更：`openspec/changes/w67-harness-ramp-gating/` · 交付：`crates/nai-kernel/src/evaluate/ramp.rs`

## 一、判断

上一轮盘点把 R1 剩 14 条按性价比排序，第 1 项是 **H18 爬坡门控**（与 H15/H17 同轨、判定面刚建好、成本最低）。
本轮执行它。判断依据是三条实测，不是转述：

1. `grep -rn 'ramp\|爬坡' crates/ --include=*.rs` ⇒ **仅命中一个无关的测试辅助函数**
   （`crates/nai-kernel/tests/metrics.rs` 里的 `fn ramp(count)`，是延迟样本的构造器）。
   ⇒ 「一个 harness 机制能不能进自动模式」在代码里**没有任何判定面**，只能靠人读代码和开会决定。
2. V3.9 §51.3 把 R1 保留的 harness 基础纪律定为 **H01–H04、H15、H17、H18**。
   前六条已各有判定面（W56 的 H01/H02 登记、W57 的 H03/H04、W66 的 H15/H17），
   **H18 是最后一条完全未开始的**——这也是它在 W64 里与 H16（能力包）**同轮却跨线**的原因。
3. 契约 `SessionMode` 只有 `Rehearsal`/`Assisted`/`AutoSupervised` **三个**取值。
   而 §5.9 与 §47.3 的爬坡是**四段**，含「影子」⇒ **H18 的默认启用集合里有一个阶段在 R1 内没有落点**。
   这不是本轮制造的矛盾，是基线里既有的一处不对称，本轮照实登记（见 §三、§六）。

**问题不是「缺能力」，是「已写下的规矩没有可判定形式」。** §47.3 只有两句话，但两句话的主语不同，
本轮的第一件事就是不让它们被合成一个谓词。

## 二、方法：先核实再裁定

| 核实项 | 实测结果 |
| --- | --- |
| 代码里是否已有爬坡实现 | 无（唯一命中是不相关的 `fn ramp(count)`） |
| `SessionMode` 有几个取值 | **3**（`Rehearsal`/`Assisted`/`AutoSupervised`），**无 `Shadow`** |
| 「影子模式」属于哪条线 | §1.4 与 §17.3 均列在 **R2（Standard）** |
| `session::change_mode` 是否已门控自动模式 | **是**，但它是**会话维度**（值守租约、`actor_allowed`、二次确认），且已实现 `AutoSupervised → Assisted` 回落 |
| §47.2 的回归有哪几类 | **6**：装配回归／意图拒识／工具失败恢复／缓存误命中／MCP 描述漂移／A2A 越权委派 |
| 六类回归的执行体属于哪条线 | **H16（能力包）**——这直接影响 INV-33 的判定（见 §三.3） |

### 2.1 不成立的做法（本轮避开）

- **并入 `evaluate::harness`**：§47.1/§47.4 判的是**一次调用**，§47.3 判的是**一个机制 × 一个阶段**。
  合并会让「这次拒绝属于哪一类」重新不可判定（与 W66 不复用 `bridge::Degradation` 同一条理由）。
- **复用 `session::change_mode`**：那是会话维度，且已覆盖自动模式的回落；机制维度是另一条轴，互补不重叠。

## 三、交付

### 3.1 两条要求分开落，不合并成一个谓词

| §47.3 原文 | 可判定形式 |
| --- | --- |
| 「新机制**默认只在演练、影子与人工辅助下启用**」 | `RampStage::enabled_by_default()`：**只有 `AutoSupervised` 为假**；`DEFAULT_ENABLED_STAGES` 照抄这三个阶段 |
| 「进入受监督自动 MUST 先通过 §47.2 的回归，并在决策日志留下启用记录」 | `evaluate_promotion`：晋升到 `AutoSupervised` 须同时过**回归**与**记录**两条，缺一条即具名拒绝 |

合成一个谓词会让「拒绝是因为默认不启用，还是因为条件没过」不可分辨——这正是本仓反复出现的失败模式。

### 3.2 爬坡的四段与默认启用集合

`RampStage` = 演练 → 影子 → 人工辅助 → 受监督自动（变体顺序**即**爬坡顺序）。
`tests/ramp.rs` 的 `ramp_order_matches_the_baseline` 与 `default_enabled_stages_are_exactly_the_first_three`
把**内容**钉死（不是钉条数）：四段标签、每段 `index()`、以及「默认启用集合恰为前三段」。

### 3.3 六类回归与「相关」二字

§47.2 的措辞是「修改 harness 的提示词、装配顺序、阈值、路由或技能清单后，MUST 通过**相关**回归才生效」，
而 §47.3 只说「通过 §47.2 的回归」。两种读法（**全六类** / **相关类别**）本轮选**后者**，
并要求调用方**声明受影响的类别**（`touched`），判定 `passed ⊇ touched`。

**关键补充**：`touched` 为**空**判 `regression_scope_undeclared`。
理由是门控的，不是语义的——若允许空声明，任何机制都能靠「我不影响任何类别」直通自动模式，
「相关」二字就无从判定。**这是解释，不是基线原文**，已作为待确认项登记（§六）。

### 3.4 回归失效不是新增要求

§47.2 的「通过相关回归**才生效**」已经蕴含：回归结论绑定某个 harness 状态。
提示词、装配顺序、阈值、路由、技能清单任一变化 ⇒ 先前的「通过」不再使这次晋升生效。
可判定形式：`passed_at_revision != Some(harness_revision)` ⇒ `regression_stale`；
`passed_at_revision` 缺失（「通过过，但不知道在什么版本上通过」）同样判失效。

### 3.5 启用记录：校验什么，不校验什么

- **校验**：存在、指名同一机制、指名目标阶段、**值内**不含明文凭据（INV-11，复用 `harness::CREDENTIAL_MARKERS`，零重复）。
- **不校验**：是否已真的追加进哈希链——那是 `nai-storage` 的只追加写入，属 runtime 接线。
  允许时门控给出 `record_to_append`，由调用方去追加。

### 3.6 INV-33：为什么这条 R1 验收不依赖能力包

§47.3 要求「通过 §47.2 的回归」，而承载六类回归的**黄金集（H16）在 V3.9 §51.3 属能力包**。这不违反 INV-33：

- 门控判定的是「回归**是否已被记录为通过**」，不是「跑一遍回归」；
  它只消费一份已声明的 `passed` 集合，**不执行**任何黄金集用例；
- 因此 H18 在**未安装任何能力包**时可判定（INV-33 满足）；
- **产生**证据的执行体是能力包里的 H16 —— 这是「判定面在 R1、证据生产在别处」，
  与 W66 的 H15/H17（判定在 kernel、接线在 runtime）完全同构。

### 3.7 具名拒绝（8 条）

`NotAnAdvance` / `RegressionScopeUndeclared` / `RegressionMissing` / `RegressionStale` /
`EnablementRecordMissing` / `RecordMechanismMismatch` / `RecordStageMismatch` / `RecordContainsCredential`。

`constructed_rejection_set_is_exactly_all` 要求**每一条都能被单独构造出来**，
且构造出的集合**恰为** `RampRejection::ALL`（既证每条可构造，也防出现未登记的名字）。

## 四、验证

| 项 | W66 | W67 | 增量 | 说明 |
| --- | --- | --- | --- | --- |
| `cargo xtask check --profile full` | EXIT=0 | **EXIT=0** | — | 第 3 次尝试通过（`CARGO_BUILD_JOBS=1`；前两次撞已知的 LNK1104 批量链接争用，**逐包能过** ⇒ 非代码缺陷） |
| 测试目标 | 88（70 + 18） | **89（71 `Running` + 18 `Doc-tests`）** | **+1** | 新增 `crates/nai-kernel/tests/ramp.rs` |
| 测试项 | 785 | **798 passed / 0 failed** | **+13** | 恰等于新增的 13 项测试 |
| `dependency boundaries` | 19 包 | 19 包 | 0 | 无新增依赖 |
| `admission registry check` | passed | passed | — | V3.9 acceptance + D01–D14 映射 |
| 前端门控 | 22 项 | 22 项 | 0 | 未触碰前端 |

**两个增量互相对照**：+1 目标与 +13 项恰好对应新增的那一个测试目标与其 13 个用例，
是「未误改别处」的旁证（沿用 W66 起固定的记账口径；注意
`Running \`…nai-xtask.exe check --profile full\`` 是 cargo 的**包装行**、不是测试目标，
本次 raw `Running` 为 72，扣掉该行后为 71）。

非空洞自检（**五组注入，每组都实测会失败**）：

| 注入（坏实现） | 被哪些测试抓住 |
| --- | --- |
| 门控悄悄不管回归 | `constructed_rejection_set_is_exactly_all`、`missing_categories_are_all_named`（2 项） |
| 门控悄悄不管启用记录 | `constructed_rejection_set_is_exactly_all`（1 项） |
| 忽略 harness 修订漂移 | `constructed_rejection_set_is_exactly_all`（1 项） |
| 把凭据扫描改成字段名黑名单 | `a_field_whose_name_looks_sensitive_but_whose_value_is_clean_passes` 等 **3 项** |
| 只报第一个缺失类别 | `missing_categories_are_all_named`（1 项） |

### 4.1 本轮补出来的一个守卫缺口（值得记住）

「扫值不扫字段名」这条守卫，我最初写的字段名是 `api_key` 与 `secret_name`。
注入测试证明它是**无效守卫**：凭据标记是 `api_key=`、`secret=`（**带等号**），
`api_key` 与 `secret_name` 这两个名字**根本不含**任何标记 ⇒ 名字扫描的实现照样放行，测试全绿。
改成 `sk-cache` / `api_key=` / `secret=`（名字**含**标记、值干净）后才真正构成守卫，
注入 4 随即被 3 项测试抓住。

⇒ **非空洞守卫的前提是「坏实现真的会被它抓住」；写守卫时要先想清楚坏实现的形状，
别拿一个看起来敏感、实际不命中判据的样本当守卫。**（与 W66「悄悄删字段」同源：
守卫钉住的是条数还是内容，以及**样本是否真的落在判据上**。）

### 4.2 契约零漂移的取证方式

`cargo xtask schema --check` **在本机跑不了**（`generate.mjs` 内的 `spawnSync(process.execPath, …)` 报
`EBUSY`，managed 与 system node 都复现 ⇒ 主机对「node 自旋 node」的限制）。**故不宣称该检查已通过**，
改用两条互补证据：

1. **结构性**：生成器输入只有 `schemas/catalog/capabilities.json` 与 `schemas/{config,plugin}/*.schema.json`。
   本轮改动为 `crates/nai-kernel/src/evaluate/{mod,ramp}.rs`、`crates/nai-kernel/tests/ramp.rs` 与文档，
   **未触及任何生成器输入** ⇒ 漂移不可能发生；
2. **CI 独立步骤**：`.github/workflows/foundation.yml` 的
   「Check generated contracts without modifying consumers」在 Linux 与 Windows 上各跑一次，push 后核对。

### CI 复核

| run | commit | 结论 | 备注 |
| --- | --- | --- | --- |
| 38191443234 | `394992d` | **success**（两 job `completed success`：ubuntu-24.04 与 windows-latest） | 本轮实质提交（判定面 + 测试 + 登记 + 文档） |

## 五、验收侧：为什么 H18 仍是 `pending`

`tests/registry.yaml` 只改 `notes`，**`status` 不动**。理由沿用 W57/W66 口径：

- **按阶段真正启用/停用机制**是 runtime 接线，本轮未做；
- **把启用记录追加进哈希链**是 `nai-storage` 的写入，本轮未做；
- **六类回归的执行体**是能力包 H16，本轮只判「是否已记录为通过」。

判定面交付 ≠ 验收达标。故本报告不出现「爬坡已生效」或「记录已进日志」这类措辞。

## 六、未做与遗留

1. **runtime 接线**：按阶段启用机制、追加启用记录进哈希链。
2. **黄金集本体（H16，能力包）**：六类用例一个都没有。
3. **基线待确认项 A——影子在 R1 无落点**：§5.9／§47.3 是**四段**，`SessionMode` 只有**三个**取值，
   §1.4／§17.3 把「影子模式」列在 **R2**。⇒ **H18 的默认启用集合里有一个阶段在 R1 内无法被行使**。
   本轮**不改基线文本**（改基线是升基线轮次的事，且 MUST 是 1:1 行替换），
   只在代码里让 `session_mode()` 对影子返回 `None`、并由测试钉死「恰好三段有落点、影子没有」。
   **待下一轮基线修订裁定**：要么给影子一个契约落点，要么把 §47.3 的默认启用集合收敛为 R1 内的三段。
4. **基线待确认项 B——「通过 §47.2 的回归」是全六类还是相关类**：本轮取「相关类 + 声明非空」。
   若裁定为全六类，只需把 `passed ⊇ touched` 改成 `passed ⊇ REGRESSION_CATEGORIES`。
5. **未触碰**：H05–H14、H16（能力包）；O01/O02/O05、A12、B01、O03/O04（需外部投入）。

## 七、方法论教训

1. **一句话里的两个主语要拆成两个谓词**。§47.3 的「默认启用」与「晋升条件」主语不同，
   合成一个谓词会让拒绝原因不可分辨——本仓已第三次因为这个理由拒绝合并（前两次：不复用
   `bridge::Degradation`、不把爬坡并进 `harness.rs`）。
2. **「相关回归」这种措辞落到代码时必须显式化**：要么声明受影响范围，要么全量。
   选前者就要堵住「空声明」——门控的漏洞往往不是判据错，是**判据的输入可以被声明成空**。
3. **非空洞守卫的样本必须先落在判据上**（§4.1）。`api_key` 不含 `api_key=`，
   拿它当「看起来敏感」的样本是自欺；注入测试是唯一可靠的检验。
4. **动手前先查「是不是已经有人做了一半」**：`session::change_mode` 已经门控自动模式，
   若不先核实就建一个新门控，会产出两个都声称门控自动模式的入口。
   本轮的结论是**互补不重叠**（会话维度 vs 机制维度），这个结论本身也是核实得来的。
5. **R1 验收可以要求「过回归」而不违反 INV-33**——只要门控消费的是**已记录的结论**而非**执行回归**。
   判定面与证据生产分离，是让「要求严格」与「默认轻量」同时成立的机制。

---

> **勘误（W74 / ADR 0052）**：本报告把 `cargo xtask schema --check` 在本机报 `EBUSY` 归因为
> 「主机对**node 自旋 node**」的限制。W74 实测本机 node 对 `git`、`process.execPath`、`cmd.exe`、
> `sh` **一律**报 `EBUSY` ⇒ 正确表述是「**宿主禁止 node 派生任何子进程**」。**结论与处置不变**
> （仍以结构性理由取证 + CI 独立步骤兜底）。原文按历史记录保留。
