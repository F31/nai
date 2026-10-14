# w99 设计

## 一、问题形状

站点表是一条**双面**声明：一面是「**在哪里跑**」（`host` + `marker`，W84 已经管住，
`reachabilityErrors` 逐字验它），另一面是「**在哪一层跑**」（`tier`）。第二面当前**完全不付代价**：

```
TEST_REGISTRATION_SITES（11 条）        tier      兑付？       谁在跑它
  xtask-registry ............ registry.rs ...... portable .. ✗ 形状检查 .. engineering_checks → registry::check(root)
  xtask-engineering-schema .. lib.rs ........... portable .. ✗ 形状检查 .. engineering_checks → schema::node(root, --test …)
  …（另有 5 条同形）                                                       （新增 tier-payment.test.mjs 后 6 条）
  xtask-contracts-suite ..... schema.rs ........ ci ........ ✗ 形状检查 .. CI step 78 的 node --test
  ci-contract-suite ......... foundation.yml ... ci ........ ✗ 形状检查 .. 同上（同一处 step）
  root-test-ui .............. package.json ..... full ...... ✗ 形状检查 .. 驱动表 ['.', 'test:ui'] → tests/ui/*.test.mjs
  desktop-test .............. apps/desktop/… ... full ...... ✗ 形状检查 .. 驱动表 ['apps/desktop','test'] → tests/*.test.mjs
```

⇒ `tier` 是一个**未付代价的声明**（与 W75 / W86 / W94 / W95 / W96 / W97 / W98 同族）。
P-A 探针（把 `npm-scripts` 的注册从 `engineering_checks` 移进 full arm，站点仍说 `portable`）
**零信号**：192 passed / 0 failed。

## 二、证据（探针 P-A，改真文件 + 真跑 + 备份拷回 + `sha256` 逐字还原）

`.baseline`：`xtask/src/lib.rs` 的 `sha256 = f61b2c73…728c`；7 个 portable 套件合跑 **192 passed / 0 failed**。

| 探针 | 改动 | 结果 |
| --- | --- | --- |
| **P-A** | `npm-scripts.test.mjs` 的注册行从 `engineering_checks` 移进 `["check","--profile","full"]` arm | **192 passed / 0 failed —— 零信号** |
| 还原 | 备份拷回（**不用 `git checkout --`**，W97 教训） | `sha256 = f61b2c73…728c` ⇒ **RESTORE OK（逐字一致）** |

P-A 说明了两件事：① `tier` 声明不付代价；② **注册点的位置**本身也没有守卫
（把 `--test` 从 portable 面搬进 full 面，portable 面少跑一个套件，无人发现）。
T2 的「marker MUST 在剥离注释后的 `engineering_checks` 区域里」正是 P-A 的守卫。

## 三、设计决定

### 决定 1：兑付判定**由解析得出**，不是一个全文 `.includes()`

| tier | 兑付面（判定域） | 由谁解析 |
| --- | --- | --- |
| `portable` | `gateRegions(libText)` 里的 `engineering_checks` 区域，**再 `stripRustComments`** | W96 `gateRegions` + W97 `stripRustComments` |
| `full` | `driverSteps(driverText)` 的 STEP 集合 + `packageScripts(files)` 的脚本体 | W98 |
| `ci` | `workflowStepsOf(workflowText)` 的 `{identity, run}` 集合 | W95 |

这是 W96 教训的第四次复用：**marker 可填自己的文本 ⇒ 自我满足**。判定域 MUST 是解析出来的结构。

### 决定 2：`portable` 的兑付形状**由 host 派生出两种**，不留可选的 `via` 字段

```
host === 'xtask/src/lib.rs'  → token = site.marker      （内联注册：--test <marker> 就写在区域里）
host !== 'xtask/src/lib.rs'  → token = /<module>::\w+\(root/  （宿主模块在区域里被调用）
```

`registry.rs` 的站点是第二种：区域里出现的是 `registry::check(root)?;`，
而 `tests/registry.test.mjs` 的字面量在 `registry.rs` 里。**不给 `via` 可选字段**：
「可选」就是判据的缝（W93），一个可省的字段会让某个站点悄悄走默认分支。

`\w+\(root` 这个形状是把「模块被调用」收紧到「**真的有一次带 `root` 的调用**」
——`root` 是本仓库这些入口的统一实参约定（`deps::check(root)`、`registry::check(root)`、
`schema::node(root, …)`）。**如实披露**：若将来某入口改了实参名，该站点的 portable 兑付会**假红**
——方向是安全的（假阳性而非假阴性），报告 §八登记。

### 决定 3：`portable` 区域 MUST 剥注释（W99 的注释陷阱，实测）

`xtask/src/lib.rs` 第 46 行的**注释**里写着 `tests/contracts/descriptor.test.mjs`：

```
// （`tests/contracts/descriptor.test.mjs`）不在此列，仍留在 contracts suite。
```

实测：`gateRegions(lib.rs)` 的 `engineering_checks` 区域**原文**含该串（`raw=true`），
**剥离注释后不含**（`stripped=false`）。⇒ 若不剥注释，两个 `ci` 站点会被这条注释**自我满足**
（T5 的反向断言也会因此失效）。剥注释是 T2/T5 的前提，不是装饰。

### 决定 4：`full` 的兑付用**脚本体**，不是「包目录里出现过这个文件」

