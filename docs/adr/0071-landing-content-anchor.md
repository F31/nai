# ADR 0071：`decided_in_repo` 的落地点 MUST 带内容证据 —— `landing` 补上锚点

- 状态：接受
- 日期：2026-10-13
- 相关：[ADR 0054](0054-pending-decisions-ledger.md)（台账六态与「语义刻意留给人读」的决策 2）、
  [ADR 0066](0066-decided-in-repo-landing.md)（第六态与 `landing`；本 ADR 补上它的必填字段**真正付账**的部分）、
  [ADR 0070](0070-decidable-in-repo-settlement.md)（W92 把 `decided_in_repo` 从 2 条扩到 4 条，
  本 ADR 承接它随之变大的风险）、
  [ADR 0053](0053-repository-scoped-evidence.md)（仓库成员集 MUST 由 `.git/index` 派生）、
  `tests/pending-decisions.yaml`、`scripts/contracts/pending-decisions.mjs`、`tests/registry.test.mjs`、
  `docs/reports/2026-10-13-w93-landing-content-anchor.md`、
  `openspec/changes/archive/2026-10-13-w93-landing-content-anchor/`

## 背景

`decided_in_repo` 这个态断言的是「**决定已作出，且已落地**」。它的必填字段是 `landing`（落在仓库的
哪里），而判据只钉一件事：**每个元素是本仓库已跟踪的路径**（ADR 0066 立这条时，把"落地点可判"
当成"已落地可判"）。

`landing` 回答的是「**落在哪个文件**」，**不**回答「**那个文件里的东西还在不在**」。这两者之间那一段
此前没有任何判据看守。W92 把该态从 **2 条扩到 4 条**（PD-22／PD-24 落到活规格），风险随之变大。

## 证据（本轮实测）

### 1. 两个探针：「已落地」是一条被判据放过的空声明

改真实文件 → 跑 `node --test tests/registry.test.mjs` → sha256 还原：

| 探针 | 改动 | 改前结果 |
| --- | --- | --- |
| ① | `PD-22.landing` 从 `openspec/specs/agent-harness/spec.md` 换成 `README.md`（**真实存在**、`git ls-files` 确认**已跟踪**） | **50 pass / 0 fail** |
| ② | `landing` 不动，把 `PD-24` 在规格里的落地句「**R1 首版委派链深度上限 MUST 为常量、MUST NOT 提供配置面**」删掉 | **50 pass / 0 fail** |

⇒ 指着**任意一个已跟踪文件**都算落地；落地内容被**撤销**了也仍然算落地。这是
「**已声明 ≠ 已执行**（无执行体即假契约）」的形状——台账说"已落地"，判据只验"路径存在"。

两个探针在本轮之后 **MUST 变红**（见「非空洞注入」的 P1／P2）。

### 2. 另两个"看起来已结清"的态：实测无可订正的数据

- `answered_by_baseline` 4 条：§46.2 表格明写 sampling「**默认禁用**」+ 启用四项条件；§41.4 明写
  「MUST 作为观测项登记，但**本轮不设告警阈值**」；§46.5 明写「**不新增** `mcp.tool@1`／`a2a.*`」；
  §51.3 的能力包清单逐行独立（MCP = H11／H12、A2A = H13／H14）。⇒ **都真的答出了**。
- `needs_precondition` 4 条：H05／H06／H07／H08／H13／H14／H16 **全部** `pending` ＋ `capability_pack`
  ＋「未开始」；PD-30 挂的 `W61`（MCP 适配）其 H11／H12 也 `pending`。⇒ **前置全部未达成**。

⇒ 本轮**只补 `decided_in_repo` 这一格**，另两态的判据缝（见遗留 3）**不在本轮范围内**。

## 决定

### 决定 1：锚点是**平行数组**，不并进 `landing`

新增字段 `landing_anchor`，与 `landing` **等长、按位置对应**；第 *i* 个锚点 MUST 真实出现在第 *i* 个
落地点文件的**内容**里。

**排除**：把 `landing` 改成 `[{path, anchor}, …]`——那会让 `REPO_PATH_FIELDS` 的解析器重写、
4 条既有数据全部改形状，并打破「元素是字符串」这个既有前提。平行数组是**纯增量**，
且守住「**不合并维度**」：路径与内容是两样东西，写成两层而非塞进一个元素。

