# W68：基础讲品循环（V3.9 §53、§5.3、§5.7、§5.4、§45.3；新增验收 B07）

- 日期：2026-10-12
- 变更：`openspec/changes/archive/2026-10-12-w68-basic-selling-loop/`
- 规格：`openspec/specs/basic-selling-loop/spec.md`（四条需求）
- 代码：`crates/nai-kernel/src/evaluate/selling.rs`、`crates/nai-kernel/tests/selling.rs`
- 登记：`tests/registry.yaml` 新增 **B07「基础讲品循环」**（`r1`、`pending`）

## 一、判断

本轮**没有**按上一次建议的第一项（A12 桌面壳宿主边界）推进，因为核实后发现该建议不成立。

| 我此前的说法 | 核实结果 |
| --- | --- |
| 「A12 只差声明式配置收敛」 | **不准确**。W54 已交付 `tauri.conf.json`／`capabilities/default.json`／`shell-policy.json` 三份**真实 Tauri 2 格式**的声明，外加 `tests/ui/desktop-shell.test.mjs` 15 项与四组非空洞守卫 |
| A12 剩下的真值 | 「可运行、可打包、可签名」的壳体——ADR 0046 的 R5 与 registry notes 早已写明缺失前置：**无 `tauri` crate、无 Rust 壳主体、未打包未签名** |
| 能否补齐 | **不能**。ADR 0046 **D3 明文否决**引入 `tauri`/`tauri-build`：它们在 Windows 上需要 WebView2 导入库且构建链访问网络 ⇒ 门控结论将取决于「本机是否装了 WebView2、网络是否可达」，即**主机事实混进仓库事实**（ADR 0041 已裁过的禁忌）。真实 WebView2 构建、安装器与 Windows 代码签名链属**发布里程碑** |

⇒ A12 与 **B01／O03／O04** 同类：判定机制已在位，**卡外部投入**。它不是「下一步」。

于是改推清单第 ③ 项，它反过来比 A12 更硬：**§53 基础讲品循环**——V3.9 自称「本版最重要的一条」，
而且核实后发现一个**可判定的真实缺口**。

## 二、方法：先核实再裁定

| 核实项 | 实测结果 |
| --- | --- |
| §53 的四项做法是什么 | 段落编排、插问后归位、重复冷却与段落切换、事实约束的行动提示 |
| 这四项是确定性规则还是需要模型 | **确定性**——§53 原句「前移的是**确定性脚本与规则**，不依赖工具规划，因此**不引入 §51.3 的任何能力包**」 |
| registry 里有对应条目吗 | **没有**。§48.5 家族索引写「B \| B01–B06」；§57.3 的 R1 清单（S05–S12／R04/R05/R08–R10／A02–A07、A12/A13／B01、B03、B05、B06／O01–O05／H01–H04、H15、H17、H18）无对应项 |
| 换个名字就能搜到吗 | **不能**。「讲品／段落编排／插问／节目／讲解／商品／卖点／售后」在 `tests/registry.yaml` 命中数**均为 0** |
| 它在 R1 的定义里吗 | **在**。§14.1 档位矩阵、§56 的「R1 = Lite」行、§2324 定义段「R1 = 单机、单会话、2D、LLM/TTS API、本地知识、**基础讲品循环**、…」 |
| 代码里已有实现吗 | **没有**。`grep '讲品\|selling\|pitch'` 在 `crates/` 无判定逻辑命中 |
| `evaluate::show` 是否已覆盖 | **部分且维度不同**。`show.rs`（195 行）判**单个 `ShowPlan` 自身**：场景/版本 id 一致、时长在范围、三类引用不超上限、打断策略在已知三值内、检查点**存在性**与匹配、敏感场景证据绑定 |
| 调度面的「冷却」是同一个吗 | **不是**。`scheduling-and-budgets` 的冷却是**观众/任务维度**（单观众占比上限、欠服务阈值），§53 的冷却是**段落维度** |
| §45.3 的禁用域有几类 | **5**：价格、库存、优惠与活动条件、承诺与售后政策、任何个体化承诺 |
| 阈值（冷却／最小间隔）能给具体数字吗 | **不能**。§48.7 把三项阈值列为待本场景回放实测且 MUST NOT 采外部观察值；§5.3 的数字明确是「场景策略初值，不是固定标准」 |

