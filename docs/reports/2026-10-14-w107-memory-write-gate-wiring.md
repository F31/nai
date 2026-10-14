# W107 报告：§42.2 记忆写入判定接上 runtime 与真实存储（H03 runtime 接线）

- 日期：2026-10-14
- 轮次：W107
- 验收：**H03**（记忆写入门槛）。**`status` 保持 `pending`**——本轮交付的是 **runtime 接线**，不是验收达标。
- 规格：`openspec/changes/2026-10-14-w107-memory-write-gate-wiring/`
- ADR：[0085](../adr/0085-memory-write-gate-wiring.md)
- 前驱：W106（H01）、W105（H18）、W104（H02）、W103（H15）

## 一、目标

把 §42.2 的记忆写入判定**接上真实记忆存储**：判定放行 ⇒ 记录落进 `memory_records`；
判定拒绝 ⇒ **具名**且**一列不落库**。

## 二、缝在哪

### 2.1 直接证据（**现测**，不是推断）

```
$ grep -rn '\badmit_write\b' --include=*.rs crates apps | grep -v /tests/ | grep -v "src/evaluate/memory.rs"
（空）
$ grep -rn '\bread_records\b' --include=*.rs crates apps | grep -v /tests/ | grep -v "src/evaluate/memory.rs"
（空）
$ grep -rn 'MemoryCandidate\|MemoryPolicy' --include=*.rs crates apps | grep -v /tests/ | grep -v "src/evaluate/memory.rs"
（空）
$ grep -rn 'memory_records' --include=*.rs crates apps | grep -v /tests/
crates/nai-storage/src/tests.rs:375
crates/nai-storage/src/tests.rs:432
```

- 前三条**零命中**：`admit_write`、`read_records`、`MemoryCandidate`、`MemoryPolicy`
  在**生产代码里零调用方**。
- 第四条命中的两处**都在测试里**，且是**目录级**断言：`tests.rs:375` 把 `memory_records` 列进
  「表都在」的清单，`tests.rs:432` 断言新建库这张表 `COUNT(*) == 0`。
  ⇒ **全仓没有任何一处生产读写路径碰过 `memory_records`。**

### 2.2 于是有一条今天**无法判**的命题

> 一次被**判定拒绝**的记忆写入，究竟有没有落库？

判定面不落库（`admit_write` 只返回 `Result<(), MemoryRejection>`），存储面不认识判定；
没有任何生产路径把两者接起来 ⇒ 这个问题在 runtime 上**不可表达**，只能靠读源码回答。

### 2.3 缝型归属

这是「**判定面存在、执行体不存在**」在本仓连续**第五轮**现身
（W103/H15 → W104/H02 → W105/H18 → W106/H01 → **W107/H03**）。

**与前四轮不同的一点**：前四轮的执行体都是**只追加哈希链**；本轮是**领域表**。
这带来一个前四轮不存在的选择面——执行体到底是「落链」还是「落库」——见 §三 决策 1。

### 2.4 为什么 H03 卡不卡外部前置的判定答案是「不卡」

W57 的 registry notes（H03）与 W89 的订正都已写明「判定面自足 ≠ 运行时接线」，
并明说 `admit_write` **在生产代码里无调用方**。而 `admit_write` 的输入**全部来自调用方**：

| 判定输入 | 来源 |
|---|---|
| `candidate.record`（作用域、kind、purpose、content、expires、authorization_ref） | 调用方给的候选 |
| `candidate.source` / `claim` / `source_ref` / `written_at_utc_ms` / `publish_ref` | 调用方给的候选 |
| `policy` | 调用方给的策略 |

⇒ **判定面不消费任何外部系统**（不喂模型、不查外部真值），所以「判定之后那一步」在仓库内**可判**。
这与 W105/W106 用的判据是**同一条**：**判定面的输入不来自外部 ⇒ 「判定之后那一步」在仓库内可判。**

## 三、设计（详见 [ADR 0085](../adr/0085-memory-write-gate-wiring.md)）

### 决策 1：执行体落在 `memory_records` 表，**不是**决策日志

§42.2 的拒因后果原文是「MUST NOT **落库**」，指向领域表。且记忆**观测**落链**已经**
由 W103 的 `record_observation` 承担 ⇒ 本轮若也写一条决策日志条目，就是**同一件事的第二处定义**，
还需要一个新的 `DecisionKind` 变体（⇒ 一次迁移）。

