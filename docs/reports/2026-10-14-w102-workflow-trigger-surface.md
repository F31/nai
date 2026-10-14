# W102 轮次报告：workflow 的**触发面**（`on:`）必须由解析兑付

- 日期：2026-10-14
- 变更：`openspec/changes/2026-10-14-w102-workflow-trigger-surface/`
- ADR：[0080](../adr/0080-workflow-trigger-surface.md)
- 新增模块：`scripts/contracts/workflow-triggers.mjs`
- 新增测试：`tests/contracts/workflow-triggers.test.mjs`（15 个用例）

## 一、目标

母题「**声明 MUST 付代价**」的**第十一次**换对象。前十一轮补的全是「workflow **里面**」的层：
步集（W95）→ 命令面（W96）→ 包集合（W97）→ npm 脚本执行面（W98）→ 站点层级（W99）→
执行配置面（W100）→ 目标绑定（W101）。本轮补的是它们的**前提**：
「这个 workflow **什么时候会跑**」——`on:`——**不在任何判定域内**。

## 二、缝在哪

先说**结论**：触发面是 W95–W101 **全部判定**的前提，而这个前提**没有任何判据**。

### 2.1 零覆盖的直接证据

- `scripts/contracts/workflow-steps.mjs` 的 `workflowJobsOf(text)` **只读 `doc?.jobs`，
  从不读 `doc?.on`**（W101 为了拿 `matrix` 才刚扩展过它，仍未碰触发面）；
- 全仓 `grep`「`.on` / `trigger` / `pull_request` / `branches:`」在 `scripts/contracts/*.mjs`
  与 `tests/**` 下**零命中**。

### 2.2 探针（先测后设计：6 条，改真文件 + 真跑 10 套件 + 逐字节还原 + `sha256` 一致）

基线 `245 passed / 0 failed`（registry 52 / schema 30 / platform 22 / workflow 13 / xtask-commands 17 /
gate-coverage 24 / npm-scripts 34 / tier-payment 22 / step-config 15 / step-target 16）。

| 探针 | 改动（真改 `.github/workflows/foundation.yml`） | 失败向量（10 套件） | 处置 |
| --- | --- | --- | --- |
| P-A | 诊断步**读方**改路径（`nai-gate.log` → 别的名） | `[0,…]` **零信号** | ⇒ §八 遗留 |
| P-B | 门控步**写方**改路径（`tee` 目标改名） | `[0,…]` **零信号** | ⇒ §八 遗留 |
| P-C | 删掉 `pull_request`（PR 不再被门控） | `[0,…]` **零信号** | **本轮裁** |
| P-D | 整个 `on:` 缩成仅 `workflow_dispatch` | `[0,…]` **零信号** | **本轮裁** |
| P-E | `push` 限定到一个**不存在**的分支 | `[0,…]` **零信号** | **本轮裁** |
| P-F | `tee` 目标换成 `/dev/null`（日志消失） | `[0,…]` **零信号** | ⇒ §八 遗留 |

**两条候选缝同时摆上桌，本轮只裁一条**：触发面（P-C/D/E）是**前提层**的缝、且仓库**已有记载**
可兑付；跨步数据传递（P-A/B/F）的写方 ↔ 读方是两处独立字符串。按「一条待裁定项只在本轮裁一个」，
后者**登记为 §八 遗留**——同时裁两条会让判据的边界与证据都变糊。

### 2.3 为什么触发面是本轮该裁的那一条

1. 它是**前提**：门控步、判定步、诊断步——它们全部假设「有人会跑这个 workflow」；
2. 仓库**已有记载**（`AGENTS.md:651`、`docs/development-plan.md:541`：
   「workflow 有 `on: push`，每次 push 都已自动触发」）⇒ 钉它**不是**给未知事实贴标签，
   而是把一句**散文**变成判据（「已声明 ≠ 已执行」的第 N 次出现）；
3. 它的失败模式**最安静**：不是「少检查一样东西」，而是**整套门控不再运行而所有判据全绿**。

### 2.4 解析事实（`yaml` v2.9.1，实测；`node -e` 内联 `\n` 不可靠 ⇒ 探针写成文件再跑）

