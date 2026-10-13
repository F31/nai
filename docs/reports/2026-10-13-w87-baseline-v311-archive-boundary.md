# W87 · 设计基线升为 V3.11（把 §56.2 的归档边界写死；订正版）

- 日期：2026-10-13
- 类型：**升基线（订正版）**——**不加能力、不改发布线归属、不改任何 `status`**
- 产出：[V3.11 方案](直播数字人%20%2B%20智能体系统技术方案%20V3.11.md)（追加**第 59 章**）、[ADR 0065](../adr/0065-design-baseline-v311.md)、
  `openspec/changes/archive/2026-10-13-w87-baseline-v311-archive-boundary/`
- 上游登记：W78 §八第 8 条（[归档](../../openspec/changes/archive/2026-10-12-w78-capability-pack-granularity/)），另被 W79–W86 §八逐轮复述（**连续八轮未收录**）

## 一、判断：这不是「缺能力」也不是「范围过宽」，是一条**一般规则自身的边界未写死**

前两轮升基线处理的分别是：W65 的**范围重排**（V3.9）与 W71 的**已登记偏差收录**（V3.10）。
本轮（W87）面对的是**同一族问题的第三类**——不是「基线缺能力」，不是「范围过宽」，
也不是「注册表与基线不一致」，而是 **§56.2「订正与追加的边界」这条一般规则，自身有一处边界没写死**。

**登记处的原文**（W78 §八第 8 条）：

> 一条待明确的判据边界：§56.2 把 `openspec/changes/archive/` 列为「保留原文 + 追加勘误段」，
> 但**未区分「上一轮发布的归档」与「本轮刚归档、尚未入库」**。本轮撞上了后者，按「保护已发布原文」
> 的立意作了**就地订正**。**这个边界值得下一次基线修订时写死**，否则每一轮都要重新判一次。

这条被 **W79–W86 §八逐轮复述**（W78/W79/W80/W81/W82/W83/W84/W85/W86），**连续八轮未收录**。
本轮即 §56 规则的**第二次跨轮次适用**（第一次是 V3.10，见 [ADR 0049](../adr/0049-design-baseline-v310.md)）。

## 二、审实：先读先判，本轮核实后又发现**第二处**

**核实一（登记处本身）**：把 W76 §九第 6 条与 W78 §九第 8 条逐条读出来——

- W76 §九第 6 条修的是**已提交**归档目录里的 `design.md` ⇒ **追加勘误段**；
- W78 §九第 8 条修的是**同一次未提交工作区内**创建并归档的 change 目录 ⇒ **就地订正**。

⇒ 二者**处置不同，而判据相同**：判别项是**「该原文是否已进入提交」**（即是否曾在**工作区之外**可见），
**不是「轮次」**（「本轮」在多人协作下无定义 ⇒ 不可判）。W78 登记时用的是「保护已发布原文」的**立意**，
本轮把它写**精确**。

**核实二（第二处，W87 自己发现的）**：§56.2 情形③ 把「**§56**」**整体**列为**保留原文**的版本专属章节，
而——

- **V3.10 §58.2 的 E5 自己就就地订正过 §56.2**（即上一轮升基线时，§56.2 是被**改写**的对象）；
- [ADR 0049](../adr/0049-design-baseline-v310.md) 决策 4 的「不改」清单**不含 §56**，只列
  §39、§48.1/§48.2、§57.1/§57.2/§57.4/§57.5。

⇒ 该行**自相矛盾**：若「§56」整体冻结，则「随版本累积订正」这条**一般规则无法被订正**，E5 与 E2 都无从发生。
按 §56.2 自身标题（「本版确立的**一般规则**」），§56.2 **不是**「关于某一版的陈述」，
故**不在**情形③ 的射程内。

**核实三（结构指针）**：改动前实测活文档「V3.10」命中 **143 行 / 31 文件**（口径见 §五）；
`tests/registry.yaml` 的 `baseline_deviations: []` **已空**（W71 全部收录）
⇒ **无其它待收录的基线偏差**，本轮范围**只有** §56.2。

