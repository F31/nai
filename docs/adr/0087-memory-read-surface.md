# ADR 0087：§42.3 记忆读取判定与 §42.4 遗忘阻断接上真实存储（H04 runtime 接线）

- 状态：已接受
- 日期：2026-10-15
- 轮次：W109
- 关联：[ADR 0085](0085-memory-write-gate-wiring.md)（H03 写入面接线，本轮的直接前驱，
  其**决策 8** 把「如何诚实派生 `revoked`」登记为具名开放问题，本轮闭合它）、
  [ADR 0067](0067-viewer-memory-single-answering-point.md)（观众记忆开关的单一应答点）、
  [ADR 0027](0027-trace-id-single-definition.md)（一处定义）

## 背景

V3.11 §42.3 把记忆读取钉成一条**固定顺序**的链：

> 读取 MUST 先按作用域过滤（workspace、session、场景），再按时效过滤。过期、被撤销或权限不足的条目
> MUST NOT 进入上下文，且 MUST NOT 用「近似条目」顶替（沿用 §21.2「不得跨知识域任意切换备份知识」）。

§42.4 把遗忘钉成一条**先阻断后清除**的链：

> TTL 到期、事实失效、权限撤销、用户请求删除四类触发共用 §24.4 的删除链：**先阻断检索与读取，
> 再异步清除副本**。

W57 交付了这两块的**判定面**（`read_records` / `forget`），W107 把写入面接上了真实存储，
却在决策 8 处停下：

> `read_records` 的条目级门控输入是 `ScopedRecord` 的 `revoked` / `permitted` / `approximate_of`。
> ……但 **`revoked` 在 `memory_records` 里没有任何承载列**……⇒ 在读取面写死 `revoked: false` 就是
> **凭空断言一条判定输入**……故 H04 的接线留给新的一轮。

于是今天**无法被判**的命题：

1. 一条**已撤销**的记忆，在 runtime 上到底会不会进上下文（不是「判定面说不该进」，而是「真有执行体拦它」）？
2. 一次**遗忘请求**发生后，读取侧是不是**真的**先失效了（而不是只记了一句「我打算删」）？
3. 一条**越作用域**的记录，是被**具名拒绝**，还是在上游被悄悄筛掉以致无人知晓？

这与 W103（H15）、W104（H02）、W105（H18）、W106（H01）、W107（H03）**完全同型**，是
「判定面存在、执行体不存在」在本仓连续**第六轮**现身。与前五轮**不同的一点**：**前五轮执行体已存在**
（决策者只需接上），本轮**连判定的输入都不存在** ⇒ 必须先**造一个承载物**，才谈得上接线。

## 决策

### 决策 1：承载物是**新增迁移** `0008-memory-revocation.sql`，MUST NOT 编辑 `0001`

```
ALTER TABLE memory_records ADD COLUMN revoked_at_ms INTEGER;
```

⚠️ **`migration::verify()` 校验每个**已应用迁移文件**的 SHA-256**（`0006` 的注释里逐字写明）
⇒ 编辑 `0001` 会让**所有既有库**失效。要加列只能新开一个迁移。

列形 `INTEGER` **可空**，与 `permission_grants` / `approvals` / `devices` 三处既有撤销列
**同族同形**；与 `0006` 的 `NOT NULL DEFAULT ''` **不同**——0006 的默认值承担「标记老行」的职责，
本列不需要：**NULL 即「未撤销」**。

⚠️ **老行取值 MUST 是 NULL，MUST NOT 是 0**：`0` 会被 `revoked_at_ms.is_some()` 读成
「1970 年就已撤销」，把**每一行老记忆**一次性判死。这条升级路径在一个**已有行**的 v7 库上实测
（空表上的 `ALTER TABLE` 什么也证明不了）。

版本号**成套四处**同改（W51 的回归正是漏了一处）：`migration::VERSION` 7 → **8**、
`SQL: [&str; 7]` → `[&str; 8]`、`schemas/config/backup.schema.json` 的 `db_schema_version` const、
契约样本断言；TS 生成物由 `cargo xtask schema` 再生并**真验** `--check` ⇒ 实测 `no drift`。

### 决策 2：三条派生**互不顶替**，各自取自一个事实

| `ScopedRecord` 字段 | 派生自 | 为何不是别处 |
|---|---|---|
| `revoked` | `revoked_at_ms.is_some()` | 唯一的撤销承载物（决策 1） |
| `permitted` | 记录**所属类别**要求的授权是否在场 | 见下 |
| `approximate_of` | 恒 `None` | 存储里**结构上不存在**顶替条目 |