等长由**耦合断言**钉住；位置错位由「锚点在对应文件里找不到」兜住。

### 决定 2：锚点是**第五类**检查，不归入既有的四类

新增 `ANCHOR_FIELDS = ['landing_anchor']`，与 `CITATION_FIELDS`（`§NNN`）／`ID_LIST_FIELDS`（登记 id）／
`REPO_PATH_FIELDS`（仓库路径）／`STRUCTURAL_FIELDS`（`id`／`disposition`）**两两不相交**。

它不归 `STRUCTURAL_FIELDS`：结构字段的判定只用到字段自身，而锚点必须与**另一个字段**配对、
并读到**仓库里另一个文件的内容**才能判。`REFERENCE_FIELDS`（"可解析引用"的扫描域）**不**纳入它——
锚点不需要被解析成章节／id／路径，它需要被**读到并比对**。

新增类别随即被既有的闭世界断言在两个方向上各点名一次（见 F8）。

### 决定 3：`landing` 与 `landing_anchor` MUST 同生同灭（耦合断言）

若锚点**可选**，"不给锚点"就是绕过本轮判据的那条缝——洞原样还在（与 W81「清单之外的字段不受检」
同形）。⇒ 新增 `COUPLED_REQUIRED_FIELDS`，判据：两者必须同时存在且等长。

**不动 `REQUIRED_FIELD`**（一态一个必填字段）的形状：把它扩成"字段列表"会牵动
`REQUIRED_FIELD.get` 的每个使用点；而「这一态的必填字段是谁」与「两个字段之间是什么关系」
是**两个不同的问题** ⇒ 单独一张表、单独的判据（与 W73 的耦合断言同形）。
触发条件同样是**字段是否存在**，与 `disposition` 无关。

### 决定 4：文件内容由 `readTrackedText` 注入，读不到 MUST 具名失败

`deps` 增一项 `readTrackedText`（同步，`(path) => string | undefined`），进入 `required` 清单
（缺了 MUST 报 `inputs are required`）。少了它，「内容真的在」会退化成恒真——
**与 `repositoryFiles` 是同一个陷阱的第二次出现**。

读不到文件时 MUST 报 `… is unreadable … the anchor check must not silently pass`；读的过程中抛错
MUST 报 `could not read …`。⇒ 「**没看到**」与「**出错了**」在类型上分开（铁律二）：
若两者都当通过，一次宿主异常就会把这条判据整体关掉。

### 决定 5：四条的真实锚点（逐条实测存在）

| 条目 | 落地点 | 锚点 |
| --- | --- | --- |
| PD-09 | `crates/nai-security/Cargo.toml` | `[target.'cfg(windows)'.dependencies]` |
| PD-09 | `crates/nai-security/src/lib.rs` | `NativeVault` |
| PD-09 | `schemas/platform-matrix/platforms.json` | `secret-protection` |
| PD-22 | `openspec/specs/agent-harness/spec.md` | `R1 首版入站只响应本机与本地信任配置中已登记的受信对端` |
| PD-24 | `openspec/specs/agent-harness/spec.md` | `R1 首版委派链深度上限 MUST 为常量` |
| PD-31 | `crates/nai-kernel/src/evaluate/memory.rs` | `viewer_memory_enabled: false` |

锚点取「**承载该决定的那一段**」，不取「文件里任意一句话」：`[target.'cfg(windows)'.dependencies]`
是「只在 Windows 引入 keyring」这一决定的承载处，而不是 `keyring` 这个词的任意出现。

**PD-22 与 PD-24 的落地点是同一个文件、锚点不同** ⇒ 这正说明「落地点相同 ≠ 决定相同」：
没有锚点，两条 `landing` 完全无法区分。

## 锚点守什么、不守什么（MUST 如实披露）

- **守住**：把门槛从「路径存在」提高到「**所指文件里真有那段内容**」。"随手改一个 `landing`"
  变成"必须同时伪造一个内容证据"；落地内容被撤销会被抓。
- **守不住**：它验的是"你指的那个文件里有这个串"，**不**验"这个串只应出现在那里"。
  把 `landing` 指到另一个**恰好也含该锚点**的文件，判据仍会绿。
- **也不验**：那段内容在**语义上**是否兑现了那个决定——那部分仍由人读（ADR 0054 决策 2）。