**两项都不属于「可纯追加」**：§56.2 明写**纯追加 MUST NOT 被用来让矛盾长期存续**；边界不定，
则**每一轮作者都要重新判一次**同一件事。⇒ MUST 就地订正（1:1 行替换）。

## 三、裁定（[ADR 0065](../adr/0065-design-baseline-v311.md)）

1. **V3.11 升为唯一实现基线**：保留 V3.10 全部条款（**无删除**），§1–§58 除订正所涉 5 行外**逐字保留**，
   另**追加 §59**（V3.11 版本说明与订正索引）。V3.10 降为**历史参考**。
2. **ADR 0049 的决策 1 被替代**，决策 2–5 继续有效；其中**决策 3（就地订正／追加勘误的边界）由 E1／E2 补明两处**。
3. **本版类型 = 订正版**：`tests/registry.yaml` 的**条目集合**与**任一条目的 `release_line`** 均未变
   ⇒ §49.2 的四栏评审**无对象**（四栏皆无）。
4. **明确不改什么**：不改任何 `status`、不改发布线归属、**不实现任何代码**、不给 §16 增行、不重排 W 编号。

## 四、交付：E1／E2 与 1:1 行替换契约

**E1**（§56.2「历史记录 ⇒ 保留原文 + 追加勘误段」行）：补明**保护对象是「已发布的原文」**——
判据为该原文**是否已进入提交**；**同一轮内在工作区内新建、尚未提交的产物** ⇒ **就地订正**，
不产生勘误段（它从未发布，没有「已发布的原文」需要保护）。

**E2**（§56.2 情形③）：把「该版专属的版本章节」清单中的「**§56**」**精确**为「**§56.1／§56.3／§56.4**」，
并注明「**§56.2 是一般规则**、随版本累积订正」。**不改任何一条规则的内容**，只把清单**写精确**。

**契约证据**（`diff`，非 `-u`，`V3.10.md` → `V3.11.md`）：

| 项 | 实测 |
|---|---|
| 规模 | **2713 行 / 226,578 字节 ⇒ 2766 行 / 233,290 字节** |
| hunk | **5** = **4 个 `c` 型**（`1c1`、`3c3`、`29c29`、`2573,2574c2573,2574`）+ **1 个 `a` 型**（`2713a2714,2766`）+ **0 个 `d` 型** |
| 既有正文（前 2713 行） | **只有 4 个 `c` 型、零 `a`·`d`** ⇒ 行号一一对应，全部既有 `§NNN` 引用继续有效 |
| 订正行数 | **5 行对 5 行**（3 行版本标识 + 2 行 §56.2 订正）；追加块 **53 行**（§59） |

⇒ 满足 §56.2 对就地订正的硬约束（**MUST 1:1 行替换**）。**基线被 ADR 批准后不可变**：
`V3.10.md` 一字未改，V3.11 是**新文件**（与 V3.9/V3.8 各自只有一次提交的先例一致）。
§59 追加块锚在**末行之后**（`2713a2714,2766`，与 ADR 0049 的 `2647a2648,2713` 同形）。

## 五、指针同步：四情形分流（MUST 逐条判定，MUST NOT 批量替换）

改动前实测 **143 行 / 31 文件**命中「V3.10」（口径：`*.md`/`*.yaml`/`*.json`/`*.mjs`/`*.ts`/`*.tsx`/`*.rs`/`*.toml`，
排除 `docs/reports/`、`docs/adr/`、`openspec/changes/archive/`、`.workbuddy/` 与基线文档本身）。按 §56.2 四情形分流：

- **情形①（声明当前基线）⇒ 更新**：`docs/design.md`、`README.md`、`AGENTS.md`、`docs/engineering.md`、
  `openspec/config.yaml`、`xtask/src/registry.rs` 输出串、`schemas/tests/registry.schema.json` 描述串、
  `apps/desktop/src/main.tsx` 界面文案、`docs/development-plan.md` 的**「V3.11 切片工作项进展」标题**
  （W71 先例：该标题随基线重命名）、`docs/platform-matrix.md` 与 `schemas/platform-matrix/platforms.json`。
