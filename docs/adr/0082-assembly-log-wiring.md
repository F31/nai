# ADR 0082：装配判定之后的**那一步**必须有执行体（H02 runtime 接线）

- 状态：已接受
- 日期：2026-10-14
- 轮次：W104
- 关联：[ADR 0081](0081-observation-log-wiring.md)（H15 的同型接线，本轮的模板）、
  [ADR 0064](0064-openapi-document-set-closure.md)（一处定义、两处使用）、
  [ADR 0027](0027-trace-id-single-definition.md)（trace 生成器一处定义）、
  [ADR 0045](0045-approval-display-text-boundary.md)（`db_schema_version` 与 `migration::VERSION` 同步）

## 背景

V3.11 §41.5 要求上下文装配 MUST 落决策日志，且 MUST 可由**日志的片段引用**重建、MUST NOT 依赖当时内存。
W69 把这条判定落成了 `crates/nai-kernel/src/evaluate/assembly.rs::rebuild_assembly`，并在模块文档与
`docs/kernel.md` 里写死了边界：

> 把装配记录追加进 `nai-storage` 的哈希链、以及执行 §47.2 的六类回归（能力包 **H16**），都是 runtime 与
> 证据生产侧的事。

这句话当时是**诚实**的。问题是它也是**不可核实**的：

```
$ grep -rn 'rebuild_assembly\|AssemblyRecord\|LoggedFragment' crates/ apps/
crates/nai-kernel/tests/*.rs        ← 只有测试
```

生产代码**零调用方**。于是两条今天**无法被判**的命题，无论仓库里有多少测试：

1. 一次被判**可重建**的装配，会不会**照样**被写进链？
2. 真写进链的那份载荷，能不能**只靠链**重排出同一段片段序列——还是有人只存了指向当时内存的指针？

这与 W103（H15）**完全同型**：**判定面存在、执行体不存在**。「有判定面」与「有执行体」是两件事，
前者是后者的必要条件、不是充分条件——本 ADR 是这条区分在本仓的**第七次**现身
（W54 桌面壳 / W68 讲品循环 / W103 观测接线之后）。

## 决策

### 决策 1：接线落在 `crates/nai-runtime`

接线必须同时看见**判定**（`nai-kernel`）与**只追加哈希链**（`nai-storage`）。`nai-runtime` 同时依赖两者，
且它本来就是「把存储、会话、决策日志、媒体门控装配成一个受监督整体」的那一层。

- 放进 `nai-kernel` ⇒ 引入 kernel→storage 依赖（`cargo xtask check-deps` 会红）；
- 放进 `nai-storage` ⇒ 让存储层认识 kernel 的领域类型（`AssemblyRecord`/`LoggedFragment`）。

两者都违反既有依赖方向。⇒ 落点是**推出来的**，不是偏好。**零新增依赖**。

### 决策 2：判定与追加**合并为一个入口**

```rust
pub fn record_assembly(&mut self, record: &AssemblyRecord<'_>, fragments: &[LoggedFragment<'_>])
    -> Result<AssemblyOutcome>;
```

若提供「先 `rebuild_assembly`、再 `append`」两个出口，「不许把不可重建的装配塞进链」这条不变量的责任就
**推给了每个调用方**——一个人忘了先判定，不变量就破了，而**测试看不出来**。这是 W92/W93「把门槛交给
被审者自己填」在**接口层**的又一实例：**不变量要由结构保证，而不是由纪律保证**。

### 决策 3：载荷恰为判定**放行**的那一份材料，且正文必落字面

```json
{ "fragments": [ {"id": …, "source_ref": …, "content": …} , … ],
  "model_version": …, "prompt_version": … }
```

- `fragments` 按**记录里的进入顺序**落盘，每项带**字面正文**；`content` MUST NOT 为空、MUST NOT 退化成
  指向当时内存的指针——这正是 H02 的判据（`FragmentContentNotInLog` 明写「记了，但正文没落盘」）。
- **MUST NOT 添加判定没审过的字段**；判定审的是「成员齐备、正文在、无凭据、顺序一致、版本非空」，
  载荷恰为这几项之和。
- **顺序 MUST 落成记录顺序**：`rebuild_assembly` 的比较基准是「日志自身顺序 vs 记录顺序」，
  若 runtime 把片段按 id 排序落盘，则「日志顺序 ≠ 记录顺序」将**恒真**，`RebuildOrderDiffers` 成空话。

### 决策 4：不可重建 ⇒ **具名拒绝**，且**一个字节都不落盘**