三条对应 §42.4 四类触发中的三类（`Ttl` 走既有 `expires_at_ms`，不另设列）。

⚠️ **`approximate_of` 恒 `None` 不是「偷懒」，是**结构蕴含**：仓库里没有任何「为顶替别条而准备的条目」
这一对象 ⇒ 无对象可派生，也不该为它造承载物（W94：能由结构蕴含的 MUST NOT 再具名一次）。
代价是 `ApproximateSubstitution` 经本入口**结构上不可达**——这在守卫的闭世界里**具名登记**，
其覆盖在内核 `read_records` 直连的用例里。

⚠️ **`permitted` 刻意不查 `permission_grants`**：取数入口 `Store::memory_scoped` **已经**要求 actor
持有一条 `session.manage` 授权，条目级若再查**同一条** grant，二者**恒一致**
⇒ `PermissionDenied` 将永远**不可达**——**守卫还在，却没有任何对象可供它守卫**
（本项目明令禁止的形状）。观众长期记忆是 §42.3 里**唯一**被要求「读取 MUST 有授权引用」的类别，
其承载物是记录**自己**的 `authorization_ref`。而 `admit_write` **不**要求观众记录带授权引用
⇒ 缺授权引用的观众记录**可入库但读不出来**，`PermissionDenied` **可达**（有用例专门钉它）。

### 决策 3：取数**不预筛**作用域——`Store::memory_scoped` 只按 `workspace_id` 过滤

`out_of_scope` 这条守卫需要有**对象**可拒。若取数阶段就按会话把别的作用域筛掉，那条守卫还在、
但它守的东西已被上游先拿走 ⇒ **假阴性**（假阳性与假阴性不对称，**永远选安全方向**）。

既有 `Store::memory_records` 保留它的会话过滤**一字不改**（有测试钉它「已过期记录仍被读回」）；
`memory_scoped` 存在的理由正是让读取面**有东西可判**。两个入口都**不施加** §42.3 的任何条目级判定。

### 决策 4：判定**一次**——存储层只取数与阻断，不判 §42.3 的六类

`read_records` 在 runtime 侧跑**一次**；存储层只回答「有哪些行、每行撤销了吗」。
一条判定答两次会分家（W88）。`now_ms` 由调用方给，入口内**不另立时间源**。

请求级违规 ⇒ `MemoryReadError::Rejected`（**整个读取不成立**，一条记录都没被考虑）；
条目级违规 ⇒ 逐条进 `outcome.refused` 并**具名**。
⚠️ 请求级拒绝 **MUST NOT** 折成一次「空读取」——否则「这次读取不成立」与「读出来零条」不可区分，
而后者是被**允许**的。故障（权限/冲突/超时）走 `MemoryReadError::Fault`，**不**记在 §42.3 名下。

### 决策 5：阻断只置一次——`Store::block_memory_read` 返回**生效中的**时刻

已撤销 ⇒ 返回**原有**时刻，**不覆盖**。撤销时刻是一条**事实**，重写它等于篡改。

⚠️ **阻断 MUST NOT 用删除行表达**：那样「已阻断」与「已清除」将无法区分，而 §42.4 要求
「**先**阻断检索与读取，**再**异步清除副本」。**清除副本不在此处**，那是另一条工作。

### 决策 6：**先**阻断，**再**判陈述——且 `read_side_blocked` 不是调用方字段

`Composition::forget_memory` 先写撤销列（阻断真的发生），再拿这个**事实**构造 `ForgetOutcome`。

⚠️ `read_side_blocked` **MUST NOT** 由调用方填：若可填，本入口就会自己构造一个恒真的
`ForgetOutcome`，判定变成装饰。代价同决策 2：`ReadSideNotBlocked` 经本入口**结构上不可达**，
其覆盖在内核 `forget()` 直连的用例里，并在守卫的闭世界里**具名登记**。

### 决策 7：非空洞守卫**双向**钉——穷尽分类 + 端到端实测集合

- `cover(rejection)` 对 `MemoryRejection`（23 变体）**穷尽 `match`**：每个变体要么归
  `Reached`（本文件真的构造了它），要么归 `Elsewhere(why)`（`why` 必须写出**承载它的是谁**——
  只写「别处」等于把洞留在原地）。kernel 新增变体而此处未决定归属 ⇒ **编译失败**。
- 闭世界：`NOT_CONSTRUCTED_HERE` **恰为 3 条**（`UnknownOperation` §42.6 操作面 /
  `ApproximateSubstitution` 结构上不存在 / `ReadSideNotBlocked` 结构上不可达）。