- **情形②（`§NNN` 版本前缀）⇒ 更新**：`tests/registry.yaml` 的 release_line 标签与 **29 条能力包标记的 `§51.3`**、
  各 `docs/*.md`（`upgrade`/`operations`/`multi-session`/`mixing`/`deployment`/`cloud`/`kernel`）与
  `openspec/specs/*` 的章节指针、轮次记录标题（`docs/kernel.md`、`AGENTS.md` 的 W57/W66–W70 小节）。
- **情形③（关于某一版）⇒ 保留**：升基线链条、`V3.10 §58.2` 的 E1–E5 收录记载、W71/W72 轮次记录、
  `openspec/specs/spec-driven-delivery/spec.md` 里**被拒绝改写**的举例、`tests/registry.test.mjs:155` 的
  「V3.10 closed」、以 `V3.9` 为**陈旧样本**的夹具。

**幸存 35 行 / 9 文件（sweep 当次实测），逐条核对全属情形③** ⇒ 「集合层对划分是瞎的」的反面：**分流 MUST 逐条判定**。
**口径提醒**：sweep 之后，本轮**新写的论述段**（`AGENTS.md`、`docs/development-plan.md` 的 W87 小节，以及本 change 目录）
又引入若干**新写的**情形③ 引用（它们不是「sweep 幸存」，而是**新叙述**）⇒ 此后全仓 `grep` 的计数**高于**上表，
差额全部落在这些新增段落里。
**回滚实例（如实登记）**：`tests/registry.yaml` 的 3 处（B07 的 `V3.10 §48.5`、`（V3.10 §58.2 的 E1–E3）`、
`见 V3.10 §58.2 的 E4、§58.4`）与 `tests/registry.test.mjs:155` 在 `replace_all` 之后**被逐一回滚**——它们是
**收录记载**（情形③），不是当前指针。另把 `scripts/contracts/baseline.mjs:369` 举例用的
「废弃草稿 … V3.11.md」改为「… V3.12.md」（因 V3.11 现**正是**基线，举例 MUST 指**非**基线）。

## 六、验证

### 6.1 定向测试

| 项 | 实测 |
|---|---|
| `node --test tests/registry.test.mjs` | **47 / 47** |
| `node --test tests/*.test.mjs tests/contracts/*.test.mjs` | **78 / 78** |
| `openspec validate w87-baseline-v311-archive-boundary --strict` | **valid** |
| `proposal.md` 的 `## Why` 段字符数 | **557**（`CHANGE_WHY_TOO_LONG` 门限 1000，非阻塞；**本轮现测**，MUST NOT 照抄上一轮） |

**一处命令坑（本轮再次踩到）**：`node --test tests/` 把**目录参数当模块解析** ⇒ `MODULE_NOT_FOUND`；
MUST **显式列出** `tests/*.test.mjs tests/contracts/*.test.mjs`（与 `engineering_checks` 的登记一致）。

### 6.2 门控

`cargo xtask check --profile full` ⇒ **EXIT=0**（`MSVC` + `CARGO_TARGET_DIR=E:/nai-build-target`）。

| 项 | 实测 |
|---|---|
| 测试目标 | **91**（**73** `Running` + **18** `Doc-tests`；74 个 `Running` 行里有 **1 个是 cargo 包装行**） |
| 通过 / 失败 | **860 passed / 0 failed** |
| 与 W84／W86 的关系 | **目标数与项数与 W84／W86 完全相同（91 / 860 / 0）**——本版零 Rust、零测试改动，数字不变本身就是「未误改代码」的旁证 |
| 依赖边界 | `dependency boundaries passed: **19** workspace packages` |
| 注册表 | `admission registry check passed (**V3.11** acceptance + D01-D14 mapping)` |
| `tests/registry.test.mjs` | **47 / 47** |
| `tests/contracts/schema.test.mjs` | **29 / 29** |
| `tests/platform/platform-matrix.test.mjs` | **22 / 22** |
| 桌面包单测 | **5 / 5** |
| `tests/ui` | **52 / 52** |
| 前端门控 | `[frontend] 全部步骤通过` |

