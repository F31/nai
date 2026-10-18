# W94：`needs_precondition` 的「还在等」MUST 是真的 —— 前置补上达成状态

- 日期：2026-10-13
- 变更：`openspec/changes/w94-precondition-freshness/`
- ADR：[0072](../adr/0072-precondition-freshness.md)
- 起始：`HEAD == origin/main == 21ee78c`，工作区干净，无活动变更，归档 91 个

## 一、母题

台账的 `needs_precondition` 断言「这条现在还不能裁定，因为它在等前置」。它的必填字段是
`precondition`（等哪些前置），而判据只钉一件事：**每个 id 解析得开**（是注册表的验收 id，
或是基线工作项表里的工作项）。

本轮是同一族缝的**第三种形状**：W93 堵的是「已落地」（`landing` 只验路径），本轮堵的是
「还在等」（`precondition` 只验可解析）。**六个态里只有这一个态的有效期会随仓库自身进展而过期**
——前置一旦达成，台账不会有任何信号，条目会一直挂在"等前置"上，而它实际上已经可以裁定了。

## 二、核实（先测后设计）

| 项 | 实测结果 |
| --- | --- |
| 缝是否存在 | `PD-26.precondition` → `["S03"]`（`status: tested`）⇒ **51/51 全绿** |
| status 取值域 | `implemented` 29 / `tested` 22 / `pending` 43 / `blocked` 6 / `not_applicable` 3 |
| `not_applicable` 的归属 | `S12` 是 `not_applicable`，而 `notes` 明写「依赖真实平台规则核对」⇒ **仍未闭合** |
| 工作项表结构 | `\| W61 \| … \| H11、H12 \| 能力包 \|` —— **四列，无状态列** |
| 派生覆盖率 | 36 行里 **11 行（W00–W10）第 3 列是路径／包名**，无验收 id ⇒ 不可判定 |
| 当前数据 | H05/06/07/08/13/14/16 全 `pending`，W61 的 H11/H12 也 `pending` ⇒ **零数据改动** |

## 三、另两个候选为何没选（实测，不是推测）

| 候选 | 实测 | 结论 |
| --- | --- | --- |
| 审计 `needs_external` 21 条 | 逐条抽样 `closure`：平台书面答复／商用授权／第三方规范／真机实测／市场项目约定 | 绝大多数**确实闭合在仓外**，未见登记不实 |
| 台账 ↔ 基线逐字对账 | 34 条 `question` 里 **11 条在基线里找不到逐字片段** | **过脆**——`question` 是复述而非引用，逐字口径会造 11 条**假阳性** |

## 四、设计与四处取舍

### 4.1 注入**事实**，不注入判定

- ❌ `deps.preconditionMet(id) -> boolean`：判定在注入侧 ⇒ 注入方算错时判据照样绿，**守卫自我满足**。
- ✅ `deps.acceptanceStatus`（验收 id → status）+ `deps.workItemAcceptance`（工作项 → 关联验收）。

与 W88 的 `repositoryFiles`、W93 的 `readTrackedText` 同一条律——**同一个陷阱的第三次出现**。

两张表**缺任一项即报错**；形状不对（不是 `Map`）也 MUST **具名报出并立刻返回**——实测
`acceptanceStatus: {}` 会在判据内部抛 `TypeError: … get is not a function`，那不是一条错，是判据崩了。

### 4.2 `W\d{2}` 经关联验收派生，措辞只说到派生那一层

基线工作项表**没有状态列** ⇒ `W61` 的达成状态只能由它第 3 列**明写**的 `H11、H12` 派生。
措辞是「`W61` 的关联验收 `H11/H12` 均已达成」，**不**写「W61 已完成」——
「关联验收达成」与「工作项做完」不是同一件事，判据只看见了前者。

### 4.3 取值域闭世界，两道

```js
ACCEPTANCE_STATUSES = ['pending', 'blocked', 'implemented', 'tested', 'not_applicable']
MET_STATUSES        = ['implemented', 'tested']
UNMET_STATUSES      = ['pending', 'blocked', 'not_applicable']
```

`shapeErrors()` 断言「MET ∪ UNMET **恰等于**取值域、且**两两不相交**」，运行时再断言「域外取值 ⇒
具名失败」。只写运行时检查是不够的：将来有人在 registry 里加一个 `deferred` 而没回头给两张表归类，
判据会**静默地不认识它**。与 W92「清空一个态」、W66「删字段照样绿」同族——
**一次合法的数据／取值变更，改变了判据的覆盖面**。

### 4.4 不可解析 ⇒ 具名失败，不静默跳过

三种形状分别具名：工作项第 3 列无验收 id／关联验收不在 `status` 表里／`status` 落在域外。
一律报错而非跳过（铁律二：「没看到」与「出错了」在**类型**上分开）。

### 4.5 不新增字段类别

`precondition` 已在 `ID_LIST_FIELDS`。达成状态是**同一字段的第二条断言**，不是新字段
⇒ 不进 `LEDGER_FIELDS`、不进 `REFERENCE_FIELDS`、不进类别划分、不加耦合对。
「解析得开」只答「这个 id 存在」，**不**答「它现在还悬空着」——两条断言 MUST NOT 互相冒充。

