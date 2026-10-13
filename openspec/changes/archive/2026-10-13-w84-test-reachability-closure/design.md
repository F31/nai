# w84 设计：可达性闭世界

## 一、核实（先核实再裁定）

用真实仓库实测，逐条落到事实：

**1. 登记机制的确切形状**（`grep` 全仓，排除 `node_modules`/`.git`/`target`/`docs`/`openspec`）：

| 站点 | host | marker | 相对基 | 层级 |
|---|---|---|---|---|
| `xtask-registry` | `xtask/src/registry.rs:5` | `tests/registry.test.mjs` | `.` | portable |
| `xtask-engineering-schema` | `xtask/src/lib.rs:47` | `tests/contracts/schema.test.mjs` | `.` | portable |
| `xtask-engineering-platform` | `xtask/src/lib.rs:59` | `tests/platform/platform-matrix.test.mjs` | `.` | portable |
| `xtask-contracts-suite` | `xtask/src/schema.rs:376` | `tests/contracts/descriptor.test.mjs` | `.` | ci |
| `ci-contract-suite` | `.github/workflows/foundation.yml:80` | `tests/contracts/descriptor.test.mjs` | `.` | ci |
| `root-test-ui` | `package.json:14` | `tests/ui/*.test.mjs` | `.` | full |
| `desktop-test` | `apps/desktop/package.json:10` | `tests/*.test.mjs` | `apps/desktop` | full |

**2. 层级归属**（读 `xtask/src/lib.rs`）：`engineering_checks` 是两个 profile 的公共前缀
⇒ 前三条 = `portable`；`frontend::check` 只在 `full` ⇒ 两个 glob = `full`；
`cargo xtask test --suite contracts` **不在** `check` profile 里（是独立 CI 步骤）⇒ `ci`。

**3. 当前在册测试文件 = 9 个**（`find`，全仓唯一扩展名 `.test.mjs`）：
`apps/desktop/tests/control.test.mjs`、`tests/contracts/{descriptor,schema}.test.mjs`、
`tests/platform/platform-matrix.test.mjs`、`tests/registry.test.mjs`、
`tests/ui/{approval-surface,desktop-shell,ui-layer-declaration,upgrade-cost}.test.mjs`。

**4. 补集盲区实测**（探针，非推断）：把 `tests/kernel/w84-probe.test.mjs`（`assert.fail`）加入索引
后跑 `check --profile portable` ⇒ `EXIT=0`、`grep -c "w84 probe"` = **0**。重跑取得带探针的绿
（91 目标 / 860 passed），探针用例名仍**零出现**。

> 首次探针运行曾因 `nai-runtime --test composition` 报 `DeadlineExceeded` 而红。按既有判据
> 「**单目标能过 ⇒ 非代码缺陷**」：单跑该目标通过 ⇒ 宿主瞬态；且**探针的用例名在日志里从未出现**，
> 与失败无关。这不是本轮引入的红。

**5. 反面假设已排除**：起初怀疑 `tests/ui/ui-layer-declaration.test.mjs` 是孤儿（只在一份报告里被
提及）。核实后**不成立**——`tests/ui/*.test.mjs` 是**通配**展开，4 个文件都跑。⇒ 记这一笔是为了
说明：**「被引用」不等于「被运行」，而「没被逐条列举」也不等于「不运行」**；两者都要读到实现。

## 二、裁定

### 1. 覆盖集必须**算出来**，不能**声明**

`TEST_REGISTRATION_SITES` 只登记「站点在哪里、写了什么」；覆盖集由 `marker` 原文在
**已跟踪路径集**上解析得出（`resolveSite`）。直接写一份「覆盖了哪些文件」的表会让
「表 == 它自己」成为恒真式（W80 结论 5）。

### 2. 用**已跟踪路径集**（`.git/index`）而不是 `readdir`

覆盖集按**仓库成员**计算：与工作区里有无散落文件无关（W75 的仓库范围纪律）。
副作用正好：**未跟踪的探针文件不参与覆盖集**，于是它必然触发闭世界（本轮证据即由此得到）。

### 3. 「已声明」≠「已执行」（假契约）

站点的 `marker` MUST 在 `host` 里**逐字存在**。一条只登记、没落地的站点必须红——
名字在表里、线上什么都没有，与 W73 的假契约同族。

### 4. **非空洞**：站点必须真的覆盖到东西

一个匹配不到任何在册文件的站点是一条空声明。注意这条**不是**「站点数 > 0」——
后者对一个空 glob 是瞎的。`frontend-gate.mjs` 已在运行时对 glob 做同样的事
（「未匹配到任何文件即失败」），本条把它**移进**判据。

### 5. 闭世界 MUST 在**扩展名层**也成立

只认 `.test.mjs` 而不管「仓库里有没有别种测试扩展名」，等于把盲区搬到**更少人看**的一层：
`foo.test.ts` 既不在覆盖集里、也不在台账里，**文件层闭世界对它完全看不见**（它的扩展名
让 `isTestFile` 为假 ⇒ 连「未被覆盖」都不会被报出）。⇒ 先判 `looksLikeTest`（宽），
再判扩展名 ∈ 清单（窄）；宽出来的那块被**具名**点名。

### 6. 台账不是豁免（自清理）

`LOCALLY_UNREACHABLE` 的每条 MUST 带非空 `reason`、MUST 是**在册测试文件**、
MUST **确实未被覆盖**。最后一条让它**自清理**：某天某个站点开始覆盖它，那条过期豁免立刻红
——否则台账会变成「加进去就再也不用管」的垃圾桶。

### 7. 「覆盖集非空」与「两集不相交」必须**成对**

台账当前为空 ⇒ 「覆盖 ∩ 台账 == ∅」退化成恒真。若站点表被清空或全部解析失败，
上面的文件层闭世界会因台账为空而**全部静默通过**——即「全不覆盖」也能绿。
⇒ 必须补一条 `covered.size > 0`。这是「非空是互斥类断言的必要一半」在本轮的具体形状。

## 三、注入设计（非空洞）

数据级（改在册集合 / 站点表 / 台账，均用**临时站点表或临时台账**注入，不改真实文件）：

- **F0** 基线：真实站点 + 真实在册集 + 空台账 ⇒ **0 错误**（本轮不引入红）。
- **F1**（洞的复现）：把探针文件作为**在册**加入 ⇒ 闭世界报「tracked but no registration site
  covers it」。**这是本轮要修的洞的直接证据**。
- **F2**（假契约）：站点 marker 改一个字符 ⇒ 报「host 不含它」。
- **F3**（glob 缩窄）：`root-test-ui` 的 marker 换成 `tests/ui/approval-surface.test.mjs` ⇒
  其余 3 个 ui 文件落进闭世界。
- **F4**（空站点）：站点 marker 指向不存在的文件 ⇒ 报「非空洞」。
- **F5**（台账过期豁免）：把**已可达**的文件放进台账 ⇒ 报「已覆盖」。
- **F6**（扩展名补集）：在册集里塞一个 `tests/x/y.test.ts` ⇒ 报扩展名不在清单。
- **F7**（反向注入）：台账里放**真**未覆盖文件 + 把它从覆盖集拿掉 ⇒ **不红**（台账合法）。
- **F8**（反向注入）：站点表顺序打乱 ⇒ **不红**。
- **F9**（覆盖集非空）：站点表清空 ⇒ 报「no registration site covers any tracked file」。

表级（改 `TEST_REGISTRATION_SITES`／`TEST_EXTENSIONS` 本身，用**临时模块副本**注入，与 W83 同法）。