本报告为此专门加了一条**否定式断言**（见 §六 的
`a_memory_write_reaches_the_memory_store_and_not_the_decision_log`）。

### 决策 2：判定与落库是**一处**入口

`Composition::write_memory(candidate, policy) -> Result<MemoryWriteOutcome>`。拆成两个出口会把
「不许把未放行的写入落库」的责任推给每个调用方。理由与 W103–W106 一致。

### 决策 3：`MemoryWriteOutcome` 恰**两态**（`Written` / `Refused`）

H03 只有**一项**判定 ⇒ 只有一个「放行 / 不放行」的轴。W106 的四态是因为 H01 有**两项**判定。
多造一态会是**没有对应事实的空壳**（W92 教训的反向应用：分类在此**恰够**）。

### 决策 4：判定**一次**，存储层不重跑，也不代答开关

`Store::record_memory` 只做存储层自己的事（`id()` 校验、`access()` 查权限、写九列、
主键冲突映射成具名 `Conflict`）。MUST NOT 重跑 §42.2（一条开关答两次会分家，W88）；
观众记忆开关的写侧**调用**判定结果，**不复写** `if !policy.viewer_memory_enabled`（ADR 0067）。

### 决策 5：九列**逐字**取自被判定审过的输入，多一列都不造

`MemoryRecord` 九字段 ↔ `memory_records` 九列逐字对应（`viewer_scoped_id` → `viewer_scope`、
`expires_at_utc_ms` → `expires_at_ms`）⇒ **零迁移**。
判定还审了 `source` / `claim` / `source_ref` / `written_at_utc_ms` / `publish_ref`，
但 §42.2 把它们定成**判定输入**而非持久化字段，**表里没有对应列** ⇒ **不落库**（如实登记，见 §十）。

### 决策 6：存储层**不**加写入面的时效规则

`record_candidate` 会在存储层拒掉已过期候选；`record_memory` **不**做。§42.3 把时效放在**读取**面。
在存储层加一条判定面从未声明的规则，会让「判定面声明了什么」与「存储层实际做了什么」变成两个口径。
——这条**被测试钉住**：`the_read_back_applies_no_read_judgement` 写一条已过期记录（§42.2 无时效规则
⇒ 判定放行 ⇒ 落库），并断言读回**仍然**返回它。

### 决策 7：写入面权限名复用 `session.manage`（**选择**，如实披露）

不新增 `memory.write`：本轮补的是「判定 → 执行体」的接线，新增权限名要连带改
`Composition::bootstrap_owner` 的授权表与测试夹具 `grant()`，那是**与判定无关**的授权面扩张。

⚠️ 这是**选择**，不是「本来如此」：`memory.access@1` 的四项操作里确有 `write`
（§42.6），一个对齐命名的 `memory.write` 权限**也站得住**；本轮刻意不扩张授权面。

### 决策 8：H04 读取面**不在本轮** —— 具名开放问题

`read_records` 的条目级门控输入是 `ScopedRecord` 的 `revoked` / `permitted` / `approximate_of`：

| 门控输入 | 能否从 `memory_records` 诚实派生 |
|---|---|
| `permitted` | **能**（`permission_grants` 是真实存储事实） |
| `approximate_of` | **能**（恒 `None`，表中无此概念） |
| **`revoked`** | **不能**——`memory_records` **没有任何撤销列** |

§42.4 的遗忘链是一条**真实的**状态（`ReadSideNotBlocked` / `ErasedClaimWithoutCompletePurge` /
`ErasedClaimWithThirdPartyRetention` / `ThirdPartyRetentionUndisclosed` 四条拒因都在描述它）
⇒ 在读取面写死 `revoked: false` 就是**凭空断言一条判定输入**，即本项目明令禁止的
「注入**判定**而非注入**事实**」（W94 的形状）。

⇒ **H04 的 runtime 接线留给新的一轮**，并作为具名开放问题登记在 `tests/registry.yaml`（H04 notes）。
配套：`Store::memory_records` 是**取数入口**（逐列还原 + 按作用域过滤），
MUST NOT 施加 §42.3 的任何条目级判定，**MUST NOT 被读作 H04 已接线**。

### 决策 9：非空洞守卫覆盖写入面**八条**，并**穷尽 `match`**

