# W141：utterance 取值域——一处声明、白名单化可用性、幽灵值具名不猜测

- 日期：2026-10-18
- 来源：W139 第 5 个结论与 N1（`docs/reports/2026-10-18-w139-causal-chain-appraisal.md`）；
  W140 的 N2 把这一项明确留给了本轮
- 交付：`crates/nai-storage/src/utterance_status.rs`（新）、三个生产源文件的 SQL 改为派生、
  `crates/nai-storage/src/tests/status_domain.rs`（新）、ADR 0115、
  `openspec/changes/w141-utterance-status-domain/`

---

## 一、开工前的勘定（逐条现测，复核 W139）

W139 给的计数本轮**全部复核吻合**（MUST 现测，MUST NOT 照抄）：

| 集合 / 写入 | 处数 | 出处（现测） |
| --- | --- | --- |
| `NOT IN(11,12,13,14,15)` 级联保护 | 6 | `store.rs` 149 / 264 / 450 / 527、`knowledge.rs` 483、`publication.rs` 446 |
| `NOT IN(12,13,14,15)` 可用性 | 3 | `store.rs` 955 / 1027 / 1122 |
| `BETWEEN 1 AND 5` 待批准带 | 3 | `store.rs` 604 / 1060 / 1164 |
| `SET status=N` 写入 | 9 | `1`（INSERT，629）/ `6`（approval，987）/ `12`（×6 级联）/ `14`（reject，1164） |

**本轮新测出 W139 未列的 3 处**：撤回审批的语句里也在读 `status=12` ——
`knowledge.rs:493`、`publication.rs:447`、`store.rs:559`（`candidate_hash IN(SELECT text_hash FROM utterances … status=12)`）。
这 3 处同样是裸字面量，一并派生。

该列在 `migrations/sqlite/0001-foundation.sql` 里是 `status INTEGER NOT NULL DEFAULT 1`：
**没有 CHECK 约束、没有枚举、没有常量**。所以上面那些数字就是它唯一的「定义」。

**取值域实测**：有写入者的只有 `{1, 6, 12, 14}`；`11`、`13`、`15`、`2..=5`、`7..=10` **零写入者**
（幽灵值）。其中 `11` 是唯一的分歧点：在级联保护集里（不被改写），却不在可用性黑名单里（被当成可用）。

**判定**：本轮**不裁定 11 是什么** —— 裁定需要它的写入者先存在。要修的是另外两件事：
① 没有一处声明；② 可用性是黑名单（把未声明的取值默认判成可用，而可用是危险方向）。

---

## 二、落了什么

### 2.1 `crates/nai-storage/src/utterance_status.rs`（新，零新增依赖）

- **四个有写入者的具名取值**：`STATUS_PENDING=1` / `STATUS_APPROVED=6` /
  `STATUS_INVALIDATED=12` / `STATUS_REJECTED=14`。
- **三条带**：待批准带 `1..=5`、不可用带 `12..=15`、级联保护带 `11..=15`
  （保护带比不可用带多一个下界值 **11** —— 那正是分歧点）。
- **`StatusClass` 五值闭世界**：`Pending` / `Approved` / `Unusable` / `ProtectedOnly` / `Unassigned`；
  `classify()` 对任意 `i64` 都返回其中一格，`else` 落到 `Unassigned`
  ⇒ 「不知道」有**可表示的取值**，调用方不必在「撒谎」与「不调用」之间选。
- **`usable()` 是白名单**：只认 `Pending | Approved`，而不是「不在黑名单里」。
- **派生链**：`usable()` → `usable_values()` → `usable_guard()`（进 SQL）；
  `cascade_guard()` 由保护带常量派生。

### 2.2 生产源码的裸字面量清零

`store.rs` / `knowledge.rs` / `publication.rs` 里 6 + 3 + 3 + 9 + 3 处 status 数字全部改为 `{}`
占位、由上述常量填充。**可用性那 3 处由黑名单 `NOT IN(12,13,14,15)` 改为白名单 `IN(1,2,3,4,5,6)`。**

### 2.3 一处真实的行为变化（且只有这一处）

`11`、`7..=10`、`16..`、`0`、负数 由「可用」变「不可用」。
已写入的取值（`1`/`6`/`12`/`14`）判定**完全不变** ⇒ 既有用例全绿（见 §六）。

---

## 三、判据（新增 8 条）

`crates/nai-storage/src/tests/status_domain.rs`（5 条）：