| `on:` 写法 | `YAML.parse(text).on` |
| --- | --- |
| `on:\n  push:\n  pull_request:` | `{push: null, pull_request: null}` |
| `on: push` | `'push'` |
| `on: [push, pull_request]` | `['push','pull_request']` |
| `on:\n  push:\n    branches: [main, release]` | `{push: {branches: ['main','release']}}` |
| `on: {}` | `{}`（空 ⇒ 永不触发） |
| （`on:` 缺失） | 键**不存在**（`hasOwnProperty` 为 `false`） |

⚠️ **`on` 是字符串键，不是布尔 `true`**（`yaml` v2 默认 YAML 1.2 core schema，实测
`Object.keys(doc)` 为 `['on']`）⇒ `doc.on` 可靠，**不需要**防御 `doc.true`。

## 三、判据设计

新增 `scripts/contracts/workflow-triggers.mjs`。六条判据 + 两条纪律：

| 编号 | 判据 | 守卫的探针 |
| --- | --- | --- |
| R1 | 事件集**双向闭世界**（解析出的每个事件在册；在册的每个事件被解析出） | P-A1／P-A5／P-B1 |
| R2 | **过滤器**双向对账，判定单元 `(event, filter-key)`，取值集合双向相等 | P-A3（P-E）／P-B2 |
| R3 | 至少有**一条自动触发**，在册侧与解析侧**各自**断言 | P-A2（P-D） |
| R4 | **事件名闭世界**（`AUTOMATIC_EVENTS` ∪ `MANUAL_EVENTS`、不相交），类别由表推导 | P-A5／P-B3 |
| R5 | 形状闭集（`on:` 三写法／过滤器键 ∈ `FILTER_KEYS`／取值非空字符串数组） | P-A4 |
| R6 | **条目字段集闭世界**（键集 MUST 恰好等于 `TRIGGER_FIELDS`） | **P-B5** |
| — | 缺输入 / 空注册表 / `on:` 缺失或空 / 事件重名 / 不可解析 ⇒ **具名失败** | P-A6／P-A7 |

**九条设计决定**（详见 [ADR 0080](../adr/0080-workflow-trigger-surface.md) 与变更 `design.md`）：

1. 在册表 `WORKFLOW_TRIGGERS` **单独一份**，MUST NOT 塞进 `WORKFLOW_STEPS`——触发器**不是任何一个
   step 的属性**，而是整个 job 集的**前置条件**（W101 决定 1 的判据是「它是不是**这个对象**的属性」）。
2. 判定单元 MUST 是 `(event, filter-key)`，MUST NOT 是裸键名（`branches` 在 `push` 与
   `pull_request` 上是**两个可分别篡改的过滤器**——W91：同名 ≠ 同一个）。
3. **事件名闭世界**；`kind` 由事件表**推导**、**MUST NOT 存成字段**（能由结构蕴含的 MUST NOT 再具名
   **一次**）。⚠️ **「除手动表以外都算自动」是错的**：一个拼错的事件名（`pus`）会被推导成自动触发
   ⇒「至少一条自动触发」在一个**永不触发**的 workflow 上通过（**假阴性，最危险的方向**）。
4. **R3 两侧各断言一次**：双向闭世界让两侧**大多数情况**同真同假，但**不是所有情况**——
   补上 `workflow_dispatch` 同时删掉 `push` ⇒ R1 成立而解析侧仍须红（P-D 的形状）。
5. **过滤器双向相等**堵的是**假阴性**方向：`push: {branches: [main]}` 与 `{branches: [nope]}`
   的事件集**完全相同**，只做 R1 看不见；而 `branches` 恰好决定 push 是否真的会触发（P-E）。
6. **解析侧只认形状、判定侧才认域**——域检查塞进解析器会让一个笔误（`branch:` 少个 `s`）把
   **整个触发面**判成「读不懂」，丢失粒度。
7. 判分由在册条目驱动，**不重复**报 W95／W100／W101 已报的东西（正交性由测试逐条断言）。
8. **条目字段集闭世界**（**由探针 P-B5 逼出来的一条**，见 §5.4）。

