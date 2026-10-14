# W109 报告：§42.3 记忆读取判定与 §42.4 遗忘阻断接上真实存储（H04 runtime 接线）

- 轮次：W109
- 日期：2026-10-15
- ADR：[0087](../adr/0087-memory-read-surface.md)
- 变更：`openspec/changes/archive/2026-10-15-w109-memory-read-surface/`
- 前驱：W107（[报告](2026-10-14-w107-memory-write-gate-wiring.md)、[ADR 0085](../adr/0085-memory-write-gate-wiring.md)）
  —— 其**决策 8** 把「如何诚实派生 `revoked`」登记为具名开放问题，本轮闭合它

## 一、目标

闭合 W107 留下的具名开放问题，把 §42.3 的读取判定与 §42.4 的遗忘链**接上真实存储**：

1. 一条**已撤销**的记忆，在 runtime 上会不会进上下文（不是「判定面说不该进」，而是「真有执行体拦它」）？
2. 一次**遗忘请求**发生后，读取侧是不是**真的**先失效了（而不是只记了一句「我打算删」）？
3. 一条**越作用域**的记录，是被**具名拒绝**，还是在上游被悄悄筛掉以致无人知晓？

## 二、缝在哪

### 2.1 直接证据（**现测**：从 `HEAD`——即本轮改动前——实测，不是推断）

```
$ git grep -n "read_records" HEAD -- crates apps | grep -v /tests/
HEAD:crates/nai-kernel/src/evaluate/memory.rs:635:pub fn read_records(     ← 只有定义
（其余三条都是文档注释）

$ git grep -n "\bforget(" HEAD -- crates apps | grep -v /tests/
HEAD:crates/nai-kernel/src/evaluate/memory.rs:839:pub fn forget(            ← 只有定义

$ git grep -n "const VERSION" HEAD -- crates/nai-storage/src/migration.rs
HEAD:crates/nai-storage/src/migration.rs:5:pub(crate) const VERSION: i64 = 7;
```

**`read_records` 与 `forget` 在生产代码里都是零调用方**（只有定义与注释）。`VERSION = 7`。

`HEAD` 版 `crates/nai-storage/src/store.rs:694` 的注释把缝写得更直白：

> `memory_records` carries **no revocation column**, so a caller cannot honestly derive the
> `revoked` input `read_records` needs

### 2.2 缝型归属

与 W103（H15）、W104（H02）、W105（H18）、W106（H01）、W107（H03）**完全同型**，
是「判定面存在、执行体不存在」在本仓连续**第六轮**现身。

与前五轮**不同的一点**：**前五轮执行体已存在**（决策者只需接上），本轮**连判定的输入都不存在**
⇒ 必须先**造一个承载物**，才谈得上接线。这是本轮与 W107 那个「零迁移」轮的**实质差别**：
本轮**必须**付一次迁移。

## 三、设计（详见 [ADR 0087](../adr/0087-memory-read-surface.md)）

八条决策，摘要：

| # | 决策 | 判据 |
|---|---|---|
| 1 | 承载物是**新增迁移** `0008`，**不编辑 `0001`** | `verify()` 校验每个迁移文件的 SHA-256，编辑 0001 会让所有既有库失效 |
| 2 | 三条派生**互不顶替**，各取自一个事实 | 写死任一项＝注入**判定**而非注入**事实**（W94 形状） |
| 3 | 取数**不预筛**作用域 | 否则 `out_of_scope` **失去对象**（假阴性，最危险方向） |
| 4 | 判定**一次** | 存储层只取数与阻断；请求级与条目级分开 |
| 5 | 阻断**只置一次**、不覆盖 | 撤销时刻是一条事实，重写它等于篡改 |
| 6 | **先**阻断、**再**判陈述；`read_side_blocked` 不是调用方字段 | 否则调用方可构造恒真 `ForgetOutcome`，判定变装饰 |
| 7 | 非空洞守卫**双向**钉 | 穷尽分类 + 端到端实测集合，两头都钉 |
| 8 | H03/H04 `status` **仍 `pending`** | 交付判定面→执行体 ≠ 验收达标 |

