# W104 轮次报告：H02 的 runtime 接线——判定面之后的**那一步**必须有执行体

- 日期：2026-10-14
- 变更：`openspec/changes/2026-10-14-w104-assembly-log-wiring/`
- ADR：[0082](../adr/0082-assembly-log-wiring.md)
- 新增模块：`crates/nai-runtime/src/assembly.rs`
- 新增测试：`crates/nai-runtime/tests/assembly.rs`（12 项）+ `migration::tests` 迁移升级用例（1 项）
- 新增迁移：`migrations/sqlite/0007-decision-log-assembly-kind.sql`
- 前置：W69（判定面）、W103（同型接线）、R1 自主项排序（用户指令「按顺序执行可自主推进任务」的续条）

## 一、目标

R1 的未达成验收项里，按「瓶颈是否在仓库外」拆出一组**可自主推进**项。W103 做了 **H15**，本轮做与之
**完全同型**的 **H02（装配可重建）**：H02 的 `notes` 自己写着「日志真实落盘（`nai-storage` 的只追加哈希链）
与运行时接线**未建**，本轮交付的是判定面」。

**本轮结论比预期大**：H02 与 H15 唯一的关键差别是**需要一个新种类**，而这个种类在本仓有**两个**闭世界
（Rust 枚举 + 存储表的 `CHECK`），第二个在动工前**没有**任何东西提示——直到测试首跑打出 `persisted: false`。

## 二、缝在哪

### 2.1 零调用方的直接证据

V3.11 §41.5 要求装配 MUST 落决策日志、且 MUST 可由**日志的片段引用**重建、MUST NOT 依赖当时内存。
W69 把判定落成了 `crates/nai-kernel/src/evaluate/assembly.rs::rebuild_assembly`，并在模块文档与
`docs/kernel.md` 里写死：

> 把装配记录追加进 `nai-storage` 的哈希链、以及执行 §47.2 的六类回归（能力包 **H16**），都是 runtime 与
> 证据生产侧的事。

这句话当时**诚实**。问题是它同时**不可核实**：

```
$ grep -rn 'rebuild_assembly\|AssemblyRecord\|LoggedFragment' crates/ apps/
crates/nai-kernel/tests/*.rs        ← 只有测试
（生产代码零命中）
```

⇒ 两条今天**无法被判**的命题，无论仓库里有多少测试：

1. 一次被判**可重建**的装配，会不会**照样**被写进链？
2. 真写进链的那份载荷，能不能**只靠链**重排出同一段片段序列——还是有人只存了指向当时内存的指针？

### 2.2 缝型归属

**判定面存在、执行体不存在**。这是本仓这条区分的**第七次**现身（W54 桌面壳 / W68 讲品循环 /
W103 观测接线之后）。「有判定面」是「有执行体」的必要条件，不是充分条件。

### 2.3 落点是推出来的，不是偏好

接线要同时看见**判定**（`nai-kernel`）与**只追加哈希链**（`nai-storage`）⇒ 只能是 `nai-runtime`
（两 crate 都已是它的依赖）。放 kernel ⇒ 引入 kernel→storage 越界依赖（`check-deps` 红）；
放 storage ⇒ 存储层认识 kernel 的领域类型。**零新增依赖**。

### 2.4 本轮**避开** H17（降级呈现接线）

H17 的落点 `CapabilityView` 是 **protobuf 生成类型**（`crates/nai-contracts/src/generated/nai.v1.rs`、
`packages/contracts-ts/src/generated/core/capability.ts`）⇒ 改它要动 `.proto` 并再生 ⇒ 触发 CI-only 的
`cargo xtask schema --check`（宿主机 `EBUSY`）。沿用 ADR 0081 决策 5 的同一判据：**本机验不了的改动不算
「可自主推进」**。H02 的 `DecisionKind` **不在任何契约名里**，故本轮做它。

## 三、设计（详见 [ADR 0082](../adr/0082-assembly-log-wiring.md)）

- **判定与追加合并为一个入口** ⇒ 不变量由结构保证，不靠调用方纪律。
- **载荷恰为判定放行的那一份**：按**记录进入顺序**的片段 `id`／来源标识／**字面正文** + 模型与提示词版本；
  `content` MUST NOT 为空、MUST NOT 是内存指针（H02 的判据正是「只存了指向当时内存的指针」）。