**决定 8 是本轮最值得记的一条**：`kind` 不写成字段、判据也不读它 ⇒ 给一个条目**手写**
`kind: 'automatic'` 时**没有任何信号**。那不是「少报」，而是**一条没人会读的声明静静地挂在表上**
——写它的人以为它有意义，甚至以为改成 `manual` 就改变了语义。这正是母题「声明 MUST 付代价」的
**反面**。⇒ 「不存 `kind`」这条决定的**兑现物**就是「键集 MUST 恰好等于
`TRIGGER_FIELDS = ['host','event','filters']`」。

## 四、实现

| 文件 | 变化 |
| --- | --- |
| `scripts/contracts/workflow-triggers.mjs` | **新增**（`TRIGGER_FIELDS`／`TRIGGER_KINDS`／`AUTOMATIC_EVENTS`／`MANUAL_EVENTS`／`FILTER_KEYS`／`WORKFLOW_TRIGGERS`／`workflowTriggersOf`／`triggerKindOf`／`filterKeyOf`／`workflowTriggerErrors`） |
| `tests/contracts/workflow-triggers.test.mjs` | **新增**，15 个用例 |
| `scripts/contracts/test-reachability.mjs` | 新增站点 `xtask-engineering-workflow-triggers`（14 → 15）；**就地订正**一处过期计数注释（§4.1） |
| `tests/registry.test.mjs` | 逐字站点清单 14 → 15 |
| `xtask/src/lib.rs` | **+11 行**（4 行登记 + 7 行注释）；Rust **零行为改动** |
| `.github/workflows/foundation.yml` | **一个字符未动**（被审对象；`sha256=2f46d5f5…8c1c` 前后一致） |

### 4.1 本轮附带订正一处**易漂移的计数**（活文档）

`scripts/contracts/test-reachability.mjs` 的台账注释原文写：

> 当前**为空**：实测 **9** 个测试文件全部落在某个站点的覆盖集里。

随站点增补（W95/W96/W97/W98/W99/W100/W101/W102 各加一个）**早已不实**，且正是
「**易漂移的计数与版本号 MUST NOT 进活文档**」这条禁令的实例。改为**不钉数字**的表述并写明理由
——它并不比「覆盖集非空」那条断言多说什么。

⚠️ **同形不同义、不订正**：`docs/adr/0062`、`AGENTS.md` 的 W84 段、`docs/development-plan.md`
的 W84 段里的「**七个站点**」是**关于 W84 那一轮**的记录 ⇒ 事实记录，**不订正**。

## 五、非空洞验证

**两层证据**：套件向量（证明「只有这一列在响」）+ **判据直调**（证明「判据本身响了几条」）。
后者是本轮新增的一层——因为套件红可能来自**前提**，而判据本身才是被断言对象。

### 5.1 判据直调边界矩阵（`.w102-judges.mjs`，不依赖测试管线）

基线：`W95=0 W100=0 W101=0 W102=0`。13 条注入逐条真改文件、逐字节还原：

| 注入 | 改动 | W95 | W100 | W101 | W102 | 还原 `sha256` |
| --- | --- | --- | --- | --- | --- | --- |
| P-A1 | 删掉 `pull_request` | 0 | 0 | 0 | **1** | `2f46d5f5…` |
| P-A2 | `on:` 缩成仅 `workflow_dispatch` | 0 | 0 | 0 | **4** | `2f46d5f5…` |
| P-A3 | `push` 限定到不存在的分支 | 0 | 0 | 0 | **1** | `2f46d5f5…` |
| P-A4 | `push` 的过滤器键域外（`branch`） | 0 | 0 | 0 | **2** | `2f46d5f5…` |
| P-A5 | 事件名域外（`pull_requests`） | 0 | 0 | 0 | **3** | `2f46d5f5…` |
| P-A6 | `on:` 为空 | 0 | 0 | 0 | **1** | `2f46d5f5…` |
| P-A7 | `on:` 整块删掉 | 0 | 0 | 0 | **1** | `2f46d5f5…` |
| **P-A8** | **等价改写为列表写法** | **0** | **0** | **0** | **0** | `2f46d5f5…` |
| P-B1 | 在册表删掉 `pull_request` | 0 | 0 | 0 | **1** | `ddb0c886…` |
| P-B2 | 在册表给 `push` 加没人施加的过滤器 | 0 | 0 | 0 | **1** | `ddb0c886…` |
| P-B3 | 在册事件名域外（`pus`） | 0 | 0 | 0 | **2** | `ddb0c886…` |
| P-B4 | 在册条目丢掉 `filters` 字段 | 0 | 0 | 0 | **2** | `ddb0c886…` |
| P-B5 | 在册条目手写 `kind` | 0 | 0 | 0 | **1** | `ddb0c886…` |