关键的一处**反向**设计：`permitted` **刻意不查 `permission_grants`**——取数入口已要求同一条
`session.manage`，条目级再查一次则二者**恒一致** ⇒ `PermissionDenied` 永不可达
＝**守卫还在却无对象可守**（本项目明令禁止的形状）。可达性由「`admit_write` 不要求观众记录带
授权引用 ⇒ 缺授权引用的观众记录可入库但读不出来」保证，并有用例专门钉它。

## 四、实现

- `migrations/sqlite/0008-memory-revocation.sql`：
  `ALTER TABLE memory_records ADD COLUMN revoked_at_ms INTEGER;`（可空，NULL = 未撤销）
- 版本号**五**处同改：`migration::VERSION` 7 → **8**、`SQL: [&str; 8]`、
  `schemas/config/backup.schema.json` 的 `db_schema_version` const、Rust 契约样本断言、
  **Node 备份契约样本**；TS 生成物由 `cargo xtask schema` 再生
  ⇒ `cargo xtask schema --check` **实测 `no drift`**（W108 侦察实测本机可跑，
  本轮不再是 W51 那轮的「不可再生面」）
- `crates/nai-storage/src/store.rs`：`StoredMemoryRecord`／`memory_scoped`（**只按 `workspace_id`
  过滤**）／`block_memory_read`（只置一次、返回生效中的时刻）；既有 `memory_records` **一字未改**
- `crates/nai-runtime/src/memory.rs`：`MemoryReadError`／`MemoryForgetError`／`memory_permitted`／
  `Composition::read_memory`／`Composition::forget_memory`
- `crates/nai-runtime/src/lib.rs`：新增两个错误类型的导出

## 五、测试与非空洞守卫

`crates/nai-runtime/tests/memory.rs` 由 **9** 项增至 **18** 项（新增 9 项）：

| 用例 | 钉的是什么 |
|---|---|
| `a_record_outside_the_scope_is_refused_by_name_and_not_filtered_away` | 越作用域**被具名拒绝**（不是被上游筛掉） |
| `an_expired_record_is_refused_by_the_read_face_not_by_the_write_gate` | 时效判在**读取**面（§42.2 无此规则 ⇒ 它确实落了库） |
| `blocking_a_record_makes_the_next_read_refuse_it_as_revoked` | 阻断后读 ⇒ `Revoked`；且**行还在**（阻断 ≠ 清除） |
| `a_second_block_does_not_rewrite_the_revocation_moment` | 重复阻断 ⇒ 时刻**不变** |
| `a_viewer_record_without_an_authorization_ref_is_refused_on_the_read_face` | `PermissionDenied` **可达** |
| `a_budget_crop_is_visible_by_name` | 超配额裁剪**具名** |
| `when_one_record_violates_every_layer_only_the_first_one_is_reported` | **顺序断言**：四层同时违反 ⇒ 报 `OutOfScope` 且**只报一次** |
| `every_rejection_is_either_constructed_here_or_named_as_constructed_elsewhere` | 穷尽分类闭世界 |
| `the_read_and_forget_face_is_reachable_end_to_end` | 实测集合 == `READ_FORGET_FACE`（13 条） |

非空洞守卫**双向**钉：

- `cover(rejection)` 对 `MemoryRejection`（23 变体）**穷尽 `match`** —— 每个变体要么归
  `Reached`（本文件真的构造了它），要么归 `Elsewhere(why)`（`why` **MUST** 写出承载它的是谁）。
  kernel 新增变体而此处未决定归属 ⇒ **编译失败**，不静默丢覆盖。
  闭世界：`NOT_CONSTRUCTED_HERE` **恰为 3 条**——`UnknownOperation`（§42.6 操作面）、
  `ApproximateSubstitution`（结构上不存在顶替条目）、`ReadSideNotBlocked`（本入口先阻断后判定）。