### 2.1 缺口的结构（一类可复用的模式）

「**门槛定义里有它、验收清单里没有它**」在本仓已出现第四次：

| 轮次 | 定义在哪里 | 缺口形态 |
| --- | --- | --- |
| W53 | A03「新增连接器不改内核」 | 命题**没有主语**（`plugins/` 下无连接器、`apps/` 下无 `platform-bridge`） |
| W54 | §1248／§1260 的桌面壳产物 | registry 66 条里**无**桌面壳条目；44 篇 ADR 里**无**形态裁决 |
| W55 | 23 文件／116 处平台条件编译 | registry 里平台维度**零表达** |
| **W68** | §53 基础讲品循环（R1 定义句） | registry 里**零条目**，且 B 系列索引停在 B06 |

⇒ 这类缺口的判据是同一条：**「这件事做完了没」无法被机器回答**。处置也同一条：
补可判定的条目 + 把规则落成纯逻辑判定面。

### 2.2 不成立的做法（本轮避开）

| 做法 | 否决理由 |
| --- | --- |
| 并入 `evaluate::show` | 两者输入形状与失败类别不同；本仓已第三次因「维度不同」拒绝合并（前两次：不复用 `bridge::Degradation`、不把爬坡并进 `harness.rs`） |
| 并入调度面 | 「冷却」一词相同、所指不同（观众维度 vs 段落维度），合并制造歧义 |
| 扩展 `scheduling-and-budgets` 规格 | 讲品循环的四条规则是一个整体的、可独立验收的性质，不是调度的一个分支 |
| 只做判定面、不立条目 | 那就仍是「有定义、无验收」——正是本轮要消除的状态 |
| 给冷却／间隔内建一套「看起来合理」的默认值 | §48.7 明文 MUST NOT 采外部观察值；阈值必须由调用方给 |
| 升级 B07 为 `implemented` | 完整验收需要真实场次讲品；本轮交付的是判定面 |
| 顺手把基线 §48.5／§57.4 的家族范围与计数改掉 | 改基线是升基线轮次的事，且 MUST 是 1:1 行替换（§56.2）；本轮是**新增条目**，不是订正既有句子 |

## 三、交付

### 3.1 §53 的四行拆成四条规则，不合并

| §53 行 | 判的对象 | 可判定形式 |
| --- | --- | --- |
| 段落编排 | 一整个序列 | 四段是否齐备且顺序正确 |
| 插问后归位 | 一个检查点 + 一次恢复 | 检查点内容是否越界、恢复位置是否落在检查点段落 |
| 冷却与切换 | 一串进入时刻 | 同段冷却与相邻切换间隔是否满足 |
| 事实约束的行动提示 | 一条提示里的若干断言 | 受约束断言是否绑定当前有效事实版本 |

四者**互不短路**：一个既缺段又乱序、还带着坏检查点与过期事实引用的计划，会一次报出**全部**类别。

### 3.2 段落编排：判序列，不判集合

`SEGMENT_ORDER` = **卖点 → 规格 → 适用场景 → 售后说明**，逐项照抄 §53；并由
`segment_order_matches_the_baseline_line` **写死**（只校验「四条、label 互异」的守卫，
对内容被改小是瞎的——W66 的教训）。

`PitchPlan::segments` 的类型是 `&[&str]`（段名文本）而**不是** `&[SegmentKind]`，这是刻意的：
这是唯一能让「出现未定义的段落名」被**表达**出来的输入形状。用枚举的话
`PitchRejection::UnknownSegment` 永远不可能触发——一个开不了火的守卫。

### 3.3 插问后归位：`carried_text` 字段是必要的建模选择