`AssemblyOutcome::Refused(AssemblyRefusal{rejections, unlogged_ids, contentless_ids, credential_ids})`：
逐条拒绝名 + 三类问题 id。返回类型是枚举 ⇒「已落日志」与「已拒绝」**类型上**可分（铁律二：
**「没看到」与「出错了」类型上分开**）。

### 决策 5：重建**只读真实链**，且**调用**同一判定

`rebuild_assembly_from_log(record, limit)`：读回 `assembly` 条目 → 按**日志自身顺序**解出
`LoggedFragment` → 交给**同一个** `rebuild_assembly` → 映射为拥有所有权的 `AssemblyRebuild`
（日志行是本函数的局部量，返回后即释放，故映射为 owned 是**传输**而非第二个判定）。

**为什么必须真读链**：若它只把调用方给的切片转手判一遍，证明的只是「判定函数在手工切片上能跑」——
W69 已证过。H02 的命题是「**决策日志**能重建」，故重建的输入 MUST 来自链。
（用例 `rebuilding_from_an_empty_log_reports_every_id_unlogged` 钉住这一点。）

### 决策 6：种类是**两处定义** ⇒ 必须新增迁移（**本轮实测发现**）

`migrations/sqlite/0005-decision-log.sql` 把种类写成闭世界：

```sql
kind TEXT NOT NULL CHECK(kind IN('event','observation',…,'duty_lease'))
```

**只改 Rust 枚举而不改它，插入会被 schema 拒绝**——而且**不响亮**：`append_decision` 对 telemetry 的
策略是「写失败 ⇒ 进有界缓冲」，于是 `ack.persisted == false`，条目静静躺在内存里。**W104 首跑即复现**：
三个用例红在 `ack.persisted` 与「日志里没有 `assembly` 行」。

⇒ 新增迁移 `0007-decision-log-assembly-kind.sql`：SQLite 不能就地改 `CHECK`，故**逐列重建**该表、
`INSERT … SELECT` **逐字复制**既有行（含四个哈希列与 `seq`）、`DROP` 旧表、`ALTER … RENAME` 回原名、
重建索引与两个只追加触发器。链**不受影响**：`entry_hash` 的 preimage 不含表名。
`migration::VERSION` 6 → 7，`SQL` 数组 +1 段。

**升级路径 MUST 被实测**（否则 `INSERT … SELECT` 只在空表上跑过）：新增
`migration::tests::version_six_upgrade_preserves_the_decision_chain_and_accepts_the_assembly_kind`——
先在 v6 库上**证明旧 CHECK 拒绝 `assembly`**，再升级，然后断言既有行逐字保留、新种类被接受、
`DELETE` 仍被触发器拒绝。

### 决策 7：`db_schema_version` 的连带面，与其**一处不可再生**的如实披露

`backup.schema.json` 的 `db_schema_version` 与 `migration::VERSION` 成对（ADR 0045）。`VERSION` 变 7 就必须
同步 schema const、其 TS 生成物、本地契约测试样本、清单断言。

⚠️ **一处本机不可再生**：`db_schema_version` 属 `TS_GENERATED_SOURCES`，再生入口
`cargo xtask schema --check` → `scripts/contracts/generate.mjs`（内部 `spawnSync(process.execPath)`）
在宿主机 `EBUSY`。**处置与理由**：生成物手工同步**一个 token**，依据是**已提交的一对**
（`backup.schema.json` 的 `const: 6` 与 `backup.ts` 的 `db_schema_version: 6;` 逐字对应）⇒
「生成物 == schema const」这条不变量在本仓**当前成立**，手工同步保持它成立；CI 的 `schema --check` 复核。

**这不是把本机验不了的改动当成已达标**，而是把不可再生面**压缩到一个 token 并明说**——
与决策 8 的判据一致。

### 决策 8：不先做 O05 / H17——**本机验不了的改动不算「可自主推进」**

沿用 ADR 0081 决策 5 的同一判据。H17 的落点 `CapabilityView` 是 protobuf 生成类型，改它要动 `.proto`
并再生 ⇒ 触发 CI-only 的 `schema --check`，本机验不了。H02 的 `DecisionKind` **不在任何契约名里**，
唯一连带面是决策 7 的一个 token，故本轮先做它。

## 后果

- 新增 `crates/nai-runtime/src/assembly.rs`：公开面只有 `AssemblyOutcome` / `AssemblyRefusal` /
  `AssemblyRebuild` 三型 + 两个方法，**不加**便利谓词（见替代方案 6）；
  `crates/nai-runtime/tests/assembly.rs`（**12 项**）；`composition.rs` 把 `short()` 放宽为 `pub(crate)`
  （复用既有逐次调用预算，ADR 0027「一处定义」）；`lib.rs` **+2 行**。