`refusal_sample(rejection)` 对 `MemoryRejection`（23 变体）**穷尽 `match`**：写入面八条各给样本，
其余十五条显式 `return None`。kernel 新增变体而此处未决定归属 ⇒ **编译失败**。

## 四、零迁移是怎么拿到的

| 代价项 | 本轮 |
|---|---|
| `DecisionKind` 新变体 | **不新增**（执行体不是决策日志，决策 1） |
| `migrations/**` | **不改**（`memory_records` 在 `0001-foundation.sql:29` 已存在且九列齐备） |
| `migration::VERSION` | **保持 7** |
| `schemas/**` / 生成物 / `db_schema_version` | **不改** |
| 新增依赖 | **零**（kernel / storage / contracts 均已是 runtime 的既有依赖） |
| 权限面 | **零改动**（复用 `session.manage`，决策 7） |

⇒ 本轮**不产生**任何只能在本机 `EBUSY` 路径（`cargo xtask schema --check`）上验证的改动面。
这正是本轮满足「**本机验不了的改动不满足『可自主推进』的定义**」这一条的方式。

## 五、实现

| 文件 | 改动 |
|---|---|
| `crates/nai-runtime/src/memory.rs` | **新增**：`MemoryWriteRefusal`、`MemoryWriteOutcome`（两态）、`Composition::write_memory` |
| `crates/nai-runtime/src/lib.rs` | 导出 `MemoryWriteOutcome` / `MemoryWriteRefusal` |
| `crates/nai-runtime/tests/memory.rs` | **新增**：9 项运行期用例 + 非空洞守卫 |
| `crates/nai-storage/src/store.rs` | **新增** `Store::record_memory`、`Store::memory_records`；`MemoryRecord` 加入既有 `use` |

**被审对象一行未改**：`admit_write`、`read_records` 及其它 kernel 判定规则逐字保持原样。

## 六、测试与非空洞守卫

`crates/nai-runtime/tests/memory.rs`，**9 项**：

| # | 用例 | 钉住的性质 |
|---|---|---|
| 1 | `an_admitted_write_lands_exactly_one_row` | 放行 ⇒ 恰一行，且写回的值 == 判定输入 |
| 2 | `a_refused_write_is_named_and_lands_nothing` | 拒绝 ⇒ 具名 + **零行** |
| 3 | `every_write_rejection_is_reachable_through_the_runtime` | **非空洞守卫**：可达的**恰为八条**（`WRITE_FACE`），且全部零落库 |
| 4 | `the_viewer_switch_closes_the_write_face_too` | 开关关闭 ⇒ 写侧被拒、零落库；开关打开 ⇒ **同一候选确实落库**（证明拒的是开关） |
| 5 | `the_stored_row_round_trips_column_by_column` | 九列**逐字往返**（含 `viewer_scoped_id` → `viewer_scope` 映射） |
| 6 | `a_memory_write_reaches_the_memory_store_and_not_the_decision_log` | 落点在**记忆表**，决策日志**零新增** |
| 7 | `a_duplicate_record_id_is_a_named_conflict_and_adds_no_second_row` | 主键冲突 ⇒ `Err(Conflict)`，不产生第二行 |
| 8 | `the_read_back_does_not_cross_scopes` | 读回**不跨会话** |
| 9 | `the_read_back_applies_no_read_judgement` | 读回**不施加** §42.3 判定（已过期记录仍被读回） |

`WRITE_FACE`（写入面八条，按 `MemoryRejection::ALL` 顺序）：
`untrusted_source_as_fact` / `unplayed_candidate_as_fact` / `write_without_scope` /
`write_without_source_ref` / `write_without_timestamp` / `fact_without_publish_ref` /
`fact_outside_semantic_memory` / `viewer_memory_disabled`。

## 七、非空洞注入验证

方法：**真改文件 + 真跑 + 备份拷回 + `sha256` 逐字节核对**。
基准 = **过完 `cargo fmt --all` 之后**的那一份：

- `crates/nai-runtime/src/memory.rs` — `c7c1272a3393746dc86b10bf0354754bc2c6deea4dfb615005b09390ca8deccf`
- `crates/nai-storage/src/store.rs` — `8a7358c4c07292326bfe240ea82f281d17c732c8c281fe6fc7301db20665acef`