**一处口径提醒（本轮再次踩到）**：数测试目标时**必须排除** cargo 的包装行
`Running \`…nai-xtask.exe check --profile full\``——它是 `cargo run` 的包装，**不是测试目标**
（本轮 74 个 `Running` 行里有 1 个是它，故 **73 + 18 = 91**）。

**一处宿主瞬态（如实登记，非代码缺陷）**：首轮门控在第一处 `cargo test -p nai-xtask --lib` 链接时
报 **`LNK1104`（无法打开刚构建的 `nai_xtask-*.exe`）**。按既有判据处置——**逐包单独跑能过 ⇒ 非代码缺陷**：
单独 `cargo test --locked -p nai-xtask --lib` **52 passed / 0 failed**；随后**限并发**
（`CARGO_BUILD_JOBS=1`）重跑 full ⇒ **EXIT=0 / 91 目标 / 860 passed / 0 failed**。
⇒ 「宿主瞬态条件 MUST NOT 改变被测系统的结论」（W70）：**未改任何代码**来绕过它。

### 6.3 两条 validate

**归档后**实测（MUST 在 `archive` 之后立刻跑 `--archived`）：

| 命令 | 实测 |
|---|---|
| `openspec validate --all --strict` | **43**（**43 个 `spec/`**；**归档前**为 **44** = 43 + 未归档的 w87） |
| `openspec validate --all --strict --archived` | **85**（**只含已归档变更、不含 specs**，每归档一次 +1；W86 为 **84**，本轮的 +1 即 w87） |

**一处规格差量的返工（如实登记）**：首版把归档边界写成**追加在既有 Requirement 正文末尾的一段**
（`MODIFIED Requirements`）。归档并入 `openspec/specs/spec-driven-delivery/spec.md` 后，
`validate --all --strict` 立即报 **`requirements[3]` Requirement 正文 >500 字符**——**strict 下告警即失败**
（42 passed / **1 failed**）。按校验器的处置建议（"**split it into separate requirements that each state
one behavior**"）改为**拆成独立 Requirement**（`## ADDED Requirements`，
`### Requirement: Historical records are protected by the commit boundary`，正文 **~280 字符**）：
① 既有 Requirement **保持 HEAD 原文不动**；② 新增 Requirement **只声明一条行为**；③ `--all --strict` 回到 **43 / 0**。
**回退与重归档**：`git checkout HEAD -- <live spec>` → 取回 change 目录 → 改差量为 `ADDED` → 重跑 `archive`（`+ 1 added`）。
该 change 目录**尚未进入任何提交** ⇒ 按本轮新立的 E1 边界，**就地订正、不产生勘误段**（见 §三）。详见 `design.md` §6。

## 七、CI 复核

**run `38360257934`（run_number **104**，`head_sha = 337590da8e…`，与本轮推送 SHA 一致）= `success`**（双平台绿）。

观测方式沿用既有纪律：**单次低频查询**（**不轮询**），每次带**唯一查询参数** `&cb=<ns>` 破坏代理缓存、
`Cache-Control: no-cache`，并**回读 `head_sha` 与本次推送比对**；`curl` 带 `--tlsv1.2 --tls-max 1.2`
（schannel 与 TLS 1.3 握手失败，W77 实测）。本轮共 4 次查询：

1. 推送后首次：run 104 `in_progress`（`updated_at = 07:03:03Z`）；
2. 等 5 分钟后：仍 `in_progress`（父 run 的 `updated_at` **未变** —— **「状态不变」≠「卡住」**）；
3. 为判断「是否真在推进」查了一次 `/jobs`：**`ubuntu-24.04` job 已 `completed / success`**（含
   step 9 `Check generated contracts without modifying consumers` 与 step 22
   `openspec validate --archived --no-interactive` 均 `success`），**`windows-latest` job 在 step 10
   `Complete engineering foundation` `in_progress`** ⇒ 正常推进；
4. 再等 6 分钟后**单次查询**：run 104 `completed` / `conclusion = success`（`updated_at = 07:14:37Z`）。

