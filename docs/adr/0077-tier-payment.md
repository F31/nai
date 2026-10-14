# ADR 0077：站点图层声明（`tier`）必须由宿主解析出的区域兑付

- **日期**：2026-10-14
- **状态**：已接受（Corresponding change：`openspec/changes/2026-10-14-w99-tier-payment`）
- **关联**：ADR 0075（被测面的包集合）、ADR 0076（npm 脚本的执行面）、ADR 0074（本机命令面的闭世界）、
  W84 的「登记站点」闭世界、W91 的「判定单元 MUST 是身份而非名字」、W97 的「注释不是实参」

## 背景

`scripts/contracts/test-reachability.mjs` 的 `TEST_REGISTRATION_SITES` 有 11 条站点，每条声明
`{id, host, marker, base, tier}`。其中 `host`/`marker` 这一面由 W84 的 `reachabilityErrors` 管住了
（逐字验「声明的 marker 真的写在 host 里」、覆盖集非空、闭世界双向）。

另一面 `tier`（∈ `portable` / `full` / `ci`，意思是「**跑它的最低一层门控**」）此前**只有形状检查**：

```
siteShapeErrors()  →  TEST_TIERS.includes(site.tier)      ← 仅此而已
```

⇒ 它是**一个未付代价的声明**。把站点表里某个 `portable` 改成 `full`（或反过来），覆盖集不变、
闭世界不变、门控全绿。实测（探针 P-A：把 `tests/contracts/npm-scripts.test.mjs` 的注册从
`engineering_checks` **移进** `["check","--profile","full"]` arm，站点仍声明 `tier: 'portable'`）：

| 编号 | 改动 | 结果 |
| --- | --- | --- |
| **P-A** | 注册行搬出 portable 面，`tier` 不变 | **192 passed / 0 failed —— 零信号** |
| P-A 还原 | 备份拷回（**不用 `git checkout --`**） | `sha256 = f61b2c73…728c` **逐字一致** |

P-A 同时说明两件事：① `tier` 声明不付代价；② **注册点的位置**本身也没有守卫——把一行
`--test` 从 portable 面搬进 full 面，portable 面少跑一个套件，无人发现。

三个陷阱在这一层同时出现：

1. **注释不是引用**（W97 教训换一层，本轮**实测**）：`xtask/src/lib.rs` 第 46 行的**注释**里写着
   `tests/contracts/descriptor.test.mjs`——「（`tests/contracts/descriptor.test.mjs`）不在此列」。
   实测：`gateRegions` 给出的 `engineering_checks` 区域**原文含**该串，**剥离注释后不含**。
   不剥注释，两个 `ci` 站点会被这条注释**自我满足**。
2. **「出现过」不是「被调用」**：`apps/desktop` 在驱动表里有 3 个 STEP（`typecheck`/`build`/`test`），
   只有 `test` 的脚本值含 `tests/*.test.mjs`。用「该目录出现过 STEP」当兑付 ⇒ 那条路径恒真。
3. **对称方向被漏掉**：只做正向（「被它声明的层级兑付了吗」）会漏掉「声明**少报**了本机门控真正
   跑到的东西」——把 marker 混进 portable 面、站点仍声明 `ci`，就是一条**合法的逃离路径**。

## 决策

### 决策 1：兑付面由**解析**得出，MUST NOT 是站点自己那段文本

| tier | 兑付面（判定域） | 由谁解析 |
| --- | --- | --- |
| `portable` | `gateRegions(libText)` 的 `engineering_checks` 区域，**再 `stripRustComments`** | ADR 0074 + 0075 |
| `full` | `driverSteps()` 的 STEP 集合 + `packageScripts()` 的脚本体 | ADR 0076 |
| `ci` | `workflowStepsOf()` 的 `{identity, run}` 集合 | ADR 0073 |

这是 ADR 0074「门控闭包区域」教训的第四次复用：**marker 可填自己的文本 ⇒ 自我满足**。

### 决策 2：`portable` 的兑付形状由 host **派生**，不留可选的 `via` 字段

```
host === 'xtask/src/lib.rs'  → 兑付串 = marker        （内联注册：`--test <marker>` 就在区域里）
host !== 'xtask/src/lib.rs'  → 兑付串 = /<module>::\w+\(root/   （宿主模块在区域里被调用）
```

`xtask-registry` 是第二种：区域里出现的是 `registry::check(root)?;`，而 `tests/registry.test.mjs`
的字面量在 `xtask/src/registry.rs` 里。**不给可选的 `via` 字段**：「可选」就是判据的缝（W93）——
一个可省的字段会让某个站点悄悄走默认分支。

`\w+\(root` 把「模块被调用」收紧到「**真的有一次带 `root` 的调用**」；`root` 是本仓库这些入口的
统一实参约定（`deps::check(root)`、`registry::check(root)`、`schema::node(root, …)`）。
**如实披露**：若将来某入口改了实参名，该站点的 portable 兑付会**假红**——方向安全（假阳性而非假阴性）。

### 决策 3：`portable` 区域 MUST 剥注释

见背景陷阱 1。剥注释是正向断言与反向断言**共同的前提**，不是装饰：
`stripRustComments` 把第 46 行那条注释删掉之后，两个 `ci` 站点才真正只是 `ci`。

### 决策 4：`full` 的兑付用**脚本体**，不是「包目录出现过 STEP」

见背景陷阱 2。marker 与脚本值都相对**包目录**表达（`tests/ui/*.test.mjs` 相对 `.`、
`tests/*.test.mjs` 相对 `apps/desktop`），两者可逐字比对；`host` 先由 `packageDirOf` 解析成包目录。