- 反向一钉：端到端用例断言**实测观察到**的集合与 `READ_FORGET_FACE` **逐项相等**
  ⇒ 「分类说可达」与「真的产生得出来」是两件事，两头都钉。

`crates/nai-storage/src/migration.rs` 增 `version_eight_upgrade_...`：在一个**已有行**的 v7 库上
跑升级（空表上的 `ALTER TABLE` 什么也证明不了），断言九列**逐字保留** + `viewer_scope` 的 NULL
保持 NULL + 撤销列默认 **NULL**（MUST NOT 是 0——0 会被 `is_some()` 读成「1970 年就已撤销」，
把**每一行老记忆**一次性判死）。

⚠️ **顺带修一处 VERSION 7→8 带出的回归**：既有 `version_six_upgrade_...` 用的是
`SQL.iter().take(VERSION - 1)`，版本号一升就**自动前移**——VERSION 7→8 后它实际改测了 v7→v8，
**用例名变成不实陈述**，而 0007 那条升级路径**失去覆盖且零信号**。已钉住
`const ASSEMBLY_KIND: usize = 7`（MUST NOT 用 `VERSION - 1`）。

## 六、非空洞注入验证

**3** 组真改文件 + 真跑 + `sha256` **逐字节**还原，共 **5 红**，每组 ≥1：

| 组 | 注入 | 结果 |
|---|---|---|
| I1 | 派生写死 `revoked = false` | **2 红**（`blocking_a_record_...`、`...end_to_end`） |
| I2 | 取数改回按 session 预筛 | **2 红**（`a_record_outside_the_scope_...`、`...end_to_end`；实测集合里 `OutOfScope` **整个消失**） |
| I3 | 阻断改为覆盖写 | **1 红**（`a_second_block_...`，差值 **22 ms**） |

I2 的信号最能说明问题：守卫**还在**，但它守的对象已被上游拿走 ⇒ 实测集合里 `OutOfScope`
**整个消失**（不是「少一条」，是**一条都没有**）。

⚠️ **I3 的 20 ms 等待不是装饰**：若两次阻断落在同一毫秒，「覆盖写」的注入会与原值**碰巧相等**
而绿着通过（**假阴性**）。差值 22 ms 正是这条等待让注入**可见**的证据。

⚠️ **本轮方法论产出（MUST 记住的坑）**：注入 **MUST** 用编辑工具做，**MUST NOT** 用 Python
读写往返——Python 会把整文件换行符**统一**（原本「编辑工具新增的 LF 行」与「既有 CRLF 行」**混排**），
`sha256` 还原校验随之**失效**（I2 第一次只能做内容级核验，已改用编辑工具重做一遍）。

## 七、门控与 validate

### 7.1 全量门控

`cargo xtask check --profile full` ⇒ **`EXIT=0`**。计数**现测**（口径 `grep -c '^test result:'`）：

- Rust `^test result:` **95**（`Running` **78** 含 1 行伪目标 + `Doc-tests` **18**）
  / **924 passed / 0 failed**
- Node TAP 根 **13** / `# tests` **319** / `# fail` **0**
- 日志内 `error[`／`LNK1104`／`out of memory`／`DeadlineExceeded` 命中 **0 / 0 / 0 / 0**
- `Running` 自洽：**78 − 1 + 18 = 95**

**对照 W108**（95 目标 / 914 passed、13 / 319）⇒ Node **逐项相同**、Rust 目标数**相同**、
`passed` **914 → 924 = +10**，恰为 `crates/nai-runtime/tests/memory.rs` **+9** 与
`crates/nai-storage/src/migration.rs` **+1**（均落在既有测试目标内 ⇒ **无需** `engineering_checks` 登记）
⇒ **计数变化逐项对得上**，不是「口径变了」。

### 7.2 归档前的非一次即绿（**3 次**；归档后另有 1 次复跑，见 7.3）