- **不可重建 ⇒ 具名拒绝，一字节不落盘**；`AssemblyOutcome` 是枚举 ⇒「已落日志」与「已拒绝」**类型上**可分。
- **重建只读真实链**：按**日志自身顺序**解出片段 → 交给**同一个** `rebuild_assembly`（**调用**，不是复写）
  → 映射为 owned 的 `AssemblyRebuild`。用例 `rebuilding_from_an_empty_log_reports_every_id_unlogged`
  钉住「输入必须来自链」。

## 四、本轮最重要的实测发现：种类是**两处定义**

`migrations/sqlite/0005-decision-log.sql` 把种类写成闭世界：

```sql
kind TEXT NOT NULL CHECK(kind IN('event','observation',…,'duty_lease'))
```

它与 Rust 的 `DecisionKind` 枚举构成**同一件事的两处定义**。只改枚举不改 CHECK ⇒ 插入被 schema 拒绝，
而且**不响亮**：telemetry 的「写失败 ⇒ 进有界缓冲」策略让条目静静躺在内存里。

**首跑现场**（三次 `ack.persisted == false` + 「日志里没有 assembly 行」）：

```
---- a_rebuildable_assembly_reaches_the_log_with_its_fragment_bodies stdout ----
thread '…' panicked at crates\nai-runtime\tests\assembly.rs:107:5:
a rebuildable assembly must persist
---- the_chain_alone_reproduces_the_logged_assembly stdout ----
thread '…' panicked: the log alone must reproduce the assembly: ["rebuild.fragment_not_logged"]
```

⇒ 新增迁移 `0007-decision-log-assembly-kind.sql`：SQLite 不能就地改 `CHECK`，故

1. 建新表（`CHECK` 含 `assembly`），
2. `INSERT … SELECT` **逐字复制**既有行（含 `seq`、`payload_hash`、`prev_hash`、`entry_hash`），
3. `DROP` 旧表、`ALTER … RENAME` 回原名，
4. 重建索引与两个只追加触发器。

链**不受影响**：`entry_hash` 的 preimage 不含表名。`migration::VERSION` 6 → 7、`SQL` 数组 +1 段。

**升级路径 MUST 被实测**（否则 `INSERT … SELECT` 只在空表上跑过）——新增
`migration::tests::version_six_upgrade_preserves_the_decision_chain_and_accepts_the_assembly_kind`：
先在 v6 库上**证明旧 CHECK 拒绝 `assembly`**，再 `migrate()`，然后断言既有行逐字保留 + 新种类被接受 +
`DELETE` 仍被触发器拒绝。

## 五、版本号的连带面，与其**一处不可再生**的如实披露

`backup.schema.json` 的 `db_schema_version` 与 `migration::VERSION` 成对（ADR 0045）⇒ 四处同步：

| 载体 | 改动 |
|---|---|
| `schemas/config/backup.schema.json` | `{"const":6}` → `{"const":7}` |
| `packages/contracts-ts/src/generated/config/backup.ts` | `db_schema_version: 6;` → `7;`（**生成物**） |
| `tests/contracts/schema.test.mjs` | 样本 6 → 7，并把「紧邻上一版」负例由 5 改为 6 |
| `crates/nai-storage/src/tests/knowledge_publication.rs` | 清单断言 6 → 7 |

⚠️ **如实披露**：`db_schema_version` 属 `TS_GENERATED_SOURCES`，再生入口 `cargo xtask schema --check`
（→ `scripts/contracts/generate.mjs`，内部 `spawnSync(process.execPath)`）在宿主机 **`EBUSY`、本机跑不了**。
**处置与依据**：生成物手工同步**一个 token**；依据是**已提交的一对**——`schemas/config/backup.schema.json`
的 `const: 6` 与 `packages/contracts-ts/src/generated/config/backup.ts` 的 `db_schema_version: 6;`
**逐字对应**，即「生成物 == schema const」这条不变量在本仓**当前成立**，手工同步保持它成立；
由 CI 的 `schema --check` 复核。**这不是把本机验不了的改动当成已达标**，而是把不可再生面**压缩到一个
token 并明说**。剩余的 schema 面（AJV 静态校验、样本）**本机已跑**（见 §六 `tests/contracts`）。