- 反向一钉：一个端到端用例断言**实测观察到**的集合与 `READ_FORGET_FACE`（13 条）**逐项相等**
  ⇒ 「分类说可达」与「真的产生得出来」是两件事，两头都钉。

### 决策 8：H03/H04 的 `status` **仍保持 `pending`**

本轮闭合的是「判定面 → 执行体」，**不是验收达标**。控制面接线、§42.4 的清除副本一侧、
Pro 跨会话（§42.5）仍未建。

## 后果

- **闭合**：`read_records` 与 `forget` 从零生产调用方变为各有落点；「已撤销的记忆会不会进上下文」
  与「遗忘后读取侧是否真的失效」从「只能读源码回答」变成**可测**。
- **一次迁移**：`0008-memory-revocation.sql`，`migration::VERSION` 7 → **8**；版本号**四处**同改
  且生成物**真验**（`cargo xtask schema --check` ⇒ `no drift`）。这是本轮与 W107「零迁移」的**实质差别**。
- **零新增依赖、零契约字段改动**：`revoked_at_ms` **不是** `MemoryRecord` 的契约字段
  （它是存储层的门控状态），故契约与生成物除版本号外无改动。
- **权限面零改动**：两个新入口复用既有权限名 `session.manage`。
- **明确留下的缝（如实登记）**：
  - §42.4 的**清除副本**一侧未建（只到阻断）⇒ 一条被遗忘的记录**仍在库里**，只是读不出来；
  - `ApproximateSubstitution` 与 `ReadSideNotBlocked` 经本入口**结构上不可达**（决策 2 / 决策 6），
    在守卫的闭世界里具名登记；
  - `permitted` **不查** `permission_grants`（决策 2）——是设计，不是遗漏。
- **不变**：`read_records` / `forget` / `admit_write` 及其它 kernel 判定规则**一行未改**；
  H03 与 H04 的 registry `status` **保持 `pending`**。

## 替代方案

1. **编辑 `0001-foundation.sql` 加列** —— `verify()` 校验每个迁移文件的 SHA-256，会让所有既有库失效。
2. **用删除行表达阻断** —— 「已阻断」与「已清除」不可区分，违背 §42.4 的顺序（决策 5）。
3. **复用既有 `expires_at_ms` 表达撤销** —— 两件事合成一列，之后无法回答「它到底是过期还是被撤销」。
4. **`revoked` 写死 `false`**（W107 之前唯一可行的形状）—— 凭空断言一条判定输入（W94 的形状）。
5. **取数按会话预筛** —— `out_of_scope` 失去对象，假阴性（决策 3，已由注入 I2 实测：实测集合里
   `OutOfScope` **整个消失**）。
6. **`permitted` 查 `permission_grants`** —— 与取数入口的授权恒一致，`PermissionDenied` 永不可达
   （决策 2）。
7. **`read_side_blocked` 由调用方填** —— 调用方可构造恒真 `ForgetOutcome`，判定变装饰（决策 6）。
8. **请求级拒绝折成空读取** —— 「这次读取不成立」与「读出来零条」不可区分（决策 4）。
9. **把 `revoked_at_ms` 加成契约字段** —— 它是存储层的门控状态，不是 `MemoryRecord` 的内容；
   加进契约会扩张契约面而 runtime 侧并不因此多判一件事。
10. **顺手做 §42.4 的清除副本** —— 那是另一条工作（需要副本清单与清理执行体），混进来会让
    「阻断已完成」与「清除已完成」重新不可区分。

## 关联

- 实现：`crates/nai-runtime/src/memory.rs`（`read_memory` / `forget_memory`）、
  `crates/nai-storage/src/store.rs`（`memory_scoped` / `block_memory_read`）、
  `crates/nai-storage/src/migration.rs`、`migrations/sqlite/0008-memory-revocation.sql`
- 测试：`crates/nai-runtime/tests/memory.rs`（**18** 项，其中 W109 新增 9 项）、
  `crates/nai-storage/src/migration.rs`（v8 升级路径实测）
- 规格：`openspec/specs/runtime-composition/spec.md`（W109 新增 2 条 + 修订 1 条）
- 报告：`docs/reports/2026-10-15-w109-memory-read-surface.md`
- 活文档：`docs/runtime.md`、`docs/storage.md`、`docs/kernel.md`、`docs/development-plan.md`
- 登记：`tests/registry.yaml`（H03 / H04 追加 W109 段；**两条 `status` 未改**）