| # | 现象 | 判据 | 处置 |
|---|---|---|---|
| 1 | Node `# fail 1`：`backup schema requires bounded hashes/versions ...` | **真回归**：`backup.schema.json` 的 const 升到 8，而 Node 备份样本仍是 `7` | 改样本为 `8`，并把「钉上一个版本必须被拒」从 `6` 改为 **`7`** —— **不改代码语义** |
| 2 | Rust `prepare_without_profile_is_rejected` 红在 `DeadlineExceeded` | **宿主瞬态**：与 W107 记录的**同一个用例、同一症状**；相关文件本轮一行未改 | **限并发**（`RUST_TEST_THREADS=1`）重跑 ⇒ 全绿，**未改一行代码** |
| 3 | （编译期）间歇 `LNK1104` | **逐目标能过** ⇒ 非代码缺陷 | **重试**，绝不改代码 |

⚠️ **第 1 条是本轮真收获**：版本号成套是**五**处，不是四处——
`migration::VERSION` / `SQL` 数组 / `backup.schema.json` const / Rust 契约样本断言 /
**Node 备份契约样本**。W51 的教训（漏一处）在本轮以**第五处**的形式复现，被门控抓出来了。

### 7.3 OpenSpec validate

变更级 `openspec validate 2026-10-15-w109-memory-read-surface --strict` ⇒ **`is valid`**。

归档前（**现测**）：`--specs --strict` **43 passed / 0 failed**、`--all --strict` **44 / 0**、
`--changes` **1 / 0** —— 与 W108 **逐项相同**。

归档动作：`runtime-composition` ⇒ **`+2 added` / `~1 modified` / `-0` / `→0`**（活规格已就地更新）。

归档后（**立刻**跑，未勾 tasks 只在这里报）：`--archived` **107 passed / 0 failed**
（W108 为 **106** ⇒ **+1**，恰为本轮这一个变更）、`--all --strict` **43 / 0**。

**归档后复跑门控**（`openspec/specs/**` 已被归档改动）⇒ **`EXIT=0`**，计数与归档前**逐项相同**
（95 / 78 / 18 / 924 / 0；Node 13 / 319 / 0；`error[`／`LNK1104`／`out of memory`／`DeadlineExceeded`
均 **0**）。

## 八、CI（**回填**）

待推送后回填：run id、`head_sha` 与推送 SHA 的核对、双平台矩阵结论。

## 九、如实披露、遗留与边界

### 9.1 如实披露

- **§42.4 只到「阻断」，未到「清除」**：一条被遗忘的记录**仍在 `memory_records` 里**，
  只是读不出来。清除副本需要副本清单与清理执行体，是**另一条工作**。
- **两条拒因经本入口结构上不可达**：`ApproximateSubstitution`（存储里不存在顶替条目）
  与 `ReadSideNotBlocked`（本入口先阻断后判定）。二者都在守卫的闭世界里**具名登记**，
  其覆盖在内核直连的用例里——**MUST NOT** 读作「已覆盖」。
- **`permitted` 不查 `permission_grants`**：是**设计**（否则 `PermissionDenied` 永不可达），
  不是遗漏。
- **`source`／`claim`／`source_ref`／`written_at_utc_ms`／`publish_ref` 仍不落库**
  （承 W107）⇒ 「这条记录当时是从哪种来源写进来的」在库里**查不到**。

### 9.2 遗留

- 控制面接线（读取面与遗忘落点**还没有**控制面调用方）
- §42.4 的清除副本一侧
- Pro 跨会话（§42.5：授权/审批/租约/设备身份不跨场继承）

### 9.3 边界（本轮**不做**的事）

- 不动 `read_records` / `forget` / `admit_write` 的**任何一行**判定规则
- 不动既有 `Store::memory_records` 的会话过滤
- 不新增权限名（复用 `session.manage`）
- 不把 `revoked_at_ms` 加进 `MemoryRecord` 契约（它是存储层的门控状态，不是记录内容）
- **不改** H03/H04 的 registry `status`（仍 `pending`）