**顺带就地订正一处陈旧**：`docs/storage.md` 的迁移段写「至 `0005-decision-log.sql`，当前 schema 5」、
备份段写「要求 `db_schema_version=5`」——W51 把 `VERSION` 升到 6 时**未同步**这两句。本轮 1:1 订正为 7。
另三处 `VERSION = 6，6 个脚本`（`docs/kernel.md`、`docs/development-plan.md`、`AGENTS.md`）同步为 7/7——
**全仓扫描确认无残留 `VERSION = 5`**，即既有轮次对这些计数一律**就地更新**。

## 六、实现

| 文件 | 改动 |
|---|---|
| `crates/nai-storage/src/decision.rs` | **+1 枚举变体** `DecisionKind::Assembly`（`as_str()` ⇒ `"assembly"`；`criticality()` 的 Critical 表**不动** ⇒ 归 Telemetry） |
| `migrations/sqlite/0007-decision-log-assembly-kind.sql` | **新增**：逐列重建 `decision_log`，`CHECK` 收 `assembly` |
| `crates/nai-storage/src/migration.rs` | `VERSION` 6 → 7、`SQL` **+1 段**、**+1 升级测试** |
| `crates/nai-runtime/src/assembly.rs` | **新增**：`AssemblyRefusal` / `AssemblyOutcome` / `AssemblyRebuild` + `record_assembly` / `rebuild_assembly_from_log` |
| `crates/nai-runtime/src/composition.rs` | `short()` 放宽为 `pub(crate)`（**不**新增第二份预算定义） |
| `crates/nai-runtime/src/lib.rs` | `mod assembly;` + `pub use`（+2 行） |
| `crates/nai-runtime/tests/assembly.rs` | **新增 12 项** |
| 契约四处 | 见 §五 |

## 七、非空洞验证

### 7.1 五组注入（备份拷回 + 真跑 + `sha256` 逐字节还原）

基准 = **过完 `cargo fmt --all` 之后**的那一份（三份文件哈希见下表）。每组注入后即刻还原并 `sha256sum -c`
核对，最后复跑基线 **storage 4/4、runtime 12/12** 全绿。

| # | 注入 | 实测红 |
|---|---|---|
| I1 | 载荷 `content` 落 `Value::Null`（只存指针） | **5 条**（正文真落盘 / 往返重建 / 顺序 / 非升序顺序 / 超大载荷） |
| I2 | 拒绝分支不再早退（照旧追加） | **7 条**（五条拒绝 + 可区分性 + 非空洞守卫） |
| I3 | 片段按 id 排序落盘 | **1 条**（`the_log_preserves_the_entry_order_rather_than_re_sorting_it`） |
| I4 | 载荷漏 `model_version` | **1 条**（`a_rebuildable_assembly_reaches_the_log_with_its_fragment_bodies`） |
| I5 | 迁移 CHECK 去掉 `assembly` | storage **1 条** + runtime **4 条** |

### 7.2 ⚠️ 两组注入逼出了**两处测试缺口**（最重要的方法论产出）

- **I3 差点零信号**：原往返用例 `the_chain_alone_reproduces_the_logged_assembly` 的 id 是
  `frag-a/frag-b/frag-c`，**升序**——「按 id 排序落盘」与「忠实按进入顺序落盘」在这种输入上
  **不可区分**。⇒ 新增 `the_log_preserves_the_entry_order_rather_than_re_sorting_it`（用 `frag-c, frag-a`
  这种**非升序**输入）。**没有这条用例，I3 的注入不会红**，而设计文档里「顺序 MUST 落成记录顺序」就成了散文。
- **I4 零信号**：原先**没有任何断言**钉住载荷携带 `model_version`/`prompt_version`——重建读的是
  `record` 参数的版本，不读链上的版本，所以漏掉版本号在其余用例里**完全不可见**。⇒ 在载荷形状用例补
  两条断言。

