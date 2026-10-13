# ADR 0066：待裁定台账的第六态与落地点（PD-09 结清）

- 状态：接受
- 日期：2026-10-13
- 相关：[ADR 0054](0054-pending-decisions-ledger.md)（台账五态的原始设计；本 ADR 替代其决策 1 中的
  「**五**态」这一计数，并为其遗留 3 与遗留 5 的**第一次判决**给出结论）、
  [ADR 0053](0053-repository-scoped-evidence.md)、[ADR 0055](0055-bringup-budget-single-home.md)、
  [ADR 0057](0057-section-boundary.md)、[ADR 0058](0058-ledger-citation-coverage.md)、
  [ADR 0059](0059-ledger-field-closure.md)、[ADR 0064](0064-openapi-document-set-closure.md)、
  `tests/pending-decisions.yaml`、`scripts/contracts/pending-decisions.mjs`、
  `scripts/contracts/git-index.mjs`、`tests/registry.test.mjs`、
  `docs/reports/2026-10-13-w88-decided-in-repo-landing.md`、
  `openspec/changes/archive/2026-10-13-w88-decided-in-repo-landing/`

## 背景

[ADR 0054](0054-pending-decisions-ledger.md)（**W76**）把基线四章「待决策与待确认」的 34 条去重条目
登记进 `tests/pending-decisions.yaml`，并给每条一个**封闭枚举的处置**与**一个必填字段**。
它的遗留 3 写着：

> **5 条 `decidable_in_repo` 的推荐尚未执行**：它们是推荐而非决定；采纳需要单独一轮
> （其中 PD-09 会触碰平台密钥后端、PD-22/PD-24 属 A2A 边界、PD-31 属 R3 前置、PD-34 需真实访谈）。

W87 §八第 1 条把这 5 条**原样复述**并写明「每条都值得单独一轮」。本轮（**W88**）按
`AskUserQuestion` 的裁定采纳 **PD-09**（*静态数据加密所用的系统密钥保护机制*）。

## 证据（本轮实测，先读后判）

### 1. PD-09 的推荐

> 按目标平台各用**系统密钥库**（Windows 原生凭据为首发实现），缺该后端的平台如实登记
> explicit-unavailable（沿用 W55 的平台维度处置），MUST NOT 自造一套跨平台密钥库

### 2. 四条子句逐条在仓库中的落地

| 子句 | 仓库事实 | 落地点 |
| --- | --- | --- |
| 按目标平台各用系统密钥库 | `NativeVault`（`keyring`） | `crates/nai-security/src/lib.rs` |
| Windows 原生凭据为首发实现 | `keyring` **只在** `[target.'cfg(windows)'.dependencies]` 下 | `crates/nai-security/Cargo.toml` |
| 缺后端的平台如实登记 `explicit-unavailable` | `secret-protection` 能力行：`windows` = supported；`linux`/`macos` = `explicit-unavailable`（`not_ported_to_target_os`）；`ios`/`android` = `out-of-scope` | `schemas/platform-matrix/platforms.json` |
| MUST NOT 自造跨平台密钥库 | 不存在这样的东西（无落地点即为该子句的证据） | —（同一能力行的 note 明写「非 Windows 的 store/load/delete 一律**显式**返回 CapabilityNotReady——是失败关闭，不是静默降级」） |

另有测试钉住该行为：`production_backend_is_explicitly_unavailable`（非 Windows）与
`native_windows_roundtrip_and_cleanup`（Windows），后者是 `#[cfg(windows)]` ⇒ 由 CI 的 windows job 执行。

⇒ **推荐已被采纳并落地**（不是「计划落地」）。

### 3. 基线**没有**回答这个问题

基线 §11.5 的原文是：「备份与录制提供可选加密，密钥由系统密钥保护机制封装（**具体机制待选型验证**）」。
⇒ 把它记作 `answered_by_baseline` 会是**一条不实声明**。这条排除是**判据级**的，不是措辞级的。

### 4. 五态表达不了「已决定」

五态回答的都是「**何处能闭合**」：基线答了／可在仓库内决定／等前置／等外部／不在范围。
**没有一态**表示「**仓库已经作出决定、并且已经落地**」。⇒ 一个已结清的条目只能永远停在
`decidable_in_repo`，而那个状态的名字叫「**可**决定」。

### 5. 落地点不能用作业项编号

实测基线的工作项表**不含 W41–W55**（§47.6 的编号说明：V3.7 计划止于 W40，W41–W44／W46–W55 由后续
实际执行轮次占用，第四部分从 W56 起编号）。⇒ PD-09 的落地轮次 **W55 无法作为 `W\d{2}` 解析**；
把落地点写成作业项，这条必填字段就会**恒假**。

## 决策

### 决策 1：新增第六态 `decided_in_repo`，它**不是**任何既有处置的注解

处置枚举由五态扩为六态。`decided_in_repo` 的语义是「**决定已在仓库内作出且已落地**」。
**MUST NOT** 把已结清的条目写回 `decidable_in_repo`（那会让台账说「还等人决定」）——
这与 W87 §九「一个载体承载两件事」是同一条纪律的**正向**适用。

### 决策 2：它的必填字段是 `landing`（落地点），**不是**决定文本