| # | 注入（真改的那一处） | 期望被打红的性质 | 红条数 | 打红的用例 |
|---|---|---|---|---|
| I1 | `write_memory` **不跑** `admit_write`，直接落库 | 拒绝必须零落库 | **3** | `the_viewer_switch_closes_the_write_face_too`、`a_refused_write_is_named_and_lands_nothing`、`every_write_rejection_is_reachable_through_the_runtime` |
| I2 | 拒绝分支**照旧落库**（先落库、再返回 `Refused`） | 同上 | **3** | 同 I1 |
| I3 | `purpose` 列由 `content.len()` **重算** | 各列必须逐字取自判定输入 | **4** | `the_read_back_applies_no_read_judgement`、`the_viewer_switch_closes_the_write_face_too`、`the_stored_row_round_trips_column_by_column`、`an_admitted_write_lands_exactly_one_row` |
| I4 | `viewer_scoped_id` → `viewer_scope` 映射被常量 `""` 填空 | 九列逐字映射 | **2** | `the_stored_row_round_trips_column_by_column`、`the_viewer_switch_closes_the_write_face_too` |
| I5 | 读回入口丢掉会话过滤（`session_id=?2 OR 1=1`） | 读回不跨作用域 | **1** | `the_read_back_does_not_cross_scopes` |
| I6 | 读回入口**加上时效过滤**（变成 §42.3 形状） | 取数入口不判定 | **1** | `the_read_back_applies_no_read_judgement` |
| I7 | 写入面把能力开关**答成「恒开」**（第二个应答点） | 开关只有一个应答点 | **2** | `every_write_rejection_is_reachable_through_the_runtime`、`the_viewer_switch_closes_the_write_face_too` |

**合计 16 红**，每一组**至少 1 红**。

**还原**：备份拷回后 `cmp` 逐字节相同、`sha256` 与基准一致、复跑基线 **9 passed / 0 failed**；
注入临时目录 `.w107-inject` 已删除（**MUST NOT** 被 commit）。

### 7.1 本轮的方法论产出

- **I5 / I6 是本轮**新增的注入形状（前四轮的执行体是哈希链，没有「按作用域取数」这一步）：
  它们描述的是**取数入口**的两个失效方向——**过滤太松**（跨作用域）与**判定太多**（把读取面的
  规则搬到取数面）。**后者尤其重要**：一个「多加了一条检查」的实现看起来更安全，
  但它让取数入口偷偷变成第二个判定面，而**没有任何需求要求它那么做**。
- **I7 的形状**是「把能力开关答两次且答错」：它不改 `admit_write`（被审对象），
  只在**调用侧**换掉策略 ⇒ 判据仍能看见。这印证了 W106 的纪律：
  **判据要能看见「调用方把判定用错了」，而不只能看见「判定本身写错了」。**

## 八、门控与 validate

### 8.1 全量门控 `cargo xtask check --profile full`

**命令与环境**：`source /c/Users/32791/.nai-msvc-env.sh` + `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1`
（末次加 `RUST_TEST_THREADS=1`），日志 `.w107-gate.log`。

**最终 `EXIT=0`。计数（现测，非推断）**：

| 面 | 计数 |
| --- | --- |
| Rust `^test result:` 行 | **95** |
| Rust `Running` 行（含 1 行伪目标 `` `nai-xtask.exe check --profile full` ``） | **78** |
| Rust `Doc-tests` | **18** |
| Rust `passed` / `failed` | **914 / 0** |
| Node TAP 套件根 / `# tests` / `# pass` / `# fail` | **13 / 317 / 317 / 0** |
| `LNK1104` / `error[` / `out of memory` 命中 | **0 / 0 / 0** |

**自洽核验**：`78 − 1（伪目标）+ 18 = 95` == `^test result:` 行数 ✓（伪目标行
`` Running `E:/nai-build-target\debug\nai-xtask.exe check --profile full` `` **带反引号、无
`test result:` 行**，与 W106 记录的形状一致）。

**与 W106（94 目标 / 78→76 `Running` / 905 `passed`）逐项对比**：
**目标 +1、`passed` +9**，恰为新增测试目标 `crates/nai-runtime/tests/memory.rs` 的
**9 条用例** ⇒ **计数变化逐项对应得上**（不是「口径变了」）。Node 面与 W106 **逐项相同**
（`tests/registry.yaml` 本轮只改 H03/H04 的 `notes` 内容、不改结构 ⇒ 用例数不变），
这与「本轮未新增任何 `.mjs`」一致。