⇒ 一般化：**注入的价值不只在于「证明守卫承重」，更在于「暴露守卫根本没覆盖的形状」**——
后者只能在**真跑**里发现，读代码读不出来。

### 7.3 宿主边界（如实）

`LNK1104`（无法打开 `assembly-*.exe`）在本次注入中**反复出现**（约每 2 次链接 1 次）。判据与处置沿用既有结论：
**逐目标能通过 ⇒ 非代码缺陷**，处置为**重试**（脚本对每个目标最多重试 3 次），**不改代码**。所有实测红/绿
数字均来自**成功链接后的运行**，回收的输出都带 `test result` 行。

## 八、门控与 validate

### 8.1 全量门控 `cargo xtask check --profile full`

**EXIT = 0**（工程基础 + 前端门全过；`device/SDK/GPU/install` 认证不在本 profile 内）。

**计数（现测，非推断）**：

- **Rust = 93/883**（**75** `Running` + **18** `Doc-tests` / **883** passed / **0** failed）。
  与 W103（92/870）**逐项对得上**：目标 **+1** 恰为新增的 `crates/nai-runtime/tests/assembly.rs`；
  `passed` **+13** = 该文件 **12** 条 + `migration::tests` 的**迁移升级用例 1** 条。
- ⚠️ **`Running` 计数 MUST 排除一行伪目标**：门控会打印
  `Running E:/nai-build-target\debug\nai-xtask.exe …`（gate **自执行** xtask 二进制），它**不是**测试目标
  （实测该行 **1** 行 ⇒ 原始 76 − 1 = **75**）。不排除就会把目标数报大 1。
- **Node 面**：本轮**未新增/删除任何 Node 套件或用例**（改动仅是把 `tests/contracts/schema.test.mjs`
  既有用例的常量 6 → 7、负例 5 → 6）⇒ 不进 Rust 目标计数。门控实测 TAP 根 **13**、`# tests` 合计 **317**、
  `# fail` **0**。⚠️ 该口径**不同于**历轮的「portable 面」数字（后者是逐个 `schema::node`＋`registry::check`
  落点之和）⇒ **MUST NOT 把 317 与 260 混为一谈**。

### 8.2 `cargo fmt --all --check`

**EXIT = 0**（首轮门控前已 `cargo fmt --all`；此后未再改 Rust 源码）。

### 8.3 ⚠️ 首次门控**不是**一次即绿——`clippy -D warnings`

首轮门控在 **clippy** 步报：

```
error: deref which would be done by auto-deref
  --> crates\nai-runtime\tests\assembly.rs:48:17
48 |             id: *id,
   = note: `-D clippy::explicit_auto_deref` implied by `-D warnings`
```

⇒ **`cargo test` 绿 ≠ 门控绿**（`cargo test` 只跑编译器、不跑 lints）。处置 = `.copied()`
（**不是**宿主瞬态、**不是**误报）。修好后重跑门控 **EXIT = 0**。

### 8.4 OpenSpec validate（归档前）

`openspec validate 2026-10-14-w104-assembly-log-wiring --strict --no-interactive` ⇒ **`is valid`**。

### 8.5 ⚠️ 归档**当场**逼出一个真实缺陷：MODIFIED 需求被追加句推过 500 字符

归档本身成功（`+3 / ~1`），但紧随其后的第一条 validate **红了**：

```
⚠ [WARNING] requirements[1]: Requirement text is very long (>500 characters).
Totals: 42 passed, 1 failed (43 items)
```

- **判据现测**：HEAD 时该需求（`Every assembled fragment is sourced, scoped and reconstructible`）正文
  **424** 字符（< 500，通过）；本轮把 W104 的边界指向句追加后 **652**（> 500）。
  ⇒ **是本轮引入的，不是既有欠账**（且归档前该 spec 的每一条需求都 < 500）。
- **⚠️ 为什么归档前没红**：`openspec validate <change>` 只校验**变更自身**（delta 结构），
  **不施加 spec 级的「需求正文长度」规则**——delta 里那份 652 字符的文本归档前**照样**报 `is valid`。
  ⇒ **变更级校验通过 ≠ 施加之后 spec 级校验通过。** 这两个校验面**必须分开验**。
