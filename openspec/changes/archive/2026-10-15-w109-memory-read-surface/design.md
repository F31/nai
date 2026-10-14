## Context

§42.3 的读取判定 `read_records(records: &[ScopedRecord], query, policy, now_ms)` 已存在且被
`crates/nai-kernel/tests/memory.rs`（21 项）覆盖；§42.4 的 `forget(outcome)` 判定**陈述是否如实**
也已存在。两者都**零生产调用方**。W107 把原因钉死在 `ScopedRecord` 的三个条目级输入上：

- `permitted` 可由 `permission_grants` 诚实派生；
- `approximate_of` 恒 `None`；
- **`revoked` 在 `memory_records` 里没有承载列** ⇒ 只能写死 ⇒ 那是**注入判定而非注入事实**（W94）。

⇒ 本轮先造承载物，再接线。

## Goals / Non-Goals

**Goals**：给「撤销」一个仓库内的承载物；让 §42.3 的三个条目级输入**各由一条仓库事实派生**；
让 §42.4 的「先阻断读取」有一个**真的会阻断**的落点。

**Non-Goals**：不改任何判定规则；不做清除侧；不做配额来源；不做 Pro 跨会话；不改验收 `status`。

## Decisions

### 决策 1：承载物是 `revoked_at_ms INTEGER`（可空），不是 `revoked INTEGER NOT NULL DEFAULT 0`

**同族同形**：`permission_grants`、`approvals`、`devices` 三处既有撤销列**都**是
`revoked_at_ms INTEGER` 可空 ⇒ 沿用同一形状，不新造。
**可空的含义是「未撤销」**，与既有三处**一致**；而 `NOT NULL DEFAULT 0` 会引入一个
「0 = 未撤销」的新约定 ⇒ 同一件事的第二处定义。

⚠️ **与 0006 的差别要说清**：0006 用 `NOT NULL DEFAULT ''` 是因为它的注释明写
「老行没有该列，默认值是**有意义的**：它标记了早于该列存在的行」。本列的 NULL **不需要**承担
「标记老行」的职责（早于该列存在的行本来就是未撤销）⇒ 可空即可，无需默认值。

### 决策 2：三条输入各来自一条不同的仓库事实，且**互不顶替**

| 输入 | 派生 | 拒因 |
|---|---|---|
| `revoked` | `revoked_at_ms IS NOT NULL` | `Revoked` |
| `permitted` | `permission_grants` 中 `(workspace, actor, 'session.manage', scope)` 存在且 `revoked_at_ms IS NULL` | `PermissionDenied` |
| `approximate_of` | 恒 `None` | `ApproximateSubstitution` |

- `permitted` 的 `scope`：**观众长期记忆**用 `viewer_scope`，其余用 `session_id`
  ——与 `scope_matches`（观众按 viewer_scope、其余按 session）**同形**，不另立一套。
- **权限名复用 `session.manage`**（W107 的同一个选择），**不新增** `memory.read` ⇒ 不扩张授权面。
  ⚠️ **这是选择不是本来如此**，与 W107 一致地披露。
- **`approximate_of` 恒 `None` 不是「写死」**：`memory_records` **结构上不存**顶替条目
  （没有指向「本条目是某条目的近似」的列）⇒ 恒 `None` 是**真的事实**。
  **能由结构蕴含的 MUST NOT 再具名一次** ⇒ 不加列、不为它造承载物。

### 决策 3：取数入口 MUST NOT 先做作用域判定

`Store::memory_records` 的 SQL 是 `WHERE workspace_id=?1 AND session_id=?2`
⇒ 它**已经**在按会话过滤。拿它喂 `read_records`，`out_of_scope` **永远没有对象**
⇒ 守卫在，但它守的对象被上游先筛掉了（**假阴性**，最危险的方向）。

⇒ 新增 `memory_scoped`：**只按 `workspace_id` 过滤**（workspace 是硬边界，不是 §42.3 的判定对象），
把作用域判定**完整留给** `read_records`。

**既有 `memory_records` 一个字不改**（有测试钉它「已过期记录仍被读回」）。

### 决策 4：判定一次，存储层不重跑 §42.3

存储层只**取数**（含 `revoked_at_ms`）与**写阻断**，MUST NOT 施加越作用域/过期/撤销/权限/顶替/配额
六类判定中的任何一类（同 W107 对写入面的约束）。派生放在 runtime。

### 决策 5：阻断只置一次

`block_memory_read` 对**已撤销**的行**不覆盖** `revoked_at_ms`
（撤销时刻是事实，重复写会篡改它）⇒ 幂等但不覆盖。

### 决策 6：`forget_memory` 先阻断、再判陈述

§42.4 原话「先阻断检索与读取，再异步清除副本」⇒ 顺序不可交换。
本轮只做**阻断**这一半；`purge_state` 如实登记 `NotStarted`，**不**宣称已清除。

## Alternatives considered

1. **编辑 `0001-foundation.sql` 加列**——`verify()` 校验每个迁移文件的 SHA-256，
   编辑 0001 会让**所有既有数据库**失效（0006 的注释逐字写着）⇒ 否决。
2. **新建 `memory_revocations` 表**——需要 JOIN，且与既有三处「撤销都是**列**」的形状不符
   ⇒ 否决（同族同形优先）。
3. **复用 `expires_at_ms` 表示撤销**（撤销时把它设为过去）——会让 `Revoked` 与 `Expired`
   两个拒因**不可区分**，且丢失原时效 ⇒ 否决（「为什么被拒」必须可判）。
4. **直接 DELETE 行表示阻断**——§42.4 要求「**先**阻断、**再异步**清除」，
   阻断与清除是**两个时刻**；删除会让「已阻断」与「已清除」不可区分 ⇒ 否决。
5. **复用 `Store::memory_records` 喂读取面**——见决策 3，`out_of_scope` 会失去对象 ⇒ 否决。
6. **新增权限名 `memory.read`**——扩张授权面；W107 已选择复用 `session.manage`，
   本轮保持一致 ⇒ 否决（并披露这是选择）。
7. **给 `approximate_of` 也造一个承载列**——结构上不存顶替条目，加列就是给一个
   **不可能非空**的输入造承载物 ⇒ 装饰 ⇒ 否决。

## Risks

- **版本号成套四处漏一处**就会在 CI 红（W51 实测）⇒ 本轮四处同改 + `schema --check` 真验。
- **`permitted` 若恒 true 会让 `PermissionDenied` 成为死分支** ⇒ 本轮用
  `permission_grants.revoked_at_ms` 让它**可达**，并写一条用例**证明它可达**（不是只断言它存在）。
- **加列会让 `memory_records` 的列数从 9 变 10** ⇒ W107 的「九列与 `MemoryRecord` 九字段逐字对应」
  这一叙述**已过期**，活文档 MUST 就地订正（列 ≠ 契约字段：撤销是**存储态**，不是契约字段）。
