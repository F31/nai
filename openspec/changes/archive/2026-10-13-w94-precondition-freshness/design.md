# w94 设计：`precondition` 的达成状态

## 一、问题形状

| 态 | 判据钉住的 | **没**钉住的 | 可判吗 |
| --- | --- | --- | --- |
| `decided_in_repo` | `landing` 是已跟踪路径 | 文件里**真的有**承载内容（W93 已补） | 可判（内容子串） |
| `needs_precondition` | `precondition` 的 id **解析得开** | 前置**现在是否仍未达成**（**本轮**） | 可判（registry status） |
| `answered_by_baseline` | `basis` 至少引用一个 `§NNN` | 那节**真的答出了**这个问题 | **不可判**（语义） |

第三行是本轮的边界：`basis` 有没有答出那个问题只能由人读（ADR 0054 决策 2）。第二行则是**可判**的——
registry 里每条验收都带 `status`——所以它不该留在"没钉住"那一列。

## 二、核实（改真实文件跑出来的，不是推断）

1. **缝存在的证据**：`PD-26.precondition` → `["S03"]`（`status: tested`）⇒ **51/51 全绿**。
2. **status 取值域实测**：`implemented` 29／`tested` 22／`pending` 43／`blocked` 6／`not_applicable` 3。
3. **`not_applicable` 归哪边**：`S12` 是 `not_applicable`，而 `notes` 明写「依赖真实平台规则核对」
   ⇒ **仍未闭合**。⇒ 归**未达成**。这是取值域里唯一需要靠内容、不能靠字面判断的一格。
4. **工作项表结构**：`| W61 | MCP 适配：… | H11、H12 | 能力包 |` —— **四列、没有状态列**。
   ⇒ `W\d{2}` 的达成状态只能由第 3 列**明写**的关联验收派生。
5. **派生率实测**：工作项表 36 行里 **11 行（W00–W10）第 3 列是路径／包名**，无验收 id ⇒ 不可判定。
   当前 4 条前置涉及的 `W61` → `H11、H12` ✓ 可判定。
6. **当前数据全部可过**：H05／H06／H07／H08／H13／H14／H16 全 `pending`；W61 的 H11／H12 也 `pending`
   ⇒ 补判据后**零数据改动**、仍全绿。

## 三、决定

### 决定 1：注入**事实**，不注入判定

两个候选形状：

- ❌ `deps.preconditionMet(id) -> boolean`：判定在注入侧。若注入方算错或漏算，判据照样绿——
  **守卫自我满足**（MEMORY §6）。
- ✅ `deps.acceptanceStatus`（id → status）+ `deps.workItemAcceptance`（W → 关联验收 id 数组）：
  注入的是两份**事实**，「达成了没有」由判据自己按 `MET_STATUSES` 算。

选后者，与 W93 的 `readTrackedText`、W88 的 `repositoryFiles` 同一条律。

### 决定 2：`W\d{2}` 经关联验收派生，措辞只说到派生那一层

`W61` 的关联验收是 `H11、H12`，两者**都**达成 ⇒ 报错措辞为
「`W61` 的关联验收 `H11、H12` 均已达成」。**不**写「W61 已完成」——
「关联验收达成」与「工作项做完」不是同一件事，判据只看见了前者。

### 决定 3：取值域闭世界，防「新增一个 status 就静默失效」

```js
ACCEPTANCE_STATUSES = ['pending', 'blocked', 'implemented', 'tested', 'not_applicable']
MET_STATUSES        = ['implemented', 'tested']
UNMET_STATUSES      = ['pending', 'blocked', 'not_applicable']
```

`shapeErrors()` 断言三者关系：**MET ∪ UNMET == ACCEPTANCE_STATUSES** 且 **两两不相交**。
运行时再断言：取到的 `status` MUST ∈ 取值域，否则具名失败。

为什么两道：只写运行时检查，将来有人往 registry 加一个 `deferred` 而没动这两张表，
判据会**静默地不认识它**（安全方向还是危险方向取决于它归哪边，反正没人会知道）⇒ 必须在
`shapeErrors()` 里把「划分完整」钉成一条断言。这与 W92「清空一个态」、W66「删字段照样绿」同族。

### 决定 4：不可解析 ⇒ 具名失败，不静默跳过

三种不可解析，分别具名：
- 工作项第 3 列无验收 id（W00–W10 那 11 行）；
- 关联验收 id 不在 `acceptanceStatus` 里；
- `status` 取值不在取值域内。

一律**报错**而非跳过：「没看到」与「出错了」在**类型**上分开（铁律二）；
「不可解析 ⇒ 失败并分类，不静默降级」（MEMORY §6）。

### 决定 5：触发条件按**字段存在**，不按 `disposition`

与 W80／W83／W88／W93 同一条律。一个已达成的 `precondition` 挂在别的处置上，
一样要报——「谁在管它」是**字段的属性**，不是这条数据当前归哪一类的属性。

### 决定 6：不新增字段类别

`precondition` 已在 `ID_LIST_FIELDS`。达成状态是**同一个字段的第二条断言**（解析得开／是否仍悬空），
不是新字段 ⇒ 不进 `LEDGER_FIELDS`、不进 `REFERENCE_FIELDS`、不进类别划分、不加耦合对。
类别划分判的是「这个字段的**引用**由谁解析」，达成状态不是引用解析 ⇒ 它是循环里第四条独立检查。

## 四、注入清单（F1–F8，全部真跑）

| # | 注入 | 期望 |
| --- | --- | --- |
| F1 | `PD-26.precondition` → `["S03"]`（已达成） | 红（探针 ① 的回归） |
| F2 | `PD-30.precondition` → `["W58"]` 且其关联 H05/H06 全改 `implemented` | 红（W 派生路径） |
| F3 | `PD-23.precondition` → `["H99"]`（形状对、registry 无） | 红（不可解析，具名） |
| F4 | 把 `["S03"]` 挂到 `PD-01`（`needs_external`）上 | 红（触发条件按字段） |
| F5 | `["S12"]`（`not_applicable`） | **不红**（归未达成，实测反例） |
| F6 | 反向：合法未达成 `["H05"]` | 不红 |
| F7 | `status` 取值改成域外的 `"deferred"` | 红（取值域闭世界） |
| F8 | `MET_STATUSES` 漏登记一个取值 | 红（`shapeErrors()` 划分断言） |

每次注入后 sha256 逐字还原校验。

## 五、明确不做

- **不验「前置达成后这条就该裁定」**——可能还有别的未决条件。
- **不把 `basis` 的语义兑现做成判据**——不可机械判定（ADR 0054 决策 2）。
- **不改 registry 的任何 `status`**、不迁任何条目的态。
- **不审计 `needs_external` 21 条**（实测无可订正的数据，见 proposal）。
