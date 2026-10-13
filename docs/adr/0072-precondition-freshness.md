# ADR 0072：`precondition` MUST 仍未达成 —— 「还在等」不能是空声明

- 日期：2026-10-13（W94）
- 状态：已接受
- 相关：ADR 0071（`landing` 内容锚点）、ADR 0054（决策 2：语义留给人读）、ADR 0066（第六态）

## 背景

台账的 `needs_precondition` 断言「这条现在还不能裁定，因为它在等前置」。它的必填字段是
`precondition`（等哪些前置），而判据只钉一件事：**每个 id 解析得开**（是注册表的验收 id，
或是基线工作项表里的工作项）。

W93 堵上了 `decided_in_repo` 的同类缝（「已落地」只验路径）。本轮是**第三种形状**：
六个态里只有 `needs_precondition` 的**有效期会随仓库自身进展而过期**——前置一旦达成，
台账不会有任何信号，条目会一直挂在"等前置"上，而它实际上已经可以裁定了。

## 证据

1. **缝存在**（改真实台账 → 跑 `node --test tests/registry.test.mjs`）：
   把 `PD-26.precondition` 从 `["H05","H06","H16"]` 换成 `["S03"]`——`S03` 在 registry 里是
   `status: tested`，**早已达成** ⇒ **51 / 51 全绿**。
2. **status 取值域实测**（`tests/registry.yaml`）：
   `implemented` 29 / `tested` 22 / `pending` 43 / `blocked` 6 / `not_applicable` 3。
3. **`not_applicable` 不能靠字面归类**：`S12`（虚拟人标识与合规门禁）的 `status` 是
   `not_applicable`，而 `notes` 明写「依赖真实平台规则核对」⇒ **它仍未闭合**。
   ⇒ 归**未达成**。这是取值域里唯一需要靠**内容**、不能靠字面判断的一格。
4. **工作项表没有状态列**：`| W61 | MCP 适配：… | H11、H12 | 能力包 |` —— 四列，无状态列
   ⇒ `W\d{2}` 的达成状态**直接不可判**，只能由第 3 列**明写**的关联验收派生。
5. **派生覆盖率**：工作项表 36 行里 **11 行（W00–W10）第 3 列是路径／包名**，无验收 id ⇒ 不可判定。
6. **当前数据全部可过**：H05／H06／H07／H08／H13／H14／H16 全 `pending`，W61 的 H11／H12 也 `pending`
   ⇒ 本轮**零数据改动**。

## 决定

### 决定 1：`precondition` 的每个元素 MUST 仍未达成；已达成 ⇒ 具名失败

措辞三件齐（条目 + 字段 + 值）：`PD-26: precondition S03 is already tested — an entry that
still waits on a met precondition is a stale claim`。

### 决定 2：注入**事实**，不注入判定

- ❌ `deps.preconditionMet(id) -> boolean`：判定在注入侧。注入方算错或漏算，判据照样绿
  ⇒ **守卫自我满足**。
- ✅ `deps.acceptanceStatus`（验收 id → `status`）+ `deps.workItemAcceptance`（工作项 → 关联验收）：
  注入两份**事实**，「达成了没有」由判据按取值域算。

与 W93 的 `readTrackedText`、W88 的 `repositoryFiles` 同一条律——**同一个陷阱的第三次出现**。

两张表**缺任一项即报错**，形状不对（不是 `Map`）也 MUST **具名报出并立刻返回**：
实测 `acceptanceStatus: {}` 会在判据内部抛 `TypeError: … get is not a function`——
那不是一条错，是判据崩了。

### 决定 3：`W\d{2}` 经关联验收派生，措辞只说到派生那一层

「`W61` 的关联验收 `H11/H12` 均已达成」。**不**写「W61 已完成」——
「关联验收达成」与「工作项做完」不是同一件事，判据只看见了前者。

### 决定 4：取值域闭世界，两道

```js
ACCEPTANCE_STATUSES = ['pending', 'blocked', 'implemented', 'tested', 'not_applicable']
MET_STATUSES        = ['implemented', 'tested']
UNMET_STATUSES      = ['pending', 'blocked', 'not_applicable']
```

- `shapeErrors()` 断言：MET ∪ UNMET **恰等于**取值域、且**两两不相交**（两个方向都具名）。
- 运行时：取到的 `status` 落在域外 ⇒ 具名失败。

为什么两道：只写运行时检查，将来有人在 registry 里加一个 `deferred` 而没回头给两张表归类，
判据会**静默地不认识它**。与 W92「清空一个态」、W66「删字段照样绿」、W93 的耦合表自洽同族：
**一次合法的数据／取值变更，改变了判据的覆盖面**。

### 决定 5：不可解析 ⇒ 具名失败，不静默跳过

三种形状分别具名：工作项第 3 列无验收 id（W00–W10 那 11 行）／关联验收不在 `status` 表里／
`status` 落在域外。一律报错而非跳过：「没看到」与「出错了」在**类型**上分开（铁律二）。