| 判据 | 管什么 |
| --- | --- |
| `the_literal_scanner_actually_fires_and_actually_forgives` | 扫描器自身的非空洞性：先用内联样本证明它**抓得到**、也**放得过**，否则「零命中」与「扫描器坏了」不可区分 |
| `no_bare_status_literal_survives_in_the_production_sources` | 生产源码里零裸字面量（**会过期**：谁新写一句带数字的 SQL 都变红）；扫描集含 `utterance_status.rs` 自己 —— 守卫不给自己开后门 |
| `the_sql_guard_and_the_rust_predicate_agree_value_by_value` | SQL 守卫与 Rust 谓词逐值一致（管「以后有人绕开派生链另写一份」） |
| `a_candidate_whose_status_was_never_declared_is_not_approvable` | 走**真实 `Store::approve`**：未声明的取值必须拒绝；带对照组（待批准/已批准必须仍可批准） |
| `the_cascade_guard_protects_exactly_the_band_it_names` | 走**真实 `store::recover`**：保护带内不被改写、带外一律改写成已失效 |

`utterance_status.rs` 模块内 3 条：闭世界（每一格都被某个取值命中，反方向也查，含 `i64` 两端）、
白名单、两带一致且不相交（编译期 `const { assert!(..) }` 钉住包含关系）。

**扫描集不含判据自己**：`tests.rs:483` 与 `approval_workflow.rs:202` 断言里钉的 `12` / `14`
**故意**留裸字面量 —— 那是**判别性**断言，改成引用常量就变成「常量比常量」，恒绿。

---

## 四、非空洞注入验证（4 组，全部真红）

| 组 | 注入 | 结果 |
| --- | --- | --- |
| 1 | `usable()` 退回黑名单写法（`!matches!(classify, Unusable)`） | **4 条红**（首跑只红 2 条 —— 见下方盲区） |
| 2 | `PROTECTED_BAND_LO` 由 11 改 12（保护带缩一格） | 2 条红（行为 + 闭世界的反方向断言） |
| 3 | 在 `store.rs` 里新写一句 `SET status=12 WHERE status=13` | 扫描判据红，消息可归因到文件 |
| 4 | `usable_guard()` 绕开派生链硬编码 `"1,2,3,4,5,6,7"` | 3 条红，消息可归因到 `status=7` |

**首跑暴露的一处盲区（如实记录）**：注入 1 第一次跑时，行为判据
`a_candidate_whose_status_was_never_declared_is_not_approvable` **是绿的**。
成因是 `usable_values()` 当时扫的区间是 `[1, 已批准]`（`1..=6`）——
`usable()` 退化成黑名单后，`1..=6` 两种写法结果相同 ⇒ guard 一字不变 ⇒ 行为不变 ⇒ 绿。
即「一致性样本 ≠ 判别性样本」：`usable()` 的语义被窄区间**掩盖**了。
处置是**修派生链而不是记一笔**：`usable_values()` 改为扫全域 `[0, 不可用带上界]`，
同一注入复测 ⇒ 4 条红。

还原后 `sha256sum -c` **3/3 OK**（`utterance_status.rs` / `store.rs` / `status_domain.rs` 逐字节一致）。
注入 1 在 **fmt-clean 的最终字节**上复测过一次（4 条红），避免「注入结论只在过渡态成立」。

---

## 五、诚实边界（MUST NOT 越线）

- **11 的语义仍未裁定**。它现在是 `ProtectedOnly`，并具名登记为「语义未定、本仓零写入者」。
  本轮**没有**写、也 MUST NOT 写「11 表示已交付/正在播」之类的话。
- **数据库层面仍无 CHECK 约束**。本轮只锁住了**代码**这一侧；给列加约束需要迁移，属独立一轮。
- `tests/registry.yaml` 88 条里**没有**对应「status 取值域」的验收条目 ⇒ 本轮**不**标任何条目为 `tested`。
- 可用性判据变严 ⇒ 若将来有代码真的写入 `7..=10` 或 `11`，它会静默变得不可用。
  这是有意的（安全方向），但新写入者 MUST 同步更新取值域声明
  （扫描判据会拦住裸字面量，但拦不住「用了常量却没想清楚语义」）。

---

## 六、门控（本机）

`cargo xtask check --profile full`（`CARGO_INCREMENTAL=0` + `CARGO_BUILD_JOBS=1`）⇒ **`GATE_EXIT=0`**。