⇒ 这是**签名 + 观测**的分工，不是全知验证。MUST NOT 被读作「已落地已被证明」。

## 本轮合法地改了既有用例的前提

W88 的反向用例「一个**可解析**的 `landing` 出现在**别的**处置上 ⇒ MUST NOT 红」，注入时只给
`landing` 不给锚点 ⇒ 现在会红，而红的原因是**缺内容证据**，不是「放错位置」。
⇒ 该用例补上锚点（`carrier.landing_anchor = ['NativeVault']`）后仍验它原本要验的那一件事。

这与 W92 的「清空一个态暴露既有用例的隐含依赖」**同族**：一次合法的判据增强会让既有用例的
**前提**失效，修法是**补齐前提**，MUST NOT 用 `if (!x) continue` 之类的短路把判据关掉。

## 非空洞注入（全部真跑 + 还原校验）

| # | 注入 | 结果 |
| --- | --- | --- |
| **P1** | 探针①复现：`PD-22.landing` → `README.md` | **红**（`PD-22` + `landing_anchor` + `README.md`） |
| **P2** | 探针②复现：删规格里 `PD-24` 的落地句 | **红**（`PD-24` + `is not present in`） |
| F1 | 锚点改成文件里没有的串 | **红** |
| F2 | `landing_anchor` 少一项（长度不等） | **红**（`correspond one-to-one`） |
| F3 | 有 `landing` 无 `landing_anchor` | **红**（`landing_anchor is missing`） |
| F4 | 锚点给空串 | **红**（`must be non-empty strings`） |
| F5 | 锚点挂到**别的处置**上且不在文件里 | **红**（按字段存在触发） |
| F6 | 反向：合法锚点挂到别的处置上 | **不红** |
| F7 | `LEDGER_FIELDS` 漏登记 `landing_anchor` | **红**（`unknown field landing_anchor`） |
| F8 | 类别并集漏掉 `ANCHOR_FIELDS` | **红**（`allowed by LEDGER_FIELDS but no check category claims it`） |

每次注入后 sha256 逐字还原（**校验 OK**）。`P1`／`P2` 由绿转红是本轮的**成果**。

⚠️ 注入过程中的两处自我纠错（记录以免重犯）：
① F1 的探针值一度选了 `viewer_memory_enabled: true`，而它**真的存在**于 `memory.rs`（W92 核实阶段
就见过 line 1049）⇒ 得到**假阴性**（判据不响，原因是我选错了值，不是判据失灵）；
② F7 的替换只替换了注释行的**前缀**，把后半句留成裸代码 ⇒ 模块加载失败、测试报 `pass=0 fail=1`
——那是**注入自身**的错，不是判据在响。

## 后果

- **正面**：`decided_in_repo` 的「已落地」不再是一条被放过的空声明；两个探针由绿转红。
- **代价**：`landing` 与 `landing_anchor` 是**按位置对应**的两份清单 ⇒ 长度不等会红，
  改动落地点时 MUST 同步改锚点（这是有意的：它正是"落地内容变了"的信号）。
- **测试数**：`tests/registry.test.mjs` **50 → 51**（新增 1 个用例，替换而非堆砌）。
- **`shapeErrors()` 四类 → 五类**：闭世界断言随之为五类，漏登记会在两个方向各报一次。

## 遗留

1. **锚点不验语义兑现**：那段内容是否真的兑现了那个决定，仍由人读（ADR 0054 决策 2）。
2. **锚点不验唯一性**：指到另一个恰好含该锚点的文件仍会绿。要堵这个洞需要"该串只应出现在那里"
   的判据，而它无法在不引入脆性的前提下判定 ⇒ **未做**。
3. **另两态的判据缝本轮未动**：`answered_by_baseline` 的 `basis` 只钉「至少引用一个 `§NNN`」，
   **不**钉「那节真的答出了这个问题」；`needs_precondition` 的 `precondition` 只钉「id 解析得开」，
   **不**钉「前置现在是否已达成」。实测这两态**当前无可订正的数据**（见 §证据 2），
   故本轮只登记，MUST NOT 读成已处理。
4. **A2A 仍是 `capability_pack`**：`delegation_depth` 在 Rust 零命中；本轮是**判据层**的改动，
   MUST NOT 被读作"A2A 已可用"。
5. **W90 §八遗留 1／3／4／5、W91 的跨语言键级消费、W92 的基线收录**均**未动**。
