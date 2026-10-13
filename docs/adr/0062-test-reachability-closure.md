# ADR 0062：测试文件的可达性闭世界——在册测试文件 MUST 被某个登记站点覆盖

- 状态：已接受
- 日期：2026-10-13
- 依据：ADR 0058/0059/0060/0061（同一族：「按清单做」的盲区是清单的补集）、
  W75（仓库文件集由 `.git/index` 纯 JS 取得；**「已声明」≠「已执行」**）、
  W73（假契约：每句「SHALL 被校验」都要能指出执行体）、
  W80（覆盖集 MUST 由清单**派生**，MUST NOT 手写第二份）、
  W82（闭世界 MUST 在**每个层级**上成立）、W41（前端门控；glob 未匹配到文件即失败）
- 影响：新增 `scripts/contracts/test-reachability.mjs`、`tests/registry.test.mjs`；
  **不改基线正文、不改 `tests/registry.yaml`、不改 `tests/pending-decisions.yaml`、不改任何 Rust、不新增依赖边**

## 背景

Node 测试是靠**逐条列举**接上电的：`xtask/src/lib.rs` 里手写
`schema::node(root, &["--test", "tests/contracts/schema.test.mjs"])`，`xtask/src/registry.rs` 里手写
`node(root, &["--test", "tests/registry.test.mjs"])`，再加上两个 glob
（`package.json` 的 `tests/ui/*.test.mjs`、`apps/desktop/package.json` 的 `tests/*.test.mjs`）。

**列举的盲区恰好是它的补集**：往仓库里放一个 `tests/kernel/foo.test.mjs`，没有任何地方会注意到它。

**实测（W84 探针，非推断）**：把 `tests/kernel/w84-probe.test.mjs`（内容只有一句
`assert.fail('this file never ran')`）加入 `.git/index` 后跑 `cargo xtask check --profile portable`：

```
EXIT=0
test result 行数 = 91   （73 Running 目标 + 18 Doc-tests）
passed = 860  failed = 0
grep -c "w84 probe" .log = 0     ← 探针的用例名在日志里**零出现**
```

⇒ 这不是「未登记 = 不跑」，而是**静默的不跑**：连「我忽略了一个文件」都没有任何信号。
一个含**必定失败**断言的测试文件能被完全忽略，这比「跑了但没覆盖」更坏——它让「仓库里所有
测试都在跑」这句话整体不可信。

同族登记（W80–W83 修的是台账的三层盲区，本条修的是**门控接线**的同一形状）：

| 轮次 | 「清单」 | 「清单的补集」 |
|---|---|---|
| W80 | 三个 `sectionRefs(...)` 手写字段 | `recommendation`/`closure`/`note`（17 条引用漏扫） |
| W81 | `CITATION_FIELDS` | 清单**之外**的新字段（`rationale: "§99.9"` 静默） |
| W82 | 条目的键集 | 信**封**（顶层）的键集 |
| W83 | `§` 一侧的触发规则 | id 一侧仍是**按处置**触发 |
| **W84** | **逐条列举 + 两个 glob** | **没有任何站点指向的测试文件** |

## 决策

1. **新增并导出 `TEST_REGISTRATION_SITES`**（七个站点，逐字写死 `{id, host, marker, base, tier}`）
   ——「测试在哪里被接上电」的**单一来源**。

2. **覆盖集由 marker 原文在「已跟踪路径集」上解析得出**（`resolveSite`），MUST NOT 另行声明一份
   「覆盖了哪些文件」的表。第二份表会让「表 == 它自己」成为**恒真式**（W80 决策 5）。
   用**已跟踪路径**而非 `readdir`：覆盖集按**仓库成员**计算（W75），工作区里的散落文件不参与——
   未跟踪的探针文件因此**必然**触发闭世界（本轮的证据正由此得到）。

3. **假契约：「已声明」≠「已执行」**（W75）。站点声明的 `marker` MUST 在其 `host` 里**逐字存在**；
   且站点 MUST 至少覆盖一个**在册**文件（**非空洞**）。一条名字在表里、线上什么都没有的站点必须红。
   「非空洞」这一条**不是**「站点数 > 0」——后者对一个空 glob 是瞎的；`frontend-gate.mjs` 已在
   运行时对 glob 做同样的事，本决策把它**移进**判据。

