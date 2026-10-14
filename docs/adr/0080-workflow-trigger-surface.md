# ADR 0080：workflow 的**触发面**（`on:`）必须由解析兑付

- 状态：已接受
- 日期：2026-10-14
- 轮次：W102
- 关联：[ADR 0073](0073-workflow-step-closure.md)（步层闭世界）、[ADR 0075](0075-gate-coverage.md)（包集合与命令面）、
  [ADR 0077](0077-tier-payment.md)（站点层级兑付）、[ADR 0078](0078-step-configuration.md)（执行配置面）、
  [ADR 0079](0079-step-target-binding.md)（目标绑定）

## 背景

W95 给 `.github/workflows/foundation.yml` 的 step 集补了**步层闭世界**；W96 管住了 `cargo xtask` 的命令面；
W97 管住了包集合与「CI 调用哪些命令」；W98 管住了 npm 脚本执行面；W99 管住了站点层级兑付；
W100 管住了执行配置面（跑不跑／在哪儿跑／失败算不算数）；W101 管住了每个判定步的**目标绑定**。

这七轮补的全是「workflow **里面**」的层。而「这个 workflow **什么时候会跑**」——`on:` ——
**不在任何判定域内**：

- `scripts/contracts/workflow-steps.mjs` 的 `workflowJobsOf(text)` 只读 `doc?.jobs`，
  **从不读 `doc?.on`**；
- 全仓 `grep`「`.on` / `trigger` / `pull_request` / `branches:`」在 `scripts/contracts/*.mjs`
  与 `tests/**` 下**零命中**。

⇒ 触发面是 W95–W101 **全部判定**的**前提**，而这个前提**没有任何判据**。

```yaml
on:                        # ← 不在任何判定域内
  push:
  pull_request:
jobs:
  foundation:
    steps: [门控步, 8 个判定步, 诊断步]   # ← W95/W96/W97/W99/W100/W101 全部在这里工作
```

### 实测（探针，改真文件、真跑 10 个 portable 套件、逐字节还原、`sha256` 逐字一致）

基线 `245 passed / 0 failed`。

| 探针 | 改动（真改 `.github/workflows/foundation.yml`） | 失败向量（10 套件） | 结论 |
| --- | --- | --- | --- |
| P-A | 诊断步**读方**改路径（`nai-gate.log` → 别的名） | `[0,…]` | **零信号**（⇒ §遗留） |
| P-B | 门控步**写方**改路径（`tee` 目标改名） | `[0,…]` | **零信号**（⇒ §遗留） |
| P-C | 删掉 `pull_request`（PR 不再被门控） | `[0,…]` | **零信号** |
| P-D | 整个 `on:` 缩成仅 `workflow_dispatch` | `[0,…]` | **零信号** |
| P-E | `push` 限定到**不存在**的分支 | `[0,…]` | **零信号** |
| P-F | `tee` 目标换成 `/dev/null`（日志消失） | `[0,…]` | **零信号**（⇒ §遗留） |

P-C/P-D/P-E 不是「漏了一个字段」：**它们让整套门控静默地不再运行**，而所有步层判据照样全绿——
因为「有人会跑这个 workflow」从来没被兑付过。仓库里对此**只有一句散文**：
「workflow 有 `on: push`，每次 push 都已自动触发」（`AGENTS.md:651`、`docs/development-plan.md:541`）。
散文不是判据。

⇒ 母题「**声明 MUST 付代价**」的第十一次出现。前十次补的是**集合**与**配对**；本轮补的是**前提**。

### 解析事实（`yaml` v2.9.1，实测）

| `on:` 写法 | `YAML.parse(text).on` |
| --- | --- |
| `on:\n  push:\n  pull_request:` | `{push: null, pull_request: null}` |
| `on: push` | `'push'` |
| `on: [push, pull_request]` | `['push','pull_request']` |
| `on:\n  push:\n    branches: [main, release]` | `{push: {branches: ['main','release']}}` |
| `on: {}` | `{}`（空 ⇒ 永不触发） |
| （`on:` 缺失） | 键**不存在**（`hasOwnProperty` 为 `false`） |