⇒ **每一条注入都只有 W102 一个判据在响**；P-A8（等价改写）四个判据**全 `0`**。
两文件 `CR=0` 不变（`read_bytes`／`write_bytes` 逐字节写入，`\r` 计数前后一致）。

### 5.2 套件向量：列级证据，与直调分开记

11 个 portable 套件（列序 registry / schema / platform / workflow / xtask-commands / gate-coverage /
npm-scripts / tier-payment / step-config / step-target / **workflow-triggers**）：

```
P-A1 0 0 0 0 0 0 0 0 0 0 3      P-B1 0 0 0 0 0 0 0 0 0 0 3
P-A2 0 0 0 0 0 0 0 0 0 0 3      P-B2 0 0 0 0 0 0 0 0 0 0 2
P-A3 0 0 0 0 0 0 0 0 0 0 3      P-B3 0 0 0 0 0 0 0 0 0 0 2
P-A4 0 0 0 0 0 0 0 0 0 0 3      P-B4 0 0 0 0 0 0 0 0 0 0 3
P-A5 0 0 0 0 0 0 0 0 0 0 3      P-B5 0 0 0 0 0 0 0 0 0 0 3
P-A6 0 0 0 0 0 0 0 0 0 0 3      P-A7 0 0 0 0 0 0 0 0 0 0 3
P-A8 0 0 0 0 0 0 0 0 0 0 0   ← 等价改写（对照）
```

⚠️ **套件层的 `workflow-triggers` 列红 2–3 条，其中只有 1 条是判据在响**
（`the shipped workflow pays for its trigger surface`）；另 1–2 条是**正交性用例的前提被探针
同时打掉**——探针改掉了 `on:` 的字面形状 ⇒ 用例的锚点 `on:\n  push:\n  pull_request:\n` 不存在 ⇒
`withoutPullRequest()` **具名失败**（它**MUST NOT** 变成 no-op）。⇒ **列向量只能证「只有这一列在响」，
「判据响了几条」必须看 §5.1 的直调。**

### 5.3 P-A8 是本轮踩到并修掉的一条**假红**（W101 §2.2 的同型）

第一次跑 P-A8（`on:` 等价改写为列表写法，期望**全绿**）得到 **2 条红**。逐条读 `not ok` 与
assertion message 后确认**不是判据在响**：那两条正交性用例把 `'  pull_request:\n'` 当字面锚点，
换成列表写法后**匹配不到** ⇒ 用例因**前提没了**而红。

**处置 = 修前提，不改断言**（铁律一）：

```js
function withoutPullRequest(text) {
  const candidates = [
    ['on:\n  push:\n  pull_request:\n', 'on:\n  push:\n'],
    ['on: [push, pull_request]\n', 'on: [push]\n'],
  ];
  for (const [from, to] of candidates) if (text.includes(from)) return patch(text, from, to);
  assert.fail('the shipped "on:" block must be one of the known spellings, or this premise is vacuous');
}
```

修后 P-A8 **全绿**（四个判据全 `0`）。第三种 `on:` 写法出现时它**具名失败**，MUST NOT 静默变成 no-op。

### 5.4 P-B5 是**判据的缝**、不是测试的缝（本轮唯一一条「探针逼出一条新判据」）

P-B5（给在册条目手写 `kind: 'automatic'`）第一次跑：**判据返回 `[]`**——只有测试的 F0
非空洞断言（`!('kind' in item)`）把它拦下。⇒ 这不是测试在响，而是**判据漏了一层**：
一条没人读的字段是一条**没有代价的声明**。补上 R6（字段集闭世界）后，P-B5 的直调由 `0` 变为 **1**，
套件层也由 1 条红变为 3 条（F0／F0b／出厂绿）。

### 5.5 真实仓库的前置条件