## 五、本轮合法地改了既有用例的前提（第二次出现）

W88 的反向用例「可解析 `precondition` 挂别的处置上 ⇒ 不红」取的是 `workItemIds` 的第一个元素
（实际是 **`W00`**），而 `W00` 在本轮口径下**不可判定**（第 3 列是路径／包名）
⇒ 该用例改用「有非空关联验收、且那些验收全部仍未达成」的工作项（实测 `W61`）。
**不是放宽断言，是把样本换成「本轮口径下真正还悬空」的那一个**——与 W93 修 W88 反向用例同形
（修法是**补齐前提**，MUST NOT 用短路把判据关掉）。

## 六、门控与计数

### 6.1 变更校验

| 项 | 结果 |
| --- | --- |
| `openspec validate w94-precondition-freshness --strict` | **valid**（EXIT=0） |
| 规格差量 | 1 ADDED（417 字符）+ 1 MODIFIED（353 字符），均 ≤ 500 |

### 6.2 全量门控

`cargo xtask check --profile full` ⇒ **EXIT=0**（完整日志统计，非抽样）：

| 项 | 数字 |
| --- | --- |
| Rust 测试 | **91 个测试目标 / 862 passed / 0 failed** |
| 预链接 | **LNK1104 = 0** ⇒ 第 1 轮即收敛，无重试 |
| `cargo fmt --all --check` | **EXIT=0**（独立复跑） |
| Node 五套件 | **52 / 30 / 22 / 5 / 52**（`registry.test.mjs` **51 → 52**，新增 1 个用例） |

**Rust 零改动、`xtask` 零改动**——本轮只改 `.mjs` 与文档。

### 6.3 两条 validate

| 时点 | 命令 | 结果 |
| --- | --- | --- |
| 归档前 | `openspec validate --all --strict` | **44 passed / 0 failed**（含活动变更） |
| 归档后 | `openspec validate --all --strict` | **43 passed / 0 failed** |
| 归档后 | `openspec validate --all --strict --archived` | **92 passed / 0 failed**（W93 是 **91** ⇒ **91 → 92**） |

归档 `w94-precondition-freshness`：`spec-driven-delivery` **+ 1 added, ~ 1 modified**
（Totals `+ 1, ~ 1, - 0, → 0`），Task status **✓ Complete**（归档前已全勾）。

**单次观测**（三件齐：唯一查询参数 `cb=<ns>` + `Cache-Control: no-cache` + 回读 `head_sha`）：

| 项 | 值 |
| --- | --- |
| run | **38416987565（#119）** |
| 回读 `head_sha` | `e3e4ee57739fdd164df1e7bbf128802cecb8003a` —— **== 推送 SHA（MATCH）** |
| 结论 | **completed / success**（`14:51:32Z → 15:01:55Z`，约 10 分 23 秒） |
| 两个 job | `foundation (windows-latest)` / `foundation (ubuntu-24.04)`，各 26 步，**全 success** |

- Ubuntu 仅 **step 11**（`if: failure()` 的 annotation 通道）跳过；Windows 另跳过 **step 8**
  （Linux C++ protobuf runtime）与 **step 20**（Linux-only 三语言套件）——**平台分工，非缺失**。
- **本机绿与 CI 绿同源**：CI **step 10** 跑的正是 `cargo xtask check --profile full`，与 §6.2 本机门控
  **同一条命令、同一个 profile**，两平台都 `success` ⇒ 本轮判据变更在 CI 侧**真跑过**。
  另：CI **step 21／22** 分别跑 `pnpm spec:validate` 与 `openspec validate --archived`，与 §6.3 同族。
- **自限声明**：观测为**作业／步骤级**（`/actions/jobs/{id}/logs` 需 admin，不可读）
  ⇒ 本轮**不断言**「CI 侧也是 91/862／52」，只断言上面这条。
- **顺带观测**：run `38409489827`（#118）= W93 回填提交 `21ee78c` = **success**
  （上一轮按约定不观测，此处顺带读到）。
- **代理端口从 `https_proxy` 读**（本轮 `61209`），未硬编码——W93 曾因写死上一轮端口而取空响应。

## 八、遗留

1. **不验「前置达成后这条就该裁定」**——可能还有别的未决条件，判据不替人做这个判断。
2. **`answered_by_baseline` 的缝仍未堵，且不可堵**：`basis` 不钉「那节真的答出了这个问题」
   ——那是语义，不可机械判定（ADR 0054 决策 2）。实测该态 4 条**都真的答了**，只登记。
3. **`needs_external` 21 条未审计**：本轮抽样了 `closure`，未见登记不实。
4. **台账 ↔ 基线的逐字对账不可行**（11/34 假阳性）。
5. **W90 §八遗留 1／3／4／5、W91 跨语言键级消费、W92 基线收录、W93 锚点唯一性**均**未动**。

## 九、如实披露

判据验的是「这个前置现在仍未达成」。它**不**验「这条除了这些前置之外没有别的未决条件」、
**不**验「前置达成后这条就一定可以裁定」、**不**验「工作项已完成」（只见其关联验收）。
⇒ MUST NOT 被读作"该裁定什么已被判据决定"。