⚠️ **`on` 是字符串键，不是布尔 `true`**（`yaml` v2 默认 YAML 1.2 core schema，实测 `Object.keys(doc)`
为 `['on']`）⇒ `doc.on` 可靠，不需要防御 `doc.true`。

## 决策

### 决策 1：在册表 `WORKFLOW_TRIGGERS` 单独一份，**不**塞进 `WORKFLOW_STEPS`

W101 决定 1 说「一个 step 的 `target` 住那个 step 自己的条目上，MUST NOT 另开一张表」——
那是**因为** target 是 *step 这个对象* 的属性。本轮的对象是**触发器**，不是 step：
它不是任何一个 step 的属性，而是**整个 job 集的前置条件**。⇒ 它有自己的在册表，字段 `{host, event, filters}`。

W101 纪律在这里的正确形式是「**同一件事 MUST NOT 两处定义**」：「这个 workflow 有哪些触发器」
只有 `WORKFLOW_TRIGGERS` 一处；解析出的集合与它双向对账，MUST NOT 在 step 条目上再声明一份。

### 决策 2：判定单元是 `(event, filter-key)`，MUST NOT 是裸 filter-key

`branches` 在 `push` 与 `pull_request` 上语义相同但**归属不同对象**。按裸键名比较，
把 `pull_request` 的 `branches` 挪给 `push` **看不出差别**（W91：「同名」≠「同一个」）。
⇒ 单元 = `${event}\u0000${key}`，对账按它建 `Map`。

### 决策 3：**事件名的闭集**是判据的骨架；类别由事件表**推导**，MUST NOT 存成字段

两个闭集：`AUTOMATIC_EVENTS`（由仓库活动驱动：`push` / `pull_request` / `pull_request_target` /
`merge_group` / `release` / `schedule` / `workflow_run`）与 `MANUAL_EVENTS`（只能被显式启动：
`workflow_dispatch` / `workflow_call` / `repository_dispatch`），二者**不相交**、并集是事件名的**闭世界**。
域外 ⇒ 具名失败（W90）。

⚠️ **为什么「域外 ⇒ 具名失败」不能省**：若把「除手动表以外的都算自动」，一个**拼错的事件名**
（`pus`）会被推导成自动触发 ⇒ 一条「至少有一个自动触发」的断言会在一个**永不触发**的 workflow
上通过——**假阴性**，最危险的方向（W92：假阳性与假阴性不对称 ⇒ 永远选安全方向）。

`kind`（`automatic` / `manual`）**不写成在册字段**：它由 `triggerKindOf(event)` 从两张表推导
（「能由结构蕴含的 MUST NOT 再具名一次」）。手写一定会与表漂移，而漂移的方向恰好是
「一个手动触发被当成自动触发」⇒ 同样是假阴性。

⚠️ **这条决定的兑现物是字段集闭世界**（探针 P-B5 逼出来的一条）：`kind` 不写，但**判据也不读它**
⇒ 给一个条目**手写** `kind: 'automatic'` 时判据**一个字都不说**。那不是「少报」，而是
**一条没人会读的声明静静地挂在表上**——写它的人以为它有意义，甚至以为改成 `manual` 就改变了语义。
这正是母题「**声明 MUST 付代价**」的反面。⇒ 条目上的键 MUST **恰好**落在
`TRIGGER_FIELDS = ['host','event','filters']` 里，域外 ⇒ 具名失败（错拼的 `filter` 同理）。

### 决策 4：R3「至少一条自动触发」是唯一直接钉「前提」的断言，且**两侧各断言一次**

- **在册侧** `declared.some(kind === 'automatic') === false` ⇒ 在册表退化成恒真；
- **解析侧** `produced.some(kind === 'automatic') === false` ⇒ 这个 workflow 事实上不会被任何仓库事件驱动。