一个必填字段，六态对称：`answered_by_baseline`→`basis`（答在哪一节）、`decidable_in_repo`→
`recommendation`（建议怎么做）、`decided_in_repo`→`landing`（**落在仓库的哪里**）、
`needs_precondition`→`precondition`、`needs_external`→`closure`、`out_of_scope`→`note`。
台账钉的是「**管到哪**」这个可判事实；决定的内容**刻意留给人读**（ADR 0054 决策 2 的边界继续有效），
它的家是 ADR 与报告。`PD-09` 的 `recommendation` **原文保留**为注解（它就是被采纳的那份建议）。

### 决策 3：`landing` 的每个元素 MUST 是**仓库已跟踪**的路径

判据的输入由 `.git/index` 派生（`scripts/contracts/git-index.mjs`，[W75](0053-repository-scoped-evidence.md)）。
**MUST NOT** 遍历工作区——那是 W75 的 F2 根因（未跟踪文件会被算成仓库成员）。
「已声明 ≠ 存在」这条纪律因此对落地点同样成立。`validatePendingDecisions` 新增必填依赖
`repositoryFiles`；缺它即具名报错（MUST NOT 降级为「跳过该检查」）。

### 决策 4：新增第三类检查类别 `REPO_PATH_FIELDS`，与 `ID_LIST_FIELDS` **不合并**

两者虽同为「必须解析得开」，但**解析器不同**（登记 id vs 仓库成员路径）⇒ 归两类
（W66「不合并不同维度」）。四类 MUST 两两不相交、且并集恰等于 `LEDGER_FIELDS`；
`shapeErrors()` 的三条断言（不相交 / 并集相等 / 引用域双向相等）随之从三扩到四。

### 决策 5：「清单型 / 文本型」由类别表决定，MUST NOT 写死字段名

原实现把「清单型 / 文本型」写成 `field === 'precondition'`（即由**写死的字段名**决定）。
本轮**实测改前那一行**：

| 注入（`PD-09.landing` 给成**字符串**，值是一个**真实存在的跟踪路径**） | 结果 |
| --- | --- |
| 改前（`field === 'precondition'`） | **0 error** —— 当文本判，「非空」即过关 |
| 改后（`idListFields ∪ repoPathFields`） | **1 error**：`PD-09: disposition decided_in_repo requires a non-empty landing` |

⇒ 写死的字段名在**新增一个清单字段的那一刻就说谎**（W83 同族：清单的补集是盲区）。

### 决策 6：路径校验按**字段是否存在**触发，MUST NOT 按处置触发

W80（`§NNN` 侧）与 W83（id 侧）已确立这条纪律；`landing` **出生即遵此律**。
可解析的落地点出现在非专属处置上 ⇒ MUST NOT 红（假阳性方向）。

### 决策 7：**不**引入 `tests/pending-decisions.schema.json`

ADR 0054 的遗留 5 预留了这次判定：「是否需要，等它出现**第二次结构变更**再判」。
本轮**正是**那次结构变更（6 态 + 1 字段 + 1 类别 + 1 依赖）。判定为**不引入**：

- 校验器已经用**闭世界**（条目键集、信封键集）+ 封闭枚举 + 类别划分把形状钉死，且**接上了电**
  （`cargo xtask check --profile full` 里跑）；
- 再写一份 JSON Schema 会在**同一件事上出现第二处定义**（[W86](0064-openapi-document-set-closure.md)
  的教训：两处定义会在某一次合法编辑后**静默分家**，而接上电的那一份才是判据）。
  ⇒ 结论是「**不引入**」，而不是「暂缓」。

## 后果与实施

- `PD-09` 由 `decidable_in_repo` 迁到 `decided_in_repo`；`DISPOSITION_COUNTS`
  `decidable_in_repo` **5 → 4**、`decided_in_repo` **0 → 1**（总数仍 **34**、分章计数不变）。
- 判据落点：`scripts/contracts/pending-decisions.mjs`（第 6 态、第 4 类别、`repositoryFiles` 依赖、
  清单型判定）、`tests/registry.test.mjs`（**47 → 50**）。
- 非空洞注入实测：F0 干净台账零错误；F1 不存在的路径具名；F2 磁盘存在但未跟踪的路径具名；
  F3 把 `landing` 移出 `REFERENCE_FIELDS` ⇒ 同一个坏值**静默**（证明该检查承重）；
  F4/F5 为决策 5 的改前/改后对照；F6 反向注入不红；F7 跟踪集 1014 条。
- 不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边、不新增契约名。

## 遗留

1. **余下 4 条 `decidable_in_repo` 的推荐仍只登记**：`PD-22`／`PD-24`（A2A 边界）、
   `PD-31`（R3 前置）、`PD-34`（需真实访谈）；每条仍值得单独一轮。核实阶段另有一处观察记入
   W88 报告 §八（`PD-31` 的措辞与 W57 已交付的 `evaluate/memory.rs` 存在张力，需下一轮核实后裁定）。
2. **「已落地」的语义判定仍有半边靠人**：脚本证「这些路径真实存在于仓库」，
   **不证**「它们真的就是这个决定的落地」。与 ADR 0054 遗留 2 是同一条刻意留给人读的边界。
   `landing` 只写**三处**落地点，不穷举（代码里的相关测试与 CI 步骤未列）。
3. **`PD-24` 的 `recommendation` 里引用 `§40.4`** 等既有引用不受本轮影响；台账基线仍为 **V3.11**。
4. **本次不重排 W 编号**：本版为 **W88**（V3.11 计划止于 W87）。