`the shipped workflow pays for its trigger surface` 断言：
① 判据在出厂状态下返回 `[]`；② 解析出的触发面**非空**；③ 至少有一条**自动**事件；
④ 在册表与解析出的**事件集合**一致。
⚠️ R2 的过滤器单元在真实仓库上是**空集**（当前 `on:` 无任何过滤器）——它的非空洞由
**合成世界**（F0 断言「至少一个带过滤器的单元 + 至少一个裸事件」）与注入用例证明，见 §九 2。

## 六、门控与 validate

### 6.1 全量门控（限并发）

命令：`source /c/Users/32791/.nai-msvc-env.sh && CARGO_TARGET_DIR=E:/nai-build-target CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo xtask check --profile full`

**结果：`EXIT=0`。**

| 计数 | 值 |
| --- | --- |
| `test result` 行数 | **91**（74 `Running` + 18 `Doc-tests`，另 1 行为 rust doc 汇总） |
| Rust | **862 passed / 0 failed** |
| `LNK1104` / `out of memory` / `error[` | **0 / 0 / 0** |
| 墙钟 | **5m20.506s** |

node 侧三段（TAP `# pass` 逐段现测）：

- **11 个 portable 套件**：`52 / 30 / 22 / 13 / 17 / 24 / 34 / 22 / 15 / 16 / 15`，**合计 260**（`workflow-triggers` 是新增的第 11 个，贡献 **15**）。
- `node scripts/frontend-gate.mjs`：**5 passed / 0 failed**。
- 前端 `test:ui`：**52 passed / 0 failed**。

**portable 面 245 → 260**（+15，恰为本轮新套件）。

**两次非零退出的处置（都不是代码缺陷）**：

1. **第一次 `EXIT=1` 停在 `cargo fmt --all --check`**：本轮在 `xtask/src/lib.rs::engineering_checks` 新增的登记行超出 **rustfmt 行宽 100**，diff 给出折行建议。定性＝**我这次改动自己的格式缺陷**（非宿主瞬态）⇒ 按建议折成 4 行后重跑。
2. **第二次 `EXIT=1`：末个目标命中宿主间歇 `LNK1104`**（`无法打开 …nai_xtask-*.exe`），冒泡为 `cargo test --locked --package nai-xtask --lib failed`。此时 `test result` 行已是 **90**（73+18 / 810 passed / 0 failed）、node 11 套件全绿。处置按既定判据：① 单独复跑该目标 ⇒ `EXIT=0 / 52 passed / 0 failed` ⇒ **逐目标能过 ⇒ 非代码缺陷**；② `CARGO_BUILD_JOBS=1` 重跑整个 `full` ⇒ **`EXIT=0`（上表）**，**代码一字未改**。

### 6.2 两条 validate

| 命令 | 结果 |
| --- | --- |
| 变更级（归档前）`validate 2026-10-14-w102-workflow-trigger-surface --strict --no-interactive` | **is valid**（无警告） |
| `validate --all --strict --no-interactive` | **43 passed / 0 failed** |
| `validate --all --strict --no-interactive --archived` | **100 passed / 0 failed** |

归档入口：`archive 2026-10-14-w102-workflow-trigger-surface --yes`（系统日期戳 **2026-10-14**）⇒ `Task status: ✓ Complete`，`engineering-foundation` 收 **+1 added**。

## 七、CI

**推送**：`41fdf61 → eb9c8b5`（`eb9c8b5e66a8fd025e90aa59b1ed46cb3a0a1009`），
`GIT_SSH_COMMAND` 经 `ssh.github.com:443` 推送 `main`。

**单次低频观测**（唯一查询参数破坏代理缓存 + `Cache-Control: no-cache` + 回读 `head_sha`）：

| 项 | 值 |
| --- | --- |
| run | **#137**（id `38517660655`） |
| `head_sha` | `eb9c8b5e66a8fd025e90aa59b1ed46cb3a0a1009` —— **== 本次推送 SHA** |
| 结论 | **status = completed / conclusion = success** |
| 墙钟 | `created 05:37:49Z → updated 05:48:16Z`（约 10m27s） |

**双平台逐 job**：

| job | 结论 | 起止 |
| --- | --- | --- |
| `foundation (ubuntu-24.04)` | **success** | `05:37:52Z → 05:42:30Z`（≈4m38s） |
| `foundation (windows-latest)` | **success** | `05:37:52Z → 05:48:15Z`（≈10m23s） |

