# w88：待裁定台账的「已决定」与落地点

## Why

W76 的台账（`tests/pending-decisions.yaml`）把 34 条待裁定项分成**五态**，五态回答的都是
**「何处能闭合」**：基线答了（`answered_by_baseline`）／可在仓库内决定（`decidable_in_repo`）／
等前置（`needs_precondition`）／等外部（`needs_external`）／不在范围（`out_of_scope`）。

W87 §八第 1 条登记：「**W76 台账的 5 条 `decidable_in_repo` 仍只登记了推荐**」。本轮采纳
`PD-09`（*静态数据加密所用的系统密钥保护机制*），核实结论是：

- **它的推荐早已在仓库落地**——`crates/nai-security/Cargo.toml` 把 `keyring` 只放在
  `[target.'cfg(windows)'.dependencies]` 下；`NativeVault` 在 Windows 走系统凭据库、非 Windows
  一律 `CapabilityNotReady`（失败关闭，不是静默降级，有测试钉住）；`platforms.json` 的
  `secret-protection` 能力行逐平台登记（`linux`/`macos` = `explicit-unavailable`）。
  推荐的四条子句（按平台用系统密钥库／Windows 原生凭据首发／缺后端的平台显式登记／
  MUST NOT 自造跨平台密钥库）**逐条成立**。
- 但台账**没有可以结清它的状态**：它只能永远挂着一条 `decidable_in_repo` 的「推荐」。

⇒ 缺口不在数据，在**分类**：五态表达不了「**这条已经闭合了、并且落在仓库的哪里**」。
把「已落地」写成「可在仓库内决定」的注解，正是 W87 §九那条「一个载体承载两件事」的同族缺陷。
本轮补第六态 `decided_in_repo`，并配一个**可判**的必填字段 **落地点**（`landing`）：
每个元素 MUST 是仓库**已跟踪**的路径。

## What Changes

- `scripts/contracts/pending-decisions.mjs`：
  - `DISPOSITIONS` 加第六态 `decided_in_repo`；`REQUIRED_FIELD` 加
    `['decided_in_repo', 'landing']`；
  - 新增第三类检查类别 **`REPO_PATH_FIELDS = ['landing']`**（与 `CITATION_FIELDS`／
    `ID_LIST_FIELDS`／`STRUCTURAL_FIELDS` 并列，四类**两两不相交**且并集恰等于 `LEDGER_FIELDS`）；
  - `REFERENCE_FIELDS`／`LEDGER_FIELDS` 逐字加入 `landing`（保持「单一来源 + 双向相等」）；
  - `shapeErrors()` 把类别数从三扩到四，并把「必填字段必须被某检查覆盖」「引用域 == 类别之并」
    两条断言扩到新类别；
  - `validatePendingDecisions` 新增必填依赖 **`repositoryFiles`**（仓库成员路径集），
    并对 `landing` 逐元素按**仓库成员**解析；
  - 必填字段的「清单型／文本型」判定改为**由类别表决定**（`ID_LIST_FIELDS ∪ REPO_PATH_FIELDS`），
    MUST NOT 写死字段名——原实现写着 `field === 'precondition'`，实测会把 `landing` 当文本判
    （探针：改前给一个**合法路径的字符串** ⇒ 0 error；改后 ⇒ 具名失败）。
  - `DISPOSITION_COUNTS`：`decidable_in_repo` **5 → 4**、新增 `decided_in_repo` **1**（总数仍 34）。
- `tests/registry.test.mjs`：注入 `repositoryFiles`；把第四类纳入「两两不相交 / 并集相等 /
  引用域双向相等 / 必填字段被覆盖」四条既有断言；新增三条判据（落地路径存在性、
  路径解析**不按处置触发**、清单型判定由类别表决定），含**正向与反向**注入。
- `tests/pending-decisions.yaml`：`PD-09` 由 `decidable_in_repo` 迁到 `decided_in_repo`，
  必填 `landing` 指向三处真实落地点；`recommendation` 原文保留（它是**被采纳的那份建议**，
  删掉就丢了「决定了什么」）。

不改基线正文、不改 `tests/registry.yaml`、不改任何 Rust、不新增依赖边、不新增契约名。

## Impact

- capability 差量：`spec-driven-delivery`（两条 MODIFIED Requirement + 一条 ADDED Requirement
  —— 该能力本就承载待裁定台账的处置与引用完整性）。
- 新增 ADR 0066；ADR 0054（台账五态的原始设计）**追加勘误段**（历史记录不就地改写）。
- 新增活文档小节（`AGENTS.md`、`docs/development-plan.md`）。
- 基线保持全绿。