「三件齐」满足：① 唯一查询参数 `&cb=<ns>`；② `Cache-Control: no-cache`；
③ 回读 `head_sha = 337590da8e` == 本次推送 SHA。**本机门控绿在此之上叠加了独立环境的复核**
——CI 的 **step 9** 独立复核**契约生成物无漂移**（本机对该步 `EBUSY`，见 §六），
**step 22** 独立复核**归档后规格全绿**（`openspec validate --archived`，与 §6.3 的 `--archived` 同源）。

## 八、遗留

1. **W76 台账的 5 条 `decidable_in_repo` 仍只登记了推荐**（`PD-09`／`PD-22`／`PD-24`／`PD-31`／`PD-34`）：
   每条都值得单独一轮，本版**不裁定**（与 W76 §八第 3 条一致）。
2. **harness 轨道 H18 与 B07 仍 `pending`**：判定面已交付，接线与真实证据是 runtime／外部工作。
3. **§56.2 情形③ 的清单是否还有其它不精确**：本版只订正「§56」一处；其余各项与 ADR 0049 决策 4
   逐项一致，**未发现矛盾**——但「未发现」≠「不存在」，若后续再撞上同类，仍按 §56.2 就地订正。
4. **`CAPABILITY_PACK_IDS` 仍为手写**（ADR 0056 遗留 4）；`cargo xtask schema --check` 本机仍 `EBUSY`
   （宿主对 node 派生任何子进程的限制）⇒ 该步**以 CI 结论为准**，**未写成「本机已通过」**。
5. §57.5 的待决策项与 §48.7 的全部事项继续有效；§18.2／§39.7 亦未变。

## 九、方法论教训

- **「一般规则」与「关于某一版的陈述」的分界 MUST 按对象判定，MUST NOT 按章号圈。**
  情形③ 原先按「§56」这个**章号**圈，把 §56.2（一般规则）与 §56.1／§56.3／§56.4（版本专属）**一并冻结**——
  而它自己的上一轮（E5）恰恰订正过 §56.2。⇒ **圈范围时圈的是「对象的性质」，不是「名字的层级」**。
- **「同一件事的判据」被两个实例共享时，写进基线的应是判据，不是任一实例的处置。**
  W76 追加勘误段、W78 就地订正——**处置不同而判据相同**；若只把「就地订正」或只把「追加勘误段」写死，
  就会否定另一个**同样正确**的实例。⇒ 写 **`判据 = 是否已进入提交`**，两个实例自动各归其位。
- **判据 MUST 可判。** 「轮次」在多人协作下无定义（每人说的「本轮」可能不同）⇒ 边界退化成约定；
  「是否已进入提交」有**唯一确定值** ⇒ 可判。**一个不可判的判据，写进基线等于没写。**
- **登记项连续八轮未收录，本身就是一种信号。** 从 W78 到 W86，每一轮 §八都要复述一次同一条——
  ⇒ **复述次数是「何时该开升基线轮次」的计量**，与 W73 的「不晚于下一次基线修订轮次」互补。
- **升基线的代价是文件重复（99.5% 相同），交换的是「每个被 ADR 批准的产物不可变」。**
  与 ADR 0049 相同的取舍：ADR 0049 的「15 hunk / 14 行 1:1 / 2713 行 / 226,578 字节」证据，
  本轮之后仍指向**同一个** `V3.10.md`，可独立复查。
- **校验器的「告警」在 `--strict` 下就是「失败」，而失败处往往是一处被合并的语义。**
  本轮把归档边界塞进既有 Requirement 的正文，代价不是「文案太长」，而是**一条 Requirement 里出现了两条行为**
  （分流 + 归档边界）——`>500 字符` 只是它的**症状**。⇒ 处置 MUST 是**拆开**（一条声明一条行为），
  MUST NOT 是「缩句让它过线」：后者会把「两条行为」这个**真缺陷**藏进更短的句子里。
- **刚立的规则立刻被自己用上。** E1 的边界（同一轮内未提交的产物 ⇒ 就地订正）在几分钟后的差量返工里
  就被适用了一次——`openspec/changes/archive/` 路径下的 w87 目录**尚未提交**，故**直接改、不加勘误段**。
  ⇒ 一条写在基线里的判据，其价值在它**第一次被用于裁决一件真事**时才被兑现。