- `crates/nai-storage/src/decision.rs` **+1 枚举变体**（`criticality()` 不并入 Critical 表）；
  `migration.rs` `VERSION` 6 → 7、`SQL` +1 段、**+1 升级测试**；
  `migrations/sqlite/0007-decision-log-assembly-kind.sql`（新增）。
- 契约：`db_schema_version` 6 → 7 四处同步（见决策 7）。
- 规格：`runtime-composition` **+3 added**；`agent-harness` **1 处 modified**（W69 那条需求末句 1:1 追加
  指向新落点的一句，其余五段场景逐字不动）。
- `rebuild_assembly` 的**判定规则一行未改** ⇒ 判定面与执行面**同轮可分别验证**。
- **非空洞注入五组**（真改文件、真跑、逐字节还原、`sha256` 一致，复跑基线 4/4 与 12/12 全绿）：
  I1 载荷 `content` 落 `null` ⇒ **5 红**；I2 拒绝分支不再早退 ⇒ **7 红**；I3 片段按 id 排序落盘 ⇒ **1 红**；
  I4 载荷漏 `model_version` ⇒ **1 红**；I5 迁移 CHECK 去掉 `assembly` ⇒ storage **1 红** + runtime **4 红**。
- ⚠️ **两组注入逼出了两处测试缺口**，已补齐（否则注入会**零信号**）：
  I3 —— 原往返用例用**升序** id，排序实现与忠实实现不可分 ⇒ 新增
  `the_log_preserves_the_entry_order_rather_than_re_sorting_it`（用非升序 id）；
  I4 —— 原先**无断言**钉住载荷携带版本号 ⇒ 在载荷形状用例补两条断言。
- **诚实边界**：H02 保持 **`pending`**。本轮可测的是「判定**之后**那一步真的发生、且只靠链可重建」；
  **装配器的真实产生者**（把九段片段喂给模型、执行 §41.2 分段预算初值标定、§47.2 六类回归属 **H16**）
  **仍未接线**。⇒ 报告 MUST NOT 出现「装配器已接入」「已不依赖当时内存」这类运行期断言。
- H17（`evaluate_degradation` → `CapabilityView`）本轮**不做**。

## 替代方案

1. **只补 happy path，不管拒绝**：判定函数已给出具名拒绝，看起来「拒绝那半已经做了」。但判定**给名字**与
   runtime**真的不追加**是两件事——W69 的判定对**任何**调用方都存在，而**没有任何调用方**。
2. **暴露 `rebuild_assembly` + `append` 两个出口**：更「灵活」，但把不变量交给调用方（决策 2）。
3. **载荷只存片段 id 与指针**（正文留在当时内存）：更小更快，但正是 H02 要禁的那一种
   （`FragmentContentNotInLog` 的原文就是「记了，但记的是内存指针」）。
4. **复用 `DecisionKind::Observation`，不新增种类**：省掉迁移。但装配记录与 §47.1 的五类观测**不是同一种
   记录**（前者的日志形状必须含**片段正文**，后者的 L1 行只有 `fragment_ref`/`fragment_source`），
   混在同一 `kind` 里会让 H15 的「五类各一条」计数口径变糊，且 `read_assembly_log` 只能靠 payload 形状
   识别——**同一件事两处定义**。
5. **就地改 `0005` 的 CHECK，不加迁移**：内部自洽（`verify` 只与同一 `SQL` 数组比对），但**已迁移的数据库
   会 checksum 漂移被拒**——迁移不可变是为「已发布」而设，不是为本机方便而设。
6. **给新模块加 `#[must_use]`/`is_logged()` 等便利 API**：枚举本身已可区分，再加谓词就是**同一件事两处
   定义**。⇒ 不加。
7. **顺手把 H17 的呈现接线一起做**：另一层对象，混进来会让证据面变糊（W102 替代方案 6 的同一条理由）。

## 关联

- 规格：`openspec/specs/runtime-composition/spec.md` 的三条 added 需求；`agent-harness` 的 1 处 modified
- 变更：`openspec/changes/archive/2026-10-14-w104-assembly-log-wiring/`
- 报告：[`docs/reports/2026-10-14-w104-assembly-log-wiring.md`](../reports/2026-10-14-w104-assembly-log-wiring.md)
- 前序：W69（判定面）、W103（同型接线）、W54（桌面壳）、W68（讲品循环）——同型的「缺失对象」轮次