**⚠️ 本机「一次即绿」不成立：本轮共跑 4 次**：

| 次 | 红点 | 归类 |
| --- | --- | --- |
| ① | `LNK1104` 打不开 `nai_engine-7bf437cb65f567a6.exe` | 宿主瞬态 |
| ② | `LNK1104` 打不开 `pipeline-696bd1f0c1a07a03.exe` | 宿主瞬态（**换了一个目标**） |
| ③ | `nai-runtime --test composition` 2 条 `StorageError{DeadlineExceeded,"operation stopped"}` | 宿主瞬态 |
| ④ | — | **`EXIT=0`** |

①② 的处置＝**预链接**（`cargo test --workspace --locked --exclude nai-xtask --no-run` +
`cargo test -p nai-xtask --lib --no-run`，逐轮重试）⇒ **workspace 第 8 轮收敛**、
xtask 第 **1** 轮即过；**代码一字未改**。

**⚠️ ③ 有一个必须如实披露的新观察**：这一次**连单跑也红**——
`cargo test -p nai-runtime --test composition` **`EXIT=101`（10 passed / 2 failed）**、
`cargo test -p nai-runtime --locked` **`EXIT=101`（18 passed / 7 failed，倒在 `assembly`）**。
这与 W84／W106 记录的「**单跑即过**」形态**不同** ⇒ 那三轮用的判据链第一步
（「单目标复跑全绿」）**本轮不成立**，故**不能**照抄「单跑即过 ⇒ 非代码缺陷」的结论。

改用的判据（四条，全部实测）：

1. **错误类是同一族**：全部失败都是 `StorageError { code: DeadlineExceeded,
   reason: "operation stopped" }`，且**全部落在 bring-up / 夹具**（`tests/common/mod.rs:118`、
   `tests/composition.rs:139`、`tests/composition.rs:183`），**没有一条**落在业务断言上。
2. **失败集合每次不同**：门控那次是 `prepare_without_profile_is_rejected` +
   `telemetry_is_buffered_when_persistence_fails_while_critical_is_reported`；单跑那次是
   `prepare_without_profile_is_rejected` + `invalid_runtime_config_is_rejected_at_provision`；
   整 crate 那次是 `tests/assembly.rs` 里另外 **7** 条 ⇒ **跨测试文件、随负载而变、不绑定任何用例**。
3. **相关文件本轮一行未改**：`crates/nai-runtime/tests/composition.rs`、`tests/assembly.rs`、
   `tests/common/mod.rs` 的 `git status --porcelain` 均为**空**。
4. **同一份代码在限并发下 95 个目标全绿**：`RUST_TEST_THREADS=1` 跑门控 ⇒ `EXIT=0`、
   `failed=0`（含 composition 与 assembly）⇒ 代码未变、**只是并发度变了**。

⇒ 判为**宿主负载下的瞬态**（与本轮新增的测试目标叠加了并发压力），**非代码缺陷**；
处置＝**限并发复跑**，**未改任何代码**。

**⚠️ 与 W106 的同一披露一致且更强**：W106 已记「CI **不带** `RUST_TEST_THREADS=1` ⇒
CI 侧仍可能遇上同一瞬态」，并被 W106 的 CI run（双平台 `success`）回答。本轮**本机默认并发下
单跑也红**比 W106 更严重，故 CI 观测（见第九节）**不能省**。

### 8.2 OpenSpec validate

**归档前**，四条**全部 `EXIT=0`**（用 `openspec.js` 真实入口）：

| 命令 | 结果 |
| --- | --- |
| `validate 2026-10-14-w107-memory-write-gate-wiring --strict` | `Change '…' is valid`（EXIT=0） |
| `validate --specs --strict` | **43 passed, 0 failed**（43 items） |
| `validate --all --strict` | **44 passed, 0 failed**（44 items） |
| `validate --changes` | **1 passed, 0 failed**（1 item = 本轮变更） |

`--all` 的 **44** == `--specs` 的 **43** + **1** 条未归档 change（即本变更）⇒ **自洽**。

### 8.3 归档与 validate

`node <openspec.js> archive 2026-10-14-w107-memory-write-gate-wiring --yes` ⇒ **`EXIT=0`**：

```
Specs to update:
  runtime-composition: update
Applying changes to openspec/specs/runtime-composition/spec.md:
  + 2 added
Totals: + 2, ~ 0, - 0, → 0
```