双向闭世界（R1）会让两者**大多数情况**同真同假，但**不是所有情况**：把 `workflow_dispatch`
也补进在册表、同时删掉 `push`，R1 成立而解析侧仍须红——这正是 P-D 的形状。⇒ 两条都必要，MUST NOT 合并。

### 决策 5：R2 过滤器**双向集合相等**——堵的是**假阴性**方向

`push: {branches: [main]}` 与 `push: {branches: [nope]}` 的**事件集完全相同**（都只有 `push`），
只做 R1 看不见 ⇒ 而 `branches` 恰好决定 push **是否真的会触发**（P-E 的零信号正来自此）。

- **施加了而没人声明**（P-E）⇒ 有人**悄悄地关掉**了触发；
- **声明了而没有施加** ⇒ 声明是**空话**。

两个方向都具名，取值集合也双向（`[main]` vs `[main, release]` 具名）；集合元素排序后比较——
**顺序不是声明的一部分**。

### 决策 6：形状闭集切两刀——**解析侧只认形状，判定侧才认域**

`workflowTriggersOf` 只做**形状**归一化（两种 `on:` 承载 + 事件配置必须是 `null` 或「值为非空字符串数组的映射」），
**不检查键名域**；`FILTER_KEYS` 与事件名域由**判据**检查。这与 W101 的 `targetsOf`/判据切分同形：
解析器负责「读得懂／读不懂」，判据负责「读懂了的东西合不合法」。把域检查塞进解析器会让一条笔误
（`branch:` 少个 `s`）把**整个触发面**判成「读不懂」，丢失粒度。

事件配置**形状**域外（`schedule: [{cron: …}]` 的取值是数组、`workflow_dispatch: {inputs: …}`
的取值是对象）⇒ 解析器**抛**一个具名错误 ⇒ 判据转成具名失败。**域外 ⇒ 具名失败，MUST NOT 静默降级**。

### 决策 7：判分由在册条目驱动，且**不重复**报别的判据已报的东西

只改 `on:` 时，W95（步集）、W100（执行配置）、W101（目标绑定）**必须全部沉默**——
正交性由测试逐条断言。反向（只改 `run` / 只改步名）时本判据也必须沉默。

### 决策 8：缺输入 / 空注册表 / `on:` 缺失或空 / 事件重名 / 不可解析 ⇒ **具名失败**

一条会因入参缺失而恒真的断言比没有断言更坏；而一个空的触发面本身就是一个「永不运行」的判定。

### 决策 9：**层级差**是本轮立论的核心——W100 与 W102 不可互推

W100 已经在「步」这一层判了「会不会跑」，但它的输入是 `job.strategy.matrix` × `step.if`
——**它读不到 `on:`**。一个 workflow 可以在**每个步上都「会跑」**，而整个 workflow **永不触发**。
⇒ 两层对象不同，不可互推。

## 后果

- **零产品代码改动**：`.github/workflows/foundation.yml` **一个字符不动**（`sha256` 前后一致：
  `2f46d5f52ce8b9af86c6b8947b0c1f012168f411ac50cc78133a68da1a248c1c`）；零新依赖。
- `tests/`（`tests/ui` 除外）与 `apps/desktop/tests` 下的 Node 套件 **12 → 13**，
  portable 面 **245 → 260**；登记站点 **14 → 15**；`xtask/src/lib.rs` **+11 行**（4 行登记 + 7 行注释，
  登记行超出 rustfmt 行宽 ⇒ 由 `cargo fmt` 折行，见报告 §六.1）；
  规格 `engineering-foundation` **+1 added**（正文 **254 字符**、8 个 scenario）。