4. **闭世界 MUST 在「扩展名层」也成立**（W82 纪律的又一次应用）。只认 `.test.mjs` 而不管
   「仓库里有没有别种测试扩展名」，等于把盲区搬到**更少人看**的一层：`foo.test.ts` 既不在覆盖集里、
   也不在台账里——因为「不是被识别的测试文件」意味着**连「未被覆盖」都不会被报出**。
   ⇒ 先判 `looksLikeTest`（宽：含 `.test.`／`.spec.`），再判扩展名 ∈ `TEST_EXTENSIONS`（窄：当前只有
   `.test.mjs`）；宽出来的那块被**具名**点名。**清单的窄，正是它闭合的原因**（见 T2 注入）。

5. **未覆盖台账不是豁免**。`LOCALLY_UNREACHABLE` 的每条 MUST 带非空 `reason`、MUST 是**在册测试文件**、
   MUST **确实未被覆盖**——「确实未被覆盖」让它**自清理**：某天某站点开始覆盖它，那条过期豁免立刻红。
   否则台账会变成「加进去就再也不用管」的垃圾桶。

6. **「覆盖集非空」与「两集不相交」必须成对**。台账当前为空 ⇒「覆盖 ∩ 台账 == ∅」退化成恒真；
   若站点表被清空或全部解析失败，文件层闭世界会因台账为空而**全部静默通过**（「全不覆盖」也能绿）。
   ⇒ 必须补一条 `covered.size > 0`。这是「**非空是互斥类断言的必要一半**」在本轮的具体形状。

7. **入参显式注入，缺任一项即报错**（与 `validatePendingDecisions` 同纪律）：判据 MUST NOT 因
   「没有输入」而静默降级。

## 后果

- **正面**：新增测试文件不再可能被静默忽略——它会红，并被**指名**。豁免存在，但必须**具名、相称、
  自清理**，且过期即红。
- **负面**：站点表成为需要维护的一份声明。这是**有意**的：它使「覆盖集」非恒真，代价是新增接线点
  时要去表里登记一处——而那正是**应该**发生的事（W81 的「改一处即可」是**承诺**不是机制，
  这里把承诺变成会被检查的数据）。
- **局限**：判据只覆盖 **Node** 测试文件与**站点 marker 的逐字存在**。它不判断「host 里的 marker
  是否**真的会执行**」（例如 marker 落在一段死代码里）——那是「文本包含」能到达的边界，
  由独立于本条的运行时门控（`frontend-gate.mjs` 的 glob 非空失败、`cargo xtask check` 的实际退出码）承载。

## 实施

- `scripts/contracts/test-reachability.mjs`：`TEST_EXTENSIONS`／`TEST_TIERS`／`TEST_REGISTRATION_SITES`／
  `LOCALLY_UNREACHABLE`／`isTestFile`／`markerMatcher`／`resolveSite`／`siteShapeErrors`／
  `extensionErrors`／`reachabilityErrors`。
- `tests/registry.test.mjs` **34 → 47**（随 `engineering_checks` 既有条目自动进 `portable`／`full`）。
- 注入：**F0**（真实仓库绿）+ **F1–F9**（数据级，含 F7／F8 两条反向）× 合成世界；
  **T0–T5**（表级，临时模块副本，含 T2「清单放宽 ⇒ 盲区重开」与 T4「还原后仍绿」）⇒ **15/15**。

## 遗留

1. **§56.2 归档边界的正式裁定**仍未落地（已被 W78 起连续六轮登记）：活文档就地订正 / 历史记录追加
   勘误段的**分界**已按惯例执行，但缺一次**升基线轮次**把它写死在基线正文里。
2. **判据只到「文本包含」**：站点 marker 存在于 host 文本中，不等于它会在一条**可达的执行路径**上
   被求值。若将来有站点把 marker 写进注释或死分支，本条不会红。需要「接线被真的走到」这一层时，
   应在运行时（而非静态文本）取证。
3. **`TEST_EXTENSIONS` 的窄是有意的**：将来引入第二种测试扩展名（如 `.test.ts`）必须先改这份清单，
   否则新文件会被 `extensionErrors` 点名。这是**想要的**摩擦，但它是**一处需要记住的**改动点——
   与 `CAPABILITY_PACK_IDS` 仍手写（ADR 0056 遗留 4）同类。
4. harness 轨道仍 `pending` 的 **H18 与 B07**、W76 台账 5 条 `decidable_in_repo`
   （`PD-09/22/24/31/34`）的推荐落地，均未动手。