**产物**（`openspec/changes/archive/2026-10-14-w107-memory-write-gate-wiring/`，共 **4** 个文件）：
`proposal.md` / `design.md` / `tasks.md` / `specs/runtime-composition/spec.md`；
原目录 `openspec/changes/2026-10-14-w107-memory-write-gate-wiring/` **已移走**；
活规格 `openspec/specs/runtime-composition/spec.md` ⇒ **+2 added**。

归档后**两条 validate**（**归档后 MUST 立刻跑**）：

| 命令 | 结果 |
| --- | --- |
| `validate --archived` | **105 passed, 0 failed**（105 items） |
| `validate --all --strict` | **43 passed, 0 failed**（43 items） |

- `--all --strict` 由归档前 **44** → 归档后 **43**（本变更已进 archive，不再是「未归档 change」）⇒ **自洽**；
- `--archived` 由 W106 归档后的 **104** → **105**（**+1**）⇒ 本变更已确实归档。

### 8.4 归档后复跑门控（`openspec/specs/**` 已被改动）

归档把两条 ADDED 需求并入活规格 ⇒ `openspec/specs/runtime-composition/spec.md` 被改
⇒ **复跑门控一次**（日志 `.w107-gate2.log`）：

**`EXIT=0`，计数与归档前（§8.1）逐项相同**：

| 面 | 归档前 §8.1 | 归档后 §8.4 |
| --- | --- | --- |
| Rust `^test result:` | 95 | **95** |
| Rust `Running` | 78 | **78** |
| Rust `Doc-tests` | 18 | **18** |
| Rust `passed` / `failed` | 914 / 0 | **914 / 0** |
| Node TAP 根 / `# tests` / `# fail` | 13 / 317 / 0 | **13 / 317 / 0** |
| `LNK1104` / `error[` | 0 / 0 | **0 / 0** |

⇒ 归档只改 `openspec/specs/**`（**不进 cargo／Node 的判据集合**）⇒ 计数**逐项不变**，
这同时是「归档没有误改代码」的旁证。

**⚠️ 本轮门控总账：共跑 5 次** —— ① ② 红在 `LNK1104`、③ 红在 `DeadlineExceeded`、
④ 绿（§8.1）、⑤ 归档后绿（本节）；另加一次 workspace 预链接循环（收敛于第 **8** 轮）。

## 九、CI（**回填**）

（本节由回填提交填写；回填提交**自身不回填**。）

## 十、遗留与边界

1. **H04 读取面未接线**（决策 8）——**具名开放问题**：「如何诚实派生 `revoked`」
   需要一个新的承载物（撤销列 / 遗忘链的落点），且必须随之付一次迁移。**本轮不假装已接**，
   登记在 `tests/registry.yaml`（H04 notes）。
2. **`MemoryCandidate` 的五个判定输入字段不落库**（决策 5）：`source` / `claim` / `source_ref` /
   `written_at_utc_ms` / `publish_ref`。这是**设计**不是遗漏——§42.2 把它们定为判定输入，
   `memory_records` 无对应列。⚠️ **但它有一个真实后果**：**「这条记录当时是从哪种来源写进来的」
   在库里查不到**（`kind`/`purpose`/`content`/`authorization_ref` 之外无来源痕迹）。
   若日后需要「按来源审计记忆写入」，那将是**另一个对象**（新增列或新增审计面），
   不是本轮可以顺手补的。
3. **权限名复用 `session.manage`**（决策 7）——是**选择**；若日后要按 `memory.access@1` 的
   操作名对齐权限，会是一次**授权面**改动，与判定无关。
4. **`expires_at_utc_ms` 与 `written_at_utc_ms` 不是同一个时间**（决策 6 / 表列映射）：
   前者落库（`expires_at_ms`），后者**只参与判定**、不落库。两者 MUST NOT 互相顶替。
5. **H03 仍 `pending`**：本轮闭合的是「判定面 → 执行体」，不是验收达标。
   真实记忆子系统的其余部分（控制面接线、§42.4 遗忘链的执行体、Pro 跨会话 §42.5）未建。
6. **`Store::memory_records` 的排序**是 `ORDER BY record_id`（确定性），**不是**插入序——
   表里没有创建时间列，故**读回顺序 ≠ 写入顺序**。若日后需要按写入序读回，
   同样是**另一个对象**（新增列）。