### 决策 5：`ci` 的兑付落在某个 step 的 `run` 文本里，且**剥整行注释**

一条没有身份的 `run` 无法被指向，也就不该成为证据（W95 的身份定义）。`run` 文本先剥整行 `#` 注释
（含「前导空白 + `#` 到行尾」的尾注形态），否则在 `run:` 里写一行 `# tests/x.test.mjs` 就能把
「没有 step 跑它」换成「有 step 提到它」。
**如实披露**：本仓库的 `stepIdentity` 对只有 `run` 的步回落为 run 首行 ⇒「具名」近乎恒真，
T4 的**实质约束**是「某个 step 的 `run` 文本逐字提到它」。

### 决策 6：反向断言——`tier` 的语义是「跑它的**最低**一层」

正向只回答「这个站点**被它声明的层级**兑付了吗」。只做正向会漏掉背景陷阱 3。
⇒ `full`/`ci` 站点的 marker（剥注释后）MUST NOT 出现在 portable 区域里。

这条反向断言**恰好**由「最低一层」这个语义定义：本仓库的既有约定是——凡是
`engineering_checks` 里跑的测试，站点都声明 `portable`（即使 `full` 也跑它们，因为 portable ⊂ full）。
一个测试若在 portable 面里，它的 `tier` MUST 是 `portable`；声明 `full`/`ci` 就是**少报**。

### 决策 7：取值域闭世界要**两道**

- `Object.keys(TIER_PAYMENTS)` MUST 等于 `TEST_TIERS`：新增一个 tier 取值而不归类，会让判据
  **静默地不认识它**。只写「运行时域外 ⇒ 失败」是不够的——那道只能挡住**已有的**取值。
- 每个 tier 取值 MUST 至少有一个站点：**没有实例的规则恒真**。

### 决策 8：缺输入按其**需要**具名

`{sites, libText, driverText, workflowText, packages}` 里，只有当**存在该 tier 的站点**时那份
输入才是必需，缺了 ⇒ 具名报错。`gateRegions` 抛（区域不可定位）⇒ 转成一条具名错误，
MUST NOT 返回空错误数组（那会把「读不懂」伪装成「没问题」）。

## 后果

- **零代码改动**：被审对象 `xtask/src/lib.rs`（除加一行登记 + 一段注释）、`scripts/frontend-gate.mjs`、
  `.github/workflows/foundation.yml`、三份 `package.json`、`TEST_REGISTRATION_SITES` 的**既有 11 条**
  一个字符都不动。零新依赖。
- 新增 `scripts/contracts/tier-payment.mjs`（判据）与 `tests/contracts/tier-payment.test.mjs`（22 用例）。
  Node 套件 **9 → 10**（portable 面 192 → **214** 用例），站点 **11 → 12**。
- 判据 8 条：规则闭世界双向、`portable` 区域兑付（两种形状）、`full` 驱动表兑付、`ci` 具名 step 兑付、
  反向断言、取值域两道、缺输入具名、不可解析必失败。
- **注入验证**：P1–P6 全部真改文件、真跑套件、备份拷回、`sha256` 逐字还原
  （`lib.rs` `4ac2bbc1…9e05`、`foundation.yml` `2f46d5f5…8c1c`），每条产生**具名**错误；
  与 ADR 0073／0074／0075／0076 及 W84 站点闭世界**互不顶替**（0/0/2/0/0/3 的分布，见报告 §五）。
- **本轮暴露出一个流程缺陷并当场修掉**：探针循环若在**上一次被中断的**残留改动上取备份，
  `RESTORE OK` 会变成「把坏状态备份拷回」的假绿（实测踩到一次：`foundation.yml` 残留 P5 改动）。
  修法 = 每轮**先按索引复原并断言干净**，再取备份。这条已沉淀进 `KNOWLEDGE.md` §11。
- **如实披露**：`portable` 的拒否靠**剥离注释后的文字域**，不是「marker 是否为一次真实调用」
  ⇒ 若将来把某个 `ci`/`full` 站点的 marker 放进 `engineering_checks` 调用脚本的字符串字面量里，
  本轮的 T2/T5 会放行。**有意的宽松**，记入报告 §九。

## 替代方案

- **只做正向，不做反向**：把 marker 混进 portable 面、站点仍声明 `ci` 成了一条合法路径
  （背景陷阱 3）。**否决**。
- **给站点加一个显式 `via` 字段说明兑付串**：更「可读」，但它是一个**可省**的字段——
  省略时走默认分支，等于给判据开一道缝（W93）。改成由 host 派生，形状总函数、无默认分支。**否决**。
- **把 `tier` 从站点表里删掉，改成由兑付面反推**：那样 `tier` 不再是**独立声明**，
  「声明的兑付」这条母题就没了对象（W80：手写的那一份会漂移，但派生出的那一份无法被**声明错**）。
  `TEST_REGISTRATION_SITES` 是独立声明，正是 T1–T5 能被违反的前提。**否决**。
- **用运行时探测代替静态解析**（例如真跑三个 profile 看谁执行了）：违反**铁律二**——
  「能不能跑」是宿主事实，换一台机器会对同一个仓库得出不同结论。

## 关联

- 前置：ADR 0075（被测面包集合）、ADR 0076（npm 脚本执行面）、W84（站点可达性闭世界）；
  本 ADR 补的是它们都够不着的**第五层对象**：站点**声明的层级**由谁兑付。
- 母题第八次：`repositoryFiles` → `readTrackedText` → W94 两张事实表 → W95 `gateSources`
  → W96 门控闭包区域 → W97 包集合 → W98 脚本执行面 → **W99 站点层级声明**。