§5.7 原文：「检查点保存段落与已完成句，**不保存未播出的生成文本**；恢复前重新校验产品、价格、
活动、素材与授权，重启默认待机。」

```rust
pub struct Checkpoint {
    pub segment: SegmentKind,
    pub completed_sentence_ids: Vec<String>,
    pub carried_text: Option<String>,   // 未播出的生成文本；MUST 为 None
}
```

`carried_text` 不在 §5.7 的字面上，但没有它，「检查点带了未播出文本」这件事在**类型上无法表达**，
测试也就构造不出该违规 ⇒ 守卫会恒真。**先想清楚坏实现的形状，再定数据结构**，
否则断言写得再漂亮也只是在验证一件不可能发生的事。

### 3.4 冷却与切换：两条规则分开，且前置条件具名

- `cooldown_not_elapsed`：**同一段落**两次进入的间隔 < 冷却；
- `switch_too_soon`：**任意相邻两次**进入的间隔 < 最小切换间隔。

阈值来自 `PitchPolicy`，模块**不内建**。两条**分开**报——因为 §53 写的是两条规则，
合并后「破坏的是哪一条」就不可判了。

进入时刻序列须**非降序**：两条规则都是关于**间隔**的陈述，而间隔在递减序列上没有定义。
故先判前置条件、再判规则；前置条件失败**具名**（`entrance_times_decrease`），不会被读成「通过」，
且区间规则**不再评估**——顺带堵死一类实现缺陷：直接相减在递减序列上会**下溢**。

### 3.5 事实约束：样本必须落在判据上

`CLAIM_KINDS` = §45.3 禁用域五类，逐项写死（`claim_kinds_match_the_disabled_domain_line`）。

两条判定**分开**：

- `action_cue_without_fact_binding`：受约束断言**没有**事实引用（§45.2：「语义相似不构成证据」）；
- `action_cue_fact_stale`：有引用，但引用**不是当前有效版本**（§5.4 evidence 阶段、§45.4 失效）。

「引用过期」这条的守卫样本刻意取**同 object、不同 revision**。原因：若样本改成「不同 object」，
一个只比 object_id 的实现照样通过 ⇒ 那条守卫就是自欺。注入 4 验证了这一点（见 §4.2）。

### 3.6 具名拒绝（10 条）

`PitchRejection` = `unknown_segment`／`segment_missing`／`segment_out_of_order`／
`checkpoint_holds_unspoken_text`／`resume_diverges_from_checkpoint`／`entrance_times_decrease`／
`cooldown_not_elapsed`／`switch_too_soon`／`action_cue_without_fact_binding`／`action_cue_fact_stale`。

外加一条总账断言：`every_rejection_is_constructible_and_none_is_unregistered` 构造出的拒绝集合
**恰为** `PitchRejection::ALL`——既证每条可构造，也防未登记的名字。

## 四、验证

| 项 | W67 | W68 | 增量 | 说明 |
| --- | --- | --- | --- | --- |
| `cargo xtask check --profile full` | EXIT=0 | **EXIT=0** | — | 第 **4** 次尝试通过（`CARGO_BUILD_JOBS=1`） |
| 测试目标 | 89（71 + 18） | **90（72 `Running` + 18 `Doc-tests`）** | **+1** | 新增 `crates/nai-kernel/tests/selling.rs` |
| 测试项 | 798 | **817 passed / 0 failed** | **+19** | 恰等于新增的 19 项测试 |
| `dependency boundaries` | 19 包 | **19 包** | 0 | 无新增依赖 |
| `admission registry check` | passed | **passed** | — | 新增 B07 后三向断言仍成立 |
| 前端门控 | 22 项 | **22 项** | 0 | 未触碰前端 |

**两个增量互相对照**：目标 +1 与项 +19 恰好对应新增的那一个测试目标与其 19 个用例，
这是「未误改别处」的旁证。

