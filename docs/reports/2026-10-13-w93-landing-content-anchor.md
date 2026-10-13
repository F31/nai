# W93：`decided_in_repo` 的落地点 MUST 带内容证据 —— `landing` 补上锚点

- 日期：2026-10-13
- 变更：`openspec/changes/w93-landing-content-anchor/`
- 相关：[ADR 0071](docs/adr/0071-landing-content-anchor.md)、
  [ADR 0066](docs/adr/0066-decided-in-repo-landing.md)（第六态与 `landing` 的由来）、
  [ADR 0070](docs/adr/0070-decidable-in-repo-settlement.md)（W92 把该态从 2 条扩到 4 条）

## 一、判断

W92 清空 `decidable_in_repo` 之后，`decided_in_repo` 从 **2 条变成 4 条**。该态断言的是
「**决定已作出，且已落地**」。本轮要问的是：**这句断言真的被判据守住了吗**。

答案是**没有**：`landing` 的判据只钉「每个元素是本仓库已跟踪的路径」——它回答「**落在哪个文件**」，
**不**回答「**那个文件里的东西还在不在**」。W92 把这个态扩大，也就把这个洞 expanded 到了 4 条上。

## 二、核实（先读后判）

### 2.1 两个探针：洞是真的

改真实文件 → 跑 `node --test tests/registry.test.mjs` → sha256 还原：

| 探针 | 改动 | 结果 |
| --- | --- | --- |
| ① | `PD-22.landing` 从 `openspec/specs/agent-harness/spec.md` 换成 `README.md`（**真实存在**、`git ls-files --error-unmatch` 确认**已跟踪**） | **50 pass / 0 fail** |
| ② | `landing` 不动，把 `PD-24` 在规格里的落地句「**R1 首版委派链深度上限 MUST 为常量、MUST NOT 提供配置面**」删掉 | **50 pass / 0 fail** |

⇒ 指着**任意一个已跟踪文件**都算落地；落地内容被**撤销**了也仍然算落地。这是
「**已声明 ≠ 已执行**（无执行体即假契约）」的形状。

### 2.2 另两个"看起来已结清"的态：实测无可订正的数据

用来决定「本轮做哪一格」，而不是想当然地全做：

- **`answered_by_baseline` 4 条**：§46.2 表格明写 sampling「**默认禁用**」+ 启用四项条件；
  §41.4 明写「利用率 MUST 作为观测项登记，但**本轮不设告警阈值**」；§46.5 明写
  「**不新增** `mcp.tool@1`／`a2a.*` 之类的契约名」；§51.3 的能力包清单逐行独立
  （MCP = H11／H12、A2A = H13／H14）。⇒ **4 条 `basis` 都真的答出了问题**，无订正可做。
- **`needs_precondition` 4 条**：H05／H06／H07／H08／H13／H14／H16 **全部** `pending`
  ＋ `capability_pack` ＋「未开始」；PD-30 挂的 `W61`（MCP 适配）其 H11／H12 也 `pending`。
  ⇒ **前置全部未达成**，无迁移可做。

⇒ 本轮**只补 `decided_in_repo` 这一格**。

## 三、裁定

补 **内容锚点**：新增字段 `landing_anchor`，与 `landing` **等长、逐项对应**，第 *i* 个锚点 MUST
真实出现在第 *i* 个落地点文件的**内容**里。四个设计取舍：