**观测纪律**：本轮不做轮询——首次查得 run 处于 `in_progress` 后，按「等到有结论再单次查询」执行，
全程 **3 次请求**（runs 列表 1 + run 详情 2 + jobs 1），远低于匿名配额 60/h。
**`head_sha` 回读 == 推送 SHA** 是「看到的是本次推送的那条 run」的唯一凭据（W89 教训）。

> 本节的 CI 结论与本报告其余部分在同一次 commit 中提交；「回填提交自身不回填」。

## 八、遗留

1. **跨步数据传递的写方 ↔ 读方无对账**（本轮登记、**未裁**）：门控步的写方
   `cargo xtask check --profile full 2>&1 | tee "$RUNNER_TEMP/nai-gate.log"` 与诊断步的读方
   `node scripts/ci-diagnose.mjs "$RUNNER_TEMP/nai-gate.log"` 是**两处独立字符串**——
   改任一侧（P-A 读方改名／P-B 写方改名／P-F `tee` 换 `/dev/null`）都**零信号**。
   「一条待裁定项只在本轮裁一个」⇒ 留给下一轮裁定落点。
2. **不判事件配置**：`workflow_dispatch.inputs`、`schedule.cron` 不是「会不会自动触发」的开关。
   本项目未使用；出现即**具名失败**（域外 ⇒ 具名，W90），MUST NOT 静默放行。
3. **不判分支／路径是否真的存在**：那要读 refs，且宿主瞬态 MUST NOT 改变结论（铁律二）。
   本轮只钉「施加的过滤器 == 声明的过滤器」。
4. **不判 workflow 头部的其它键**（`permissions:` / `concurrency:` / `env:`）：各自独立的一层。
5. **正交性用例的前提仍依赖 `on:` 的字面写法**：`withoutPullRequest()` 目前覆盖两种写法，
   第三种（或探针扰动）会**具名失败**——方向安全（假阳性），但会让套件层的列向量带上前提噪声。
6. **`TRIGGER_FIELDS` 的域是 `['host','event','filters']`**：将来若触发器需要新字段（例如
   `concurrency` 之类），MUST 显式加进这张表，否则会被 R6 具名。
7. W90 §八 1/3/4/5、W91 跨语言键级消费、W92 基线收录、W93 锚点唯一性、W94 `needs_external` 审计、
   W95 `ci-only` 成因、W96 五项、W97 六项、W98 §八 1/2/7、W99 §八 1/2/3、W100 §八 2/3/4、
   W101 §八 1/2/3/4/5 **均未动**。

## 九、如实披露

1. **本轮裁的是「两条候选缝」里的一条**：P-A/P-B/P-F（跨步数据传递）同样零信号，但按
   「一条待裁定项只在本轮裁一个」登记为 §八 遗留。MUST NOT 把「本轮没裁」读成「那条缝不存在」。
2. **R2 在真实仓库上是空集**：当前 `on:` 没有任何过滤器 ⇒ 过滤器对账无单元可比。
   它的非空洞由**合成世界 + 注入用例**证明——这是「判据非空洞」与「在册数据恰好简单」两件事，
   MUST NOT 靠「反正现在没过滤器」把它当成装饰。
3. **套件层与判据层必须分开记**：§5.2 的列向量里 `workflow-triggers` 列的红**大多包含前提红**；
   只有 §5.1 的判据直调才是「判据本身响了几条」的权威来源。这一条与 W101 §2.2
   （「第一轮探针的红大多不是判据在响」）同源，本轮以**新增一层直调**把它变成结构性保障。
4. **`on` 的 YAML 语义是实测的**（`yaml` v2.9.1，YAML 1.2 core schema ⇒ `on` 是字符串键）。
   若将来换 YAML 解析器或 schema 版本，`doc.on` 的可靠性 MUST 重新实测。
5. **`bash` 内联 `\n` 不可靠**：本轮两次踩到（`node -e` 把 `\n` 当字面、`python -c` 锚点计数判错）
   ⇒ 探针一律**写进文件再跑**。这是工具纪律，不是判据缺陷，但会让「探针报错」被误读成「判据在响」。
6. **本轮就地订正了一处活文档中的过期计数**（§4.1），并明确区分了三处「同形不同义」的历史记录。