`full` 的判定域是：`packageDirOf(host)` → 驱动表里 `dir` 等于它的 STEP → **那些 STEP 的
`script` 在该包 `scripts` 里的值时** MUST 含 marker。marker 与脚本体都相对**包目录**表达
（`tests/ui/*.test.mjs` 相对 `.`、`tests/*.test.mjs` 相对 `apps/desktop`），两者可逐字比对。

「该目录下有任何 STEP」不算兑付：`root-test-ui` 与 `desktop-test` 各自只有一个 STEP，
但驱动表里 `apps/desktop` 有 3 个 STEP（`typecheck` / `build` / `test`）——
只有 `test` 的值含 `tests/*.test.mjs`。用「目录出现过」当兑付会让那条路径**恒真**。

### 决定 5：`ci` 的兑付必须落在**具名 step** 的 `run` 里，且剥整行注释

「具名」= `workflowStepsOf` 给出的 `identity !== ''`（W95 的身份定义）。理由与前几轮同：
一条没有名字的 `run` 无法被指向，也就不该成为证据。`run` 文本先**剥整行 `#` 注释**
（含「前导空白 + `#` + 到行尾」的尾注形态），否则在 `run:` 里写一行 `# tests/x.test.mjs`
就能把「没有 step 跑它」换成「有 step 提到它」。

### 决定 6：T5 反向断言（对称方向）——「逃离 portable 面」必须被拒

正向只回答「这个站点**被它声明的层级**兑付了吗」。只做正向会漏掉一个对称的洞：
把 marker 混进 `engineering_checks` 里（例如把 `ci` 站点的测试也塞进 portable 面），
站点仍声明 `ci` ⇒ 声明**少报**了本机门控真正跑到的东西。

⇒ `full` / `ci` 站点的 marker（**剥离注释后**）MUST NOT 出现在 portable 区域里。
实测在本仓库上：`descriptor.test.mjs` 只出现在注释里（剥后不含）、`tests/ui/*.test.mjs`
与 `tests/*.test.mjs` 通过 `tests/contracts/…` / `tests/platform/…` 也**不**是子串 ⇒ 三条都放行。

### 决定 7：规则闭世界 + 每个取值至少一个站点

- `Object.keys(TIER_PAYMENTS)` MUST 等于 `TEST_TIERS`（新增一个 tier 取值而没归类 ⇒ 红；
  取值域闭世界要两道——运行时「域外 ⇒ 失败」只挡得住**已有的**取值）。
- 每个 tier 取值 MUST 至少有一个站点：**没有实例的规则恒真**（与 W96/W97 的「四类都非空」同形）。

### 决定 8：缺输入按其**需要**具名，MUST NOT 静默跳过

入参 `{sites, libText, driverText, workflowText, packages}`：
只有当**存在该 tier 的站点**时那份输入才是必需；缺了 ⇒ 具名报错。
`libText` 不可解析（`gateRegions` 抛）⇒ 转成一条具名错误，MUST NOT 返回空错误数组
（那会把「读不懂」伪装成「没问题」）。

## 四、边界（与相邻判据互不顶替；实测数字见报告 §五）

| 判据 | 管什么 | 本轮的**独有**覆盖 |
| --- | --- | --- |
| W84 `reachabilityErrors` | 站点**在哪跑**（host+marker 逐字、覆盖集、闭世界） | 「tier 与真实区域**不符**」它一个字都不说 |
| W95 `workflowStepErrors` | **step** 在不在、`kind` 与 `localEvidence` | 它管 `kind`，不管 `tier`；同一个 workflow 文件、不同字段 |
| W97 `coverageErrors` | 包集合包含关系 | 无交叠 |
| W98 `scriptErrors` | 脚本**谁调用它** | 它管「脚本有没有消费者」；本轮管「站点的层级声明」，`full` 的判定域**共用** W98 解析出的 STEP 集合，但断言不同的东西 |
| W96 `registryShapeErrors` | 命令面 | 无交叠 |

## 五、非空洞

- 规则闭世界**非空**被断言（`TIER_PAYMENTS` 键集 == `TEST_TIERS`、每格 ≥ 1 站点、12 个站点逐字钉住）。
- 三种 tier 的兑付面**两两不同**（区域 / 驱动表 STEP / CI step），一条证据不可能同时给两种签名。
- 每条判据都有**注入用例**（合成世界 F0–F9 级 + 真实文件 P 级注入），且注入 MUST 具名失败。
- 反向（正向不红）也有用例：合成世界里 tier 与区域一致时**不红**。

## 六、明确不做 / 如实披露

1. **不验测试能跑通**（要执行，且**铁律二**：宿主瞬态 MUST NOT 改变结论）。
2. **不判 `frontend::check` 的真实执行**：只判驱动表这一层接线（真跑由 `check --profile full` 管）。
3. **不判 CI 的 `if:` / `continue-on-error`**（W98 §六 2 遗留，仍在）。
4. `portable` 的拒否是**文字域**而非「真实调用」（见 proposal §Impact 末段）：有意宽松。
5. `\w+\(root` 形状的假红方向（决定 2 末段）：有意保守。
6. **不改任何被审对象**（除 `lib.rs` 加一行登记）；`TEST_REGISTRATION_SITES` 的既有 11 条一字不动。
7. W90/W91/W92/W93/W94/W95/W96/W97/W98 的登记遗留**均未动**。