- **⚠️ 为什么本机门控也没红**：门控（`cargo xtask check --profile full`）**不跑** `openspec validate`；
  `scripts/contracts/workflow-steps.mjs` 把 `pnpm spec:validate` 与 `openspec validate --archived` 归为
  **`not-in-profile`**（本机**能**跑、但**没接进** profile）。而 CI（`.github/workflows/foundation.yml`）**跑这两条**
  ⇒ **若不在此处发现，会直接红在 CI。** 这是「**本机绿 ≠ CI 绿**」的又一次实例，且这次差的是**整个校验面**。
- **处置（两处就地 1:1）**：把追加句压到**房风格的最短形式**（保留边界语义、去掉两条需求名的逐字引用）：
  「…判定的是「能否只靠日志重建」；落链与运行期重建由 `runtime-composition` 承担，本需求 MUST NOT 被读作「已落链」。」
  活文档 `openspec/specs/agent-harness/spec.md` 与归档 delta
  `openspec/changes/archive/2026-10-14-w104-*/specs/agent-harness/spec.md` **逐字一致**（实测 `live === delta → true`）。
  依据本仓「历史记录**同轮未提交 ⇒ 就地订正**」判据（归档目录**本轮尚未 commit**）⇒ 就地改，**不**追加勘误段。
- **复测（四条，全绿）**：

  | 命令 | 结果 |
  | --- | --- |
  | `openspec validate --specs --strict` | 43 passed / 0 failed（EXIT 0） |
  | `openspec validate --changes --strict` | 无待校验项（EXIT 0） |
  | `openspec validate --all --strict`（**CI 等价**） | 43 passed / 0 failed（EXIT 0） |
  | `openspec validate --archived`（**CI 等价**） | 102 passed / 0 failed（EXIT 0） |

- **门控复跑（第三次，对应真正的提交树）**：改动 `openspec/specs/**` 后重跑
  `cargo xtask check --profile full` ⇒ **EXIT = 0**，计数**现测复核不变**：Rust **93/883**（75 + 18 / 883 / 0），
  Node 套件 **13** / `# tests` **317** / `# fail` **0**。⇒ 归档与被改的 spec **都没**挪动任何被测计数。
- **方法论**（已进 `KNOWLEDGE.md` §6.27）：本仓把 `openspec validate` 划到 `not-in-profile` ⇒
  本地闭环里**必须显式补跑** CI 的那两条（`--all --strict` 与 `--archived`），否则改动只在本机「看起来」是绿的。

## 九、CI

（见下方「回填」小节。）

## 十、遗留

- **H02 保持 `pending`**：本轮可测的是「判定**之后**那一步真的发生、且只靠链可重建」；**装配器的真实产生者**
  （把九段片段喂给模型、§41.2 分段预算初值标定、§47.2 六类回归属 **H16**）**仍未接线**。**如实披露不是缺陷，
  不实声明才是。**
- **H17**（`evaluate_degradation` → `CapabilityView` 与界面）**不做**，属另一轮。
- 既有遗留（非本轮产生，不主动清理）：W90 §八 1/3/4/5；W91 跨语言键级消费；W92 基线收录；W93 锚点唯一性；
  W94 `needs_external` 审计；W95 `ci-only` 成因；W96 五项；W97 六项；W98 §八 1/2/7；W99 §八 1/2/3；
  W100 §八 2/3/4；W101 §八 1/2/3/4/5；W102 §八 1/2/3/4/5/6/7；**W103 §八**。
- **本轮新增披露**：`db_schema_version` 的 TS 生成物为**手工同步**（§五）；CI 的 `schema --check` 是本轮
  唯一的非本机验证环节。
- **本轮新增披露（归档当场）**：MODIFIED 需求曾被追加句推过 500 字符、**归档后才由第一条 validate 报出**；
  已就地把活 spec 与归档 delta 同步压短（§8.5）。**根因是校验面错位**——变更级 validate 不施加 spec 级长度规则，
  且 `openspec validate` 在本仓属 `not-in-profile`（门控不跑）⇒ 已改为**本地显式补跑 CI 的两条**。
