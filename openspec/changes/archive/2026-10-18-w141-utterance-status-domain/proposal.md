# Proposal: W141 — utterance 取值域：一处声明、白名单化可用性

## Why

W139 第 5 个结论把 `utterances.status` 量成一句话：**它是裸整数，而且有三套互相不一致的口径**；
W139 的 N1 把「锁定词汇」记为**独立一轮**。本轮开工前逐条复核，计数与 W139 **全部吻合**
（MUST 现测，MUST NOT 照抄）：

| 集合 / 写入 | 处数 | 出处（现测） |
| --- | --- | --- |
| `NOT IN(11,12,13,14,15)` 级联保护 | 6 | `store.rs` 4 处 + `knowledge.rs` + `publication.rs` |
| `NOT IN(12,13,14,15)` 可用性 | 3 | `store.rs:955` / `:1027` / `:1122` |
| `BETWEEN 1 AND 5` 待批准带 | 3 | `store.rs:604` / `:1060` / `:1164` |
| `SET status=N` 写入 | 9 | `1`（INSERT）/ `6`（approval）/ `12`（×6 级联）/ `14`（reject） |
| 附带：撤回审批时的读取 | 3 | `status=12` 出现在 `approvals` 撤回语句里（`knowledge.rs`、`publication.rs`、`store.rs` 各一） |

该列在 `migrations/sqlite/0001-foundation.sql` 里是 `status INTEGER NOT NULL DEFAULT 1`：
**没有 CHECK 约束、没有枚举、没有常量**。所以上面那些数字就是它的**唯一**定义 ——
散落在 SQL 字符串里、且互不一致。

**分歧点只有一个值：11**。它在级联保护集里（不被改写），却不在可用性黑名单里（被当成可用）。
本轮**不裁定 11 是什么** —— 裁定需要它的写入者先存在，而它今天零写入者。

真正需要修的是另外两件事：

1. **没有一处声明**：取值域只以裸字面量存在，改任何一处都不会牵动另一处。
2. **可用性是黑名单**：`NOT IN(12,13,14,15)` 把「还没人声明过的取值」（`7..=10`、`16..`、以及 11）
   默认判成**可用**。可用是危险方向（可用 ⇒ 可播出 / 可外发），不对称时 MUST 选安全一侧。

**验收映射**：本轮落在 `approval-workflow` 规格（该规格已含「拒绝候选 SHALL 将 utterances 状态
置为 Blocked」与「存储恢复使待审批失效」两条，正是这些语句所在）。`tests/registry.yaml` 的 88 条里
**没有**对应「status 取值域」的验收条目 ⇒ 本轮**不**标任何条目状态，只落内部判据。

## What changes

- `crates/nai-storage/src/utterance_status.rs`（新，零新增依赖）：
  - 四个**有写入者**的具名取值：`STATUS_PENDING=1` / `STATUS_APPROVED=6` /
    `STATUS_INVALIDATED=12` / `STATUS_REJECTED=14`；
  - 三条带：待批准带 `1..=5`、不可用带 `12..=15`、级联保护带 `11..=15`；
  - `StatusClass` 五值（`Pending`/`Approved`/`Unusable`/`ProtectedOnly`/`Unassigned`）
    与 `classify()` —— **全域覆盖**，`else` 落到 `Unassigned`，所以「不知道」有一个可表示的取值；
  - `usable()`（白名单）与由它派生的 `usable_values()` / `usable_guard()` / `cascade_guard()`。
- 生产源码里的 status 数字**全部**改为由上述常量派生（6 + 3 + 3 + 9 + 3 处）。
- 判据：`crates/nai-storage/src/tests/status_domain.rs`（4 条）+ 模块内 3 条。
- 文档：ADR 0115、报告、`docs/development-plan.md`。

## Impact

- **行为只变一处**：可用性由黑名单改成白名单 ⇒ `11`、`7..=10`、`16..`、`0`、负数由「可用」
  变「不可用」。已写入的取值（`1`/`6`/`12`/`14`）判定**完全不变**，故既有 90 条用例全绿。
  这是「两处口径不一致时选拒绝」，不是合并也不是放宽。
- **零新增依赖**，纯逻辑 + SQL 字符串派生。
- `utterance_status.rs` 是取值域的**唯一**声明处；新增任何裸字面量都会被扫描判据挡下
  （一条**会过期**的断言）。
- `tests/registry.yaml` 88 条里没有对应验收条目 ⇒ 本轮**不**标任何条目为 `tested`。

## Alternatives considered

- **把 11 并进终态**：否决。11 在可用性口径里是「可用」，并进终态等于替它选了一边；
  而「合并」与「放宽」都不是裁定。本轮把它留在 `ProtectedOnly` 并具名登记。
- **给 11 编一个语义（比如「已交付」）**：否决。语义 MUST 由它的写入者定义，而它今天零写入者；
  编一个名字等于把「不知道」写成「知道」。
- **把可用性放宽到 `NOT IN(11..15)`**（让两处一致，方向取宽松）：否决。放宽会让更多取值进入
  危险方向；不对称时 MUST 选安全一侧。
- **把取值域做成完整枚举**：否决。真实形状是「带 + 具名值」—— `BETWEEN 1 AND 5` 说明
  `1..=5` 是一个**带**而不是五个枚举值；枚举化会伪造出五个并不存在的状态名。
- **`usable_values()` 只扫 `[1, 已批准]` 区间**：否决（**注入验证实测到的盲区**）。
  那样 `usable()` 一旦退化成黑名单写法，`1..=6` 两种写法结果相同 ⇒ guard 一字不变 ⇒
  行为判据**不会红**。改为扫全域 `[0, 不可用带上界]`。
- **让 `classify` / `usable` 仅供判据引用**：否决。那样它们是「没有被生产代码调用的判定面」
  （`dead_code` 告警已经指出来了）。改为让 `usable_values()` 由 `usable()` 派生，
  使派生链真的通到 SQL。