**关于 4 次尝试**：第 1 次（默认并行）与前两次（`JOBS=1`）失败。**同一份代码第 4 次通过
⇒ 前三次失败与代码无关**——这是本轮的核心论证，而它不依赖逐次取证。
失败详情因每次重写同一日志文件而**未留存**，故本报告**不宣称**已逐次定位根因；
「本机已登记的 LNK1104 批量链接争用」是**与本轮现象一致的既有记录**，不是本轮实测。

**OpenSpec 两条口径**（互不替代，都跑）：

| 口径 | W67 | W68 | 增量 |
| --- | --- | --- | --- |
| `validate --all --strict`（规格） | 42 | **43 passed / 0 failed** | **+1**（新能力 `basic-selling-loop`） |
| `validate --all --strict --archived`（已归档变更） | 65 | **66 passed / 0 failed** | **+1** |

### 4.1 本轮踩到的一个新坑：新建能力的 `## Purpose` 占位符

`openspec archive` 在**创建新能力**时会往主规格里写一行占位符：

```
## Purpose
TBD - created by archiving change w68-…. Update Purpose after archive.
```

`validate <change> --strict`（**变更**口径）**不报**这个，`validate --all --strict`（**规格**口径）
才报一条 ⚠，且**首轮实测时它表现为 `42 passed, 1 failed`**——即 strict 的失败计数里。
`--all --strict --archived` 也**看不出来**（它只含归档变更目录）。

⇒ 这正是记忆里 W56 那条坑的**同族**：「需求正文 >500 字符只在 `specs/` 里失败，变更差量里不报，
归档前看不出来」。新增一条：**新建能力的轮次必须在归档后补写 `## Purpose`**，
且补写要**直接改主规格**——提示语已说明「a `## Purpose` in a delta is read only when the capability is
created, so it cannot replace this one」。补写后两条口径均为 0 failed。

### 4.2 非空洞自检：五组注入，各自被抓住

| 注入（坏实现） | 被哪些测试抓住 |
| --- | --- |
| 段落判定改成**比较集合**（排序后比较） | `the_right_segments_in_the_wrong_order_are_refused`、`a_plan_that_is_short_and_out_of_order_reports_both`、`every_rejection_…`（**3 项**） |
| 缺段**只报第一个** | `a_missing_segment_is_refused_and_named`、`a_plan_that_is_short_and_out_of_order_reports_both`（**2 项**） |
| 悄悄**忽略未知段名** | `an_undefined_segment_name_is_refused`、`a_plan_that_is_short_and_out_of_order_reports_both`、`every_rejection_…`（**3 项**） |
| 事实检查只问「**引用是否存在**」 | `a_claim_standing_on_a_revision_no_longer_in_force_is_refused`、`every_rejection_…`（**2 项**） |
| 区间规则**不做前置条件门控** | `a_decreasing_entrance_series_is_refused_by_name`（**1 项**） |

注入 4 是本轮最有价值的一次：它命中的**正是**那条为「同 object、不同 revision」设计的样本。
若样本换成「不同 object」，这个坏实现会全绿通过。

### 4.3 契约零漂移的取证方式

`cargo xtask schema --check` **在本机跑不了**（`generate.mjs` 内的 `spawnSync(process.execPath, …)` 报
`EBUSY`，managed 与 system node 都复现 ⇒ 主机对「node 自旋 node」的限制）。**故不宣称该检查已通过**，
改用两条互补证据：

1. **结构性**：生成器输入只有 `schemas/catalog/capabilities.json` 与 `schemas/{config,plugin}/*.schema.json`。
   本轮改动为 `crates/nai-kernel/src/evaluate/{mod,selling}.rs`、`crates/nai-kernel/tests/selling.rs`、
   `tests/registry.yaml`（新增一条 `pending` 条目，**未新增契约名**）与文档 ⇒ 漂移不可能发生；
2. **CI 独立步骤**：`.github/workflows/foundation.yml` 的
   「Check generated contracts without modifying consumers」在 Linux 与 Windows 上各跑一次，push 后核对。