| 取舍点 | 采用 | 排除与理由 |
| --- | --- | --- |
| 形状 | **平行数组**（新增 `landing_anchor`） | 排除「把 `landing` 改成 `[{path, anchor}]`」——那会重写 `REPO_PATH_FIELDS` 的解析器、改 4 条既有数据形状，并打破「元素是字符串」这个既有前提。平行数组是**纯增量**，且守住「**不合并维度**」 |
| 类别 | **第五类 `ANCHOR_FIELDS`** | 锚点是"文件内容证据"：不是 `§NNN`、不是登记 id、不是路径；也不归 `STRUCTURAL_FIELDS`（结构字段的判定只用到字段自身，而锚点必须与另一个字段配对、并读到另一个文件才能判）。与四类**两两不相交** |
| 强制性 | **耦合断言 `COUPLED_REQUIRED_FIELDS`**（同生同灭且等长） | 锚点若**可选**，"不给锚点"就是绕过判据的那条缝——洞原样还在。不动 `REQUIRED_FIELD`（一态一个必填字段）的形状：那是**另一个问题** |
| 输入 | **新增依赖 `readTrackedText`**，读不到 MUST 具名失败 | 少了它「内容真的在」会退化成恒真——**与 `repositoryFiles` 是同一个陷阱的第二次出现**。读不到时 MUST 报，MUST NOT 静默降级 |

## 四、实现

- `scripts/contracts/pending-decisions.mjs`：
  - 新增 `ANCHOR_FIELDS`、`COUPLED_REQUIRED_FIELDS`；`LEDGER_FIELDS` **11 → 12**；
  - `shapeErrors()`：类别划分**四类 → 五类**，并新增**耦合表自洽**断言（两个字段都 ∈ `LEDGER_FIELDS`、
    分属**不同**类别、同为清单型）；
  - `validatePendingDecisions`：`required` 加 `readTrackedText`；新增耦合检查与锚点检查；
    触发条件一律**按字段存在**，与 `disposition` 无关；
  - `REFERENCE_FIELDS` **不**纳入锚点——它不需要被解析，需要被**读到并比对**。
- `tests/pending-decisions.yaml`：4 条 `decided_in_repo` 各配真实锚点（**数据改动**）。
- `tests/registry.test.mjs`：**50 → 51**（新增 1 个用例）；注入 `readTrackedText`、缺输入清单加它、
  字段数 11 → 12、类别清单四类 → 五类、修 W88 反向用例的前提。

**四条锚点**（逐条实测存在）：

| 条目 | 落地点 | 锚点 |
| --- | --- | --- |
| PD-09 | `crates/nai-security/Cargo.toml` | `[target.'cfg(windows)'.dependencies]` |
| PD-09 | `crates/nai-security/src/lib.rs` | `NativeVault` |
| PD-09 | `schemas/platform-matrix/platforms.json` | `secret-protection` |
| PD-22 | `openspec/specs/agent-harness/spec.md` | `R1 首版入站只响应本机与本地信任配置中已登记的受信对端` |
| PD-24 | `openspec/specs/agent-harness/spec.md` | `R1 首版委派链深度上限 MUST 为常量` |
| PD-31 | `crates/nai-kernel/src/evaluate/memory.rs` | `viewer_memory_enabled: false` |

锚点取「**承载该决定的那一段**」，不取「文件里任意一句话」：`[target.'cfg(windows)'.dependencies]`
是「只在 Windows 引入 keyring」这一决定的承载处。**PD-22 与 PD-24 落地点相同、锚点不同** ⇒
正说明「落地点相同 ≠ 决定相同」——没有锚点，两条 `landing` 完全无法区分。

**锚点守什么、不守什么（如实披露）**：把门槛从"路径存在"提高到"**所指文件里真有那段内容**"，
但**不**验"这个串只应出现在那里"（指到另一个恰好含该锚点的文件仍绿），也**不**验那段内容
**语义上**兑现了决定（仍由人读，ADR 0054 决策 2）⇒ MUST NOT 被读作"已落地已被证明"。

## 五、非空洞注入（P1/P2 + F1–F8，全部真跑）

