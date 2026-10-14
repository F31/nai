# w102：workflow 的**触发面**（`on:`）——「门控什么时候跑」必须由解析兑付

## Why

前十一轮补的全是「workflow **里面**」的层：步集（W95）、命令面（W96）、包集合（W97）、npm 脚本执行面（W98）、站点层级（W99）、执行配置面（W100）、目标绑定（W101）。

但「workflow **什么时候跑**」——`on:` ——**不在任何判定域内**。`workflowJobsOf` 只读 `doc?.jobs`，**从不读 `doc?.on`**；全仓 `grep` 「`.on`／`trigger`／`pull_request`／`branches:`」在 `scripts/contracts/*.mjs` 与 `tests/**` 下**零命中**。于是它是 W95–W101 **全部判定**的前提，而这个前提**没有任何判据**。

实测（真改 `.github/workflows/foundation.yml`、真跑 10 个 portable 套件、逐字节还原、`sha256` 一致）：删掉 `pull_request`（PR 不再被门控）⇒ 零信号；把整个 `on:` 缩成仅 `workflow_dispatch`（push/PR 都不再触发）⇒ 零信号；把 `push` 限定到一个**不存在**的分支（push 事实上不再触发）⇒ 零信号。

这三条不是「漏了一个字段」：**它们让整套门控静默地不再运行**，而所有步层判据照样全绿——因为「有人会跑这个 workflow」从来没被兑付过。仓库里对此**只有一句散文**：「workflow 有 `on: push`，每次 push 都已自动触发」（`AGENTS.md:651`、`docs/development-plan.md:541`）。散文不是判据。

⇒ 母题「声明 MUST 付代价」的第十一次出现。前十次补的是**集合**与**配对**；本轮补的是**前提**。

## What Changes

1. **新增 `scripts/contracts/workflow-triggers.mjs`**：在册表 `WORKFLOW_TRIGGERS` + 闭集 `TRIGGER_FIELDS` / `TRIGGER_KINDS` / `MANUAL_EVENTS` / `AUTOMATIC_EVENTS` / `FILTER_KEYS` + 解析器 `workflowTriggersOf(text)` + 类别推导 `triggerKindOf(event)` + 判定单元 `filterKeyOf(event, key)` + 主判据 `workflowTriggerErrors({host, workflowText, registered})`。
2. **R1 事件集双向闭世界**：`on:` 解析出的每个事件 MUST 在册；在册的每个事件 MUST 出现在 `on:` 里。
3. **R2 过滤器双向对账**：判定单元 = `(event, filter-key)`，取值集合双向相等——堵「`push` 被限定到不存在的分支」（假阴性方向：门控事实上不再跑）。
4. **R3 至少一条自动触发**：在册侧与解析侧 MUST 各有 ≥1 个自动事件——堵「缩成仅 `workflow_dispatch`」。
5. **R4 事件名闭世界**（`AUTOMATIC_EVENTS` ∪ `MANUAL_EVENTS`，不相交），且**类别由事件表推导、MUST NOT 存成字段**——手写的类别一定会与表漂移，而漂移方向恰好是「把手动触发当成自动触发」。
6. **R5 形状闭集**：`on:` 的三种写法（字符串／数组／映射）都归一化；过滤器键 MUST ∈ `FILTER_KEYS`；过滤取值 MUST 是非空字符串数组。
7. **R6 字段集闭世界**：在册条目的**键集 MUST 恰好是** `TRIGGER_FIELDS`——一条**没人会读**的字段（手写的 `kind`、错拼的 `filter`）是一条**没有代价的声明**，正对着母题的反面。
8. **缺输入 / 不可解析 / `on:` 缺失 / `on:` 空 / 事件重名 ⇒ 具名失败**，MUST NOT 静默降级。
9. **`WORKFLOW_TRIGGERS` 在册两条**：`push` 与 `pull_request`，`filters={}`（实测该 workflow 未施加任何过滤器——「没有过滤器」也显式写出来）。
10. **新增 `tests/contracts/workflow-triggers.test.mjs`**：合成世界（三写法 + 各过滤器 + 自动/手动）+ 真实仓库 + 逐条注入 + 非空洞 + 与 W95/W100/W101 正交。
11. **规格**：`openspec/specs/engineering-foundation/spec.md` 新增 1 条 Requirement（ADDED）。
12. **文档**：ADR `0080`、`.workbuddy/memory/KNOWLEDGE.md` §6.25、`AGENTS.md`、`docs/development-plan.md`、`docs/reports/2026-10-14-w102-workflow-trigger-surface.md`。

## Impact

- **零产品代码改动**：`.github/workflows/foundation.yml` **一个字符都不动**（本轮只加判据）。
- `tests/`（`tests/ui` 除外）与 `apps/desktop/tests` 下的 Node 套件 **12 → 13**；portable 面用例 **245 → 245+N**；登记站点 **14 → 15**；`engineering_checks` 多一行；零新依赖。
- **明确不做**：不判**事件配置**（`workflow_dispatch.inputs`、`schedule.cron`——它们不是「会不会自动触发」的开关；本项目未使用，出现即**具名失败**，域外 ⇒ 具名）；不判 `on:` 里的分支/路径是否**真的存在**（那要读 refs）；不判触发面与矩阵各腿的关系（W100 的对象）。
- **如实披露**：当前仓库**没有任何过滤器** ⇒ R2 的过滤器对账在**真实仓库上是空集**，它的非空洞由**合成世界 + 注入用例**证明（见 design §五）。这是「判据非空洞」与「在册数据恰好简单」两件事——MUST NOT 靠「反正现在没过滤器」把这条判据当成装饰。
- **遗留登记（§八）**：跨步**数据传递**（门控日志的写方 `tee "$RUNNER_TEMP/nai-gate.log"` 与读方 `ci-diagnose.mjs "$RUNNER_TEMP/nai-gate.log"` 是两处独立字符串、无对账）——本轮探针 P-A/P-B/P-F 实测零信号，但按「一条待裁定项只在本轮裁一个」的纪律，登记为遗留。