**这条合法地改了一条既有用例的前提**：W88 的反向用例取 `workItemIds` 的第一个元素（实际是 `W00`），
而 `W00` 现在不可判定 ⇒ 该用例改用「有非空关联验收、且那些验收全部仍未达成」的工作项（实测 `W61`）。
不是放宽断言，是把样本换成**本轮口径下真正还悬空**的那一个。

### 决定 6：触发条件按**字段存在**，不按 `disposition`

与 W80／W83／W88／W93 同一条律。已达成的前置挂到别的处置上，一样要报
（实测 `PD-01`（`needs_external`）加 `precondition: ["S03"]` ⇒ 红）。

### 决定 7：不新增字段类别

`precondition` 已在 `ID_LIST_FIELDS`。达成状态是**同一字段的第二条断言**，不是新字段
⇒ 不进 `LEDGER_FIELDS`、不进 `REFERENCE_FIELDS`、不进类别划分、不加耦合对。
类别划分判的是「这个字段的**引用**由谁解析」，达成状态不是引用解析 ⇒ 循环内第四条独立检查。

「解析得开」只答「这个 id 存在」，**不**答「它现在还悬空着」——两条断言 MUST NOT 互相冒充。

## 验证（全部真跑）

| # | 注入 | 结果 |
| --- | --- | --- |
| P1 | `PD-26.precondition` → `["S03"]`（已达成） | **由绿转红**（`fail=5`，具名命中） |
| F1 | `PD-27.precondition` → `["S04"]` | 红 |
| F2 | `MET_STATUSES` 含 `pending` ⇒ `PD-30` 的 `W61` 经 `H11/H12` 派生 | 红 |
| F3 | `PD-26.precondition` → `["W00"]`（无关联验收） | 红（不可判定） |
| F4 | `PD-01`（`needs_external`）加 `precondition: ["S03"]` | 红（按字段触发） |
| F5 | `["S12"]`（`not_applicable`） | **不红**（实测反例） |
| F6 | `["H05"]`（`pending`） | 不红 |
| F7 | 取值域漏掉 `tested` 而 MET 仍认领它 | 红（方向②：分类认领域外取值） |
| F8 | `MET_STATUSES` 漏登记 `tested` | 红（方向①：域里没人认领） |

每次注入后 sha256 逐字还原校验，**ALL-OK**。

**一处自我纠错**：F7 第一次选的值不对——把 `tested` 从取值域里拿掉后它仍在 `MET_STATUSES` 里，
运行时不会判成域外，报红的是 `shapeErrors` 的**另一个方向**。修正后 F7 改验方向②，
**运行时**遇域外 `status` 的具名失败由测试 ⑤ 覆盖（`domainBreach.set('H05','deferred')`）。

## 如实披露

判据验的是「这个前置现在仍未达成」，它**不**验：

- 「这条除了这些前置之外没有别的未决条件」；
- 「前置达成后这条就一定可以裁定」；
- 「工作项已完成」（只见其关联验收）。

⇒ MUST NOT 被读作"该裁定什么已被判据决定"。

## 后果

- `scripts/contracts/pending-decisions.mjs`：三个 status 常量、`baselineWorkItemAcceptance()`、
  两个必填依赖（含形状检查）、`shapeErrors()` 的取值域闭世界、循环内前置达成检查。
- `tests/registry.test.mjs`：注入两张表、缺输入清单加两项、新增 1 个用例（6 个子断言）、
  修 W88 反向用例的前提、新增「不可判定 MUST NOT 放行」断言。
- **`tests/pending-decisions.yaml` 与 `tests/registry.yaml` 零改动**：纯判据改动。
- 测试数 **51 → 52**。

## 遗留

1. **不验「前置达成后这条就该裁定」**——可能还有别的未决条件，判据不替人做这个判断。
2. **`answered_by_baseline` 的缝仍未堵，且不可堵**：`basis` 只钉「至少引用一个 `§NNN`」，
   **不**钉「那节真的答出了这个问题」——后者是语义，不可机械判定（ADR 0054 决策 2）。
   实测该态 4 条**都真的答了**，故无数据订正可做，只登记。
3. **`needs_external` 21 条未审计**：本轮逐条抽样了 `closure`，绝大多数确实闭合在仓外
   （平台书面答复／商用授权／第三方规范／真机实测／市场项目约定）⇒ 未见登记不实。
4. **台账 ↔ 基线的逐字对账不可行**：34 条 `question` 里 11 条在基线里找不到逐字片段
   （`question` 是复述而非引用）⇒ 逐字口径会造 11 条假阳性。换口径等于引入模糊匹配，更脆。
5. **W90 §八遗留 1／3／4／5、W91 的跨语言键级消费、W92 的基线收录、W93 的锚点唯一性**均**未动**。