| # | 注入 | 结果 |
| --- | --- | --- |
| **P1** | 探针①复现：`PD-22.landing` → `README.md` | **红**（`PD-22` + `landing_anchor` + `README.md`） |
| **P2** | 探针②复现：删规格里 `PD-24` 的落地句 | **红**（`PD-24` + `is not present in`） |
| F1 | 锚点改成文件里没有的串 | **红**（`PD-31` + `is not present in`） |
| F2 | `landing_anchor` 少一项（长度不等） | **红**（`PD-09` + `correspond one-to-one`） |
| F3 | 有 `landing` 无 `landing_anchor` | **红**（`PD-22` + `landing_anchor is missing`） |
| F4 | 锚点给空串 | **红**（`PD-24` + `must be non-empty strings`） |
| F5 | 锚点挂到**别的处置**上且不在文件里 | **红**（`PD-01` + `landing_anchor`） |
| F6 | 反向：合法锚点挂到别的处置上 | **不红**（51/0） |
| F7 | `LEDGER_FIELDS` 漏登记 `landing_anchor` | **红**（`unknown field landing_anchor`） |
| F8 | 类别并集漏掉 `ANCHOR_FIELDS` | **红**（`allowed by LEDGER_FIELDS but no check category claims it`） |

**P1／P2 由绿转红是本轮的成果**：同样的两处改动，改前 50/0 全绿、改后 46/5 且具名命中。
每次注入后 **sha256 逐字还原（校验 OK）**。各注入点红的用例数是 **3–8** 个不等（前提共享：
主判据 + 若干含"期望 clean"前提的用例），**不是**判据重叠。

⚠️ **注入过程中的两处自我纠错**，记录以免重犯：

1. **F1 的探针值一度选错**：用了 `viewer_memory_enabled: true`，而它**真的存在**于
   `crates/nai-kernel/src/evaluate/memory.rs`（W92 核实阶段就见过 line 1049）⇒ 判据不响，
   得到**假阴性**。危险在于：输出看起来像"判据失灵"，实际是**我选错了值**。
2. **F7 的替换只替换了注释行的前缀**，把后半句留成了裸代码 ⇒ 模块加载失败、测试报
   `pass=0 fail=1`。那是**注入自身**的错，不是判据在响——`pass=0` 是这类错误的特征信号。

## 六、验证

### 6.1 分面

| 面 | 命令 | 结果 |
| --- | --- | --- |
| 台账判据 | `node --test tests/registry.test.mjs` | **50 → 51，全绿** |
| 变更校验 | `openspec validate w93-landing-content-anchor --strict` | **valid** |
| 归档前全量 | `openspec validate --all --strict` | **44 passed / 0 failed**（含活动变更；归档后见 §6.3） |

### 6.2 全量门控

`cargo xtask check --profile full` = **EXIT=0**（完整日志实测，非抽样）：

| 指标 | 值 | 与 W92 对比 |
| --- | --- | --- |
| `test result` 行数（目标数） | **91** | 一致 |
| passed 合计 | **862** | 一致 |
| failed 合计 | **0** | 一致 |
| `LNK1104` 命中 | **0**，门控**一次通过**（日志中未观测到预链接重试） | W92 是第 1 轮即收敛 |
| `cargo fmt --all --check`（独立复跑） | **EXIT=0** | 一致 |
| frontend 步骤 | 「全部步骤通过（工程基础，非渲染/人工 UI 验收）」 | 一致 |

**Node 侧 5 个套件全绿**：`tests/registry.test.mjs` **51/51**（**本轮改动，W92 是 50**）、
`tests/contracts/schema.test.mjs` 30/30、其余 22/5/52。

Rust **零改动**、`cargo xtask` **零改动**、`tests/registry.yaml` **零改动** ⇒ 与 W92 同形
（**91 目标 / 862 passed**，两侧数字不变属预期，不是"门控没跑到新判据"——Node 侧 registry 的
**51/51** 就是本轮判据在门控里跑过的证据）。

### 6.3 归档与两条 validate

归档：`openspec archive w93-landing-content-anchor --yes` ⇒ `spec-driven-delivery: update`
（`+ 1 added, ~ 2 modified`；Totals `+ 1, ~ 2, - 0, → 0`），归档为
`2026-10-13-w93-landing-content-anchor`；活规格已含新增 Requirement
`Every landing site carries content evidence that is actually there` 与其 6 个 Scenario。