| 步骤 | 结果 |
| --- | --- |
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean（零告警） |
| `cargo test --workspace --locked --exclude nai-xtask` | 85 个 `Running` 目标 + 18 个 `Doc-tests`，102 条 `test result:` 汇总，**1141 passed / 0 failed** |
| `cargo test --locked --package nai-xtask --lib` | 通过 |
| JS 判据套件 | 14 套：57 / 30 / 22 / 14 / 18 / 24 / 34 / 22 / 16 / 16 / 15 / 12 / 11 / 5，全绿 |
| `node scripts/frontend-gate.mjs` | 「全部步骤通过（工程基础，非渲染/人工 UI 验收）」；桌面 `test:ui` 71/71 |

**总数对得上**：W140 的门控是 1133 passed，本轮 1141 ⇒ **正好 +8**，与新增判据数一致
（既有用例零回归的算术证据）。新增 8 条在门控日志里逐条 `ok`。

**中途的间歇 `LNK1104`（非代码缺陷，如实记录）**：预热首次在 `nai-runtime` 的 `observation`
测试二进制上命中 `LNK1104`（`无法打开文件 observation-….exe`）。判据按既有纪律取「**逐目标能过**」：
`nai-storage` 及其依赖编译全部成功，`cargo test -p nai-storage` EXIT=0 ⇒ 非本轮代码回归。
处置是**限并发 + 串行预构建 + 先把测试二进制「只链接不运行」预热到 EXIT=0**
（`target/debug/deps` 447 个 exe），再跑门控即一次绿。**未**改代码、**未**放宽判据。

**Clippy 拦下的三处（已修，未放宽）**：`assertions_on_constants`（两行常量比较 ⇒ 改为编译期
`const { assert!(..) }`，语义更强：改常量破坏包含关系 ⇒ 编译不过）、`while_let_loop`（扫描循环）。

## 七、CI 观测

推送 **`7d9839b`**，运行 **`39092207440`**，回读 `head_sha` = `7d9839b2be2a7457b0b3efd0f3bc570f4589ed7a`
与推送 SHA **完整 40 位一致**（三件齐：`cb=` 纳秒唯一参数 + `Cache-Control: no-cache` + 回读 `head_sha`）。

| 平台 | 结论 | 步数（API 口径） |
| --- | --- | --- |
| `foundation (ubuntu-24.04)` | `completed / success` | 20 |
| `foundation (windows-latest)` | `completed / success` | 20 |

逐平台看完**整条步骤序列**。非 success 项共 3 条，全部是 `skipped` 且逐条核到原因：
步 11「Publish the gate failure summary as annotations」在门控通过时不执行（两平台同）；
步 8「Install Linux C++ protobuf runtime」与步 14「Full three-language contract suite
(Linux C++ toolchain)」只在 Linux 跑（Windows 侧 skipped）。
步号从 16 跳到 30 是 `if` 未触发的步**不生成记录**（既有口径，MUST NOT 拿 `jobs[].steps`
条数与 workflow 的 `- name:` 步数相减）。

---

## 八、具名登记（本轮发现但不做）

| 编号 | 事项 | 归宿 |
| --- | --- | --- |
| N1 | 数据库层面 `utterances.status` 无 CHECK 约束 ⇒ 直连数据库仍可写入任意整数 | 需迁移（`migration::VERSION` 升版 + 六处成套），属独立一轮 |
| N2 | 幽灵值 `13` / `15` 与 `2..=5` / `7..=10` 同样零写入者，语义未定 | 与 11 同型；等各自的写入者出现时再命名 |
| N3 | `classify()` / `usable()` 的消费者今天只有 `usable_values()` 一条链；跨 crate 的判定面未开 | 没有调用方就不开接口（接缝不是免费的） |
| N4 | `status=6`（已批准）在判据里没有一条字面断言（只有 `12` / `14` 有） | 判别性断言的补缺，属独立一轮 |
| N5 | `target-check/`（12G，10-15 遗留 scratch）仍未清理，未被 `.gitignore` 覆盖 | 沿用 W140 N6：改用显式 `git add -- <path>` 规避；是否写进 `.gitignore` 留给后续轮次裁定 |
| N6 | `openspec archive` 提示「Why section should not exceed 1000 characters」（本轮 2171、W140 1953） | **不是本轮引入**：W140 同样超限。它是 archive 的软提示，**不影响**任何 validate（`--all --strict` 43/0、`--archived` 138/0 全过），CI 跑的也是 validate。按「MUST NOT 缩句过线」，把 Why 拆短会丢掉现测依据表；该表在报告 §一已有承载 ⇒ 处置留给后续轮次统一裁定 |