<!--CI-->

## 五、验收侧：为什么 B07 是 `pending` 而不是 `implemented`

| 判定面已交付 | 完整验收仍缺 |
| --- | --- |
| 「这个编排计划是否合法」可判定 | **段落内容与话术**：§53 的四个段落名是基线给的，但每一段讲什么、怎么讲是内容创作 |
| 「检查点是否越界、落点是否正确」可判定 | **恢复前的重校验**：§5.7 的「重新校验产品、价格、活动、素材与授权」由 §5.4 守门与知识层执行 |
| 「按给定阈值是否满足」可判定 | **阈值本身**：§48.7 待本场景回放实测，MUST NOT 采外部观察值 |
| 「受约束断言是否有依据」可判定 | **TTS 时长与真实播放**：属 media 与 provider 层 |

判定面交付 ≠ 验收达标。故本报告**不出现**「讲品循环已可运行」或「检查点恢复已端到端可用」这类措辞。

## 六、未做与遗留

1. **基线偏差（本轮新登记）**：新增 B07 后，V3.9 §48.5 的「B \| B01–B06」与 §57.4 的计数行
   「slice_0 10 + r1 36 + r2 9 + capability_pack 29 + r3 1 + experimental 2 = **87**（本版只改归属，不加条目）」
   **不再等于实际值**。本轮**不改基线文本**（那是升基线轮次的事且 MUST 是 1:1 行替换），
   偏差在报告、`docs/kernel.md`、`docs/development-plan.md`、`AGENTS.md` 与 B07 的 `notes` 里如实登记，
   **留待下一轮升基线一并收录**——与 W54 新增 A12、W55 新增 A13 的处置一致。
2. **运行时接线**：段落内容生产、TTS 时长与回放采样、把决策日志记录真正落盘。
3. **§5.3 的其余条款**未纳入本轮判定面：聚类合并、节目感知预合成、价值/秒排序、每位观众冷却与占比上限
   ——其中部分已在 `scheduling-and-budgets` 的判定面里（观众维度），**剩余者不属 §53**。
4. **未触碰**：A12（卡外部投入，见 §一）、O01／O02／O05（需真实候选来源与生产接线）、B01／O03／O04
   （需 100 题集、真机、干净 Windows 安装等外部投入）、M 系列与 H05–H14/H16（能力包）。
5. **A12 仍然 `pending`**：本轮只**核实**了它不可推进，未改动它任何字段。

## 七、方法论教训

1. **「只差 X」这类判断必须核实**。上一轮我把 A12 说成「只差声明式配置收敛」，
   而它的 registry `notes` 与 ADR 0046 的 R5 **早就写明**缺失前置是真实依赖、可运行壳体与打包签名链。
   建议里出现「只差接线」时，先回去读那条 notes——**别人的登记比自己复述建议可靠**。
2. **「门槛定义里有它、验收清单里没有它」是一类可判定的缺口**（§2.1 已是第四次）。
   判据统一：「这件事做完了没」能否被机器回答；处置统一：补条目 + 落判定面。
   **查这类缺口的方法是查定义句与清单的交集，而不是查清单本身**。
3. **数据结构的形状决定守卫能不能开火**（§3.3）。`carried_text` 与
   `PitchPlan::segments: &[&str]` 都是为了让违规**可表达**而选的——用枚举就没有 `unknown_segment`，
   不给字段就没有 `checkpoint_holds_unspoken_text`。**先定坏实现的形状，再定类型**。
4. **前置条件要具名，而不是静默跳过**（§3.4）。递减序列上不判间隔是对的，
   但「不判」必须**同时**报出「为什么判不了」；否则「跳过」与「通过」在日志里长得一样。
5. **同一轮里能同时用上前面几轮的教训，是因为它们被写进了 design 而不是只有报告**
   （§3.5 的样本设计直接引 W67 的教训，§3.2 的内容守卫直接引 W66 的）。
   **教训落在 design 里才在下一次开工时生效。**