| 命令 | 结果 |
| --- | --- |
| `openspec validate --all --strict`（**归档前**） | **44 passed / 0 failed**（含活动变更） |
| `openspec validate --all --strict`（**归档后**） | **43 passed / 0 failed**（与 W92 的 44 → 43 同形） |
| `openspec validate --all --strict --archived`（归档后） | **91 passed / 0 failed**（W92 的 90 → 91） |

⚠️ **一处流程自查**：归档时 CLI 报「2 incomplete task(s)」——本轮归档前只勾了 **9/11**。
缺的两项（报告+门控、归档+提交+CI）在归档**之后**才全部完成，故在归档目录里
**就地补勾**（该目录本轮尚未提交 ⇒ 属「同轮未提交就地订正」，MUST NOT 留成不实的历史记录）。

## 七、**CI_PLACEHOLDER**

## 八、遗留

1. **锚点不验语义兑现**：那段内容是否真的兑现了那个决定，仍由人读（ADR 0054 决策 2）。
2. **锚点不验唯一性**：指到另一个恰好含该锚点的文件仍会绿。要堵这个洞需要"该串只应出现在那里"
   的判据，而它无法在不引入脆性的前提下判定 ⇒ **未做**。
3. **另两态的判据缝本轮未动**：`answered_by_baseline` 的 `basis` 只钉「至少引用一个 `§NNN`」，
   **不**钉「那节真的答出了这个问题」；`needs_precondition` 的 `precondition` 只钉「id 解析得开」，
   **不**钉「前置现在是否已达成」。实测这两态**当前无可订正的数据**（§2.2），
   故本轮只登记，MUST NOT 读成已处理。
4. **A2A 仍是 `capability_pack`**：`delegation_depth` 在 Rust 零命中；本轮是**判据层**的改动，
   MUST NOT 被读作"A2A 已可用"。
5. **W90 §八遗留 1／3／4／5、W91 的跨语言键级消费、W92 的基线收录**均**未动**。

## 九、方法论教训

1. **「必填字段存在」不等于「这个态真的付了账」。** `decided_in_repo` 有 `landing`、判据也验了它，
   但验的是**路径**，而该态断言的是**落地**。⇒ 每加一个"必填字段"，都要问一句：
   **它钉的那个可判事实，是否就是这个态在声明的那件事**。
2. **一次合法的判据增强会让既有用例的"前提"失效。** W88 的反向用例「可解析 `landing` 挂别的处置上
   ⇒ 不红」注入时只给 `landing` ⇒ 现在会红，而红的原因是**缺内容证据**而不是"放错位置"。
   修法是**补齐前提**，MUST NOT 用 `if (!x) continue` 之类的短路把判据关掉——那正是
   W92「清空一个态」的同族陷阱。
3. **探针值 MUST 先确认它「不存在」。** F1 拿 `viewer_memory_enabled: true` 当"不存在的串"，
   而它真的存在（W92 就见过）⇒ 得到**假阴性**，且输出看起来像判据失灵。
   ⇒ **「红」不等于「红在我以为的地方」**（W92 已记一次，本轮第二次以不同形式复现：这次是**该红没红**）。
4. **`pass=0 fail=1` 是"注入自身坏了"的特征信号。** F7 把注释行截断成裸代码 ⇒ 模块加载失败。
   看到 `pass=0` 时 MUST 先怀疑注入脚本，MUST NOT 把它当成"判据在响"。
5. **「可选」是判据的缝。** 锚点若做成可选，"不给锚点"就成了绕过它的那条路。
   ⇒ 新增一条守卫时，MUST 同时问「不给它会怎样」——答案若是"照旧通过"，那条守卫就是装饰。
6. **两个字段按位置对应，就要有长度断言。** 平行数组的代价是"对应关系"没有结构保障；
   `COUPLED_REQUIRED_FIELDS` 的等长断言与"锚点在对应文件里"是**两道**互补的锁，缺一不可。