- 注入 **P-A1–P-A8 + P-B1–P-B5**（13 条）+ 1 条等价改写对照，全部**真改文件、真跑 11 个套件、
  逐字节还原**（`foundation.yml` 与 `workflow-triggers.mjs` 的 `sha256` 前后一致、两文件 `CR=0` 不变）。
  另加一层**判据直调**（不依赖测试管线）：**13 条注入每一条都是
  `W95=0 / W100=0 / W101=0 / W102≥1`**，P-A8（等价改写为列表写法）**四个判据全 `0`**。
  ⚠️ **套件层与判据层必须分开记**：套件向量里的 `workflow-triggers` 列红 2–3 条，
  但其中**只有 1 条是判据在响**，另 1–2 条是**正交性用例的前提被探针同时打掉**（锚点不存在 ⇒
  `withoutPullRequest()` **具名失败**，MUST NOT 变 no-op）⇒ 列向量只能证「只有这一列在响」。
- **如实披露**：当前仓库的 `on:` **没有任何过滤器** ⇒ R2 的单元集合在真实仓库上是**空集**，
  它的非空洞由**合成世界**（覆盖一个带过滤器的单元 + 一个裸事件）与**注入用例**（P-A3/A4 + P-B2）
  共同证明——「判据非空洞」与「在册数据恰好简单」是两件事。
- **订正（就地，活文档）**：`scripts/contracts/test-reachability.mjs` 的台账注释原文写
  「实测 **9** 个测试文件全部落在某个站点的覆盖集里」——随站点增补**早已不实**，
  且是「易漂移的计数进活文档」的同一条禁令。改为不钉数字的表述并写明理由。
- **遗留（如实登记）**：P-A/P-B/P-F 实测零信号的**跨步数据传递**缝（门控日志的写方
  `tee "$RUNNER_TEMP/nai-gate.log"` 与读方 `ci-diagnose.mjs "$RUNNER_TEMP/nai-gate.log"`
  是两处独立字符串、无对账）本轮**不裁**——「一条待裁定项只在本轮裁一个」。
  另：**不判事件配置**（`workflow_dispatch.inputs`、`schedule.cron`，出现即具名失败）；
  **不判分支/路径是否真的存在**（那要读 refs，且宿主瞬态 MUST NOT 改变结论）；
  **不判 `permissions:` / `concurrency:` / `env:`**（workflow 头部是各自独立的一层）。

## 替代方案

1. **只做 R1（事件集双向），不做 R2（过滤器）**：P-E 完全看不见——而 P-E 的形状
   （`push` 限定到不存在的分支）正是「门控事实上不再跑」最隐蔽的一种。
2. **「除手动表以外都算自动」**：少一张表，但拼错的事件名会被判成自动触发 ⇒
   「至少一个自动触发」在最需要它的时候失效。假阴性方向。
3. **把 `kind` 写进在册条目再断言它等于推导值**：多一个字段、多一条会失败的路，而**不增加任何信息**
   （能由结构蕴含的 MUST NOT 再具名一次）。⇒ 不存。
4. **把触发器塞进 `WORKFLOW_STEPS`**：触发器不是任何一个 step 的属性，塞进去要么重复声明
   （每个 step 都写一遍 `on:`）要么引入一个没有身份的伪 step。
5. **只判「事件集非空」**：`on: [workflow_dispatch]` 会通过——而它意味着门控**只在有人手工点按钮时才跑**。
6. **顺手把 `permissions:` 也一起判了**：那是另一层对象，混进来会让本轮的边界与证据变糊。
7. **不检查条目字段集**（本轮**先写成这样、被探针 P-B5 否证**）：看上去「反正判据不读它」，
   但一条没人读的字段是**没有代价的声明**——写它的人以为它有意义。⇒ 改为字段集闭世界。

## 关联

- 规格：`openspec/specs/engineering-foundation/spec.md` 的
  「Every workflow's trigger surface is paid for by parsing its `on:` block」
- 报告：[`docs/reports/2026-10-14-w102-workflow-trigger-surface.md`](../reports/2026-10-14-w102-workflow-trigger-surface.md)
- 变更：`openspec/changes/archive/2026-10-14-w102-workflow-trigger-surface/`
- 记忆：`.workbuddy/memory/KNOWLEDGE.md` §6.25；`AGENTS.md` 的 W102 段
