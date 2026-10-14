# Design — W104 装配判定接上真实哈希链（H02 runtime 接线）

## 1 缝在哪里

W69 交付了 `rebuild_assembly`（判定：日志能否只靠自己重排同一段片段序列），并在 `assembly.rs` 模块文档与
`docs/kernel.md` 里写死了边界：

> 把装配记录追加进 `nai-storage` 的哈希链、以及执行 §47.2 的六类回归（能力包 **H16**），都是 runtime 与
> 证据生产侧的事。

实测这句话的分量：`grep -rn 'rebuild_assembly\|AssemblyRecord\|LoggedFragment' crates/ apps/` 在
**生产代码**里零命中，只有 `crates/nai-kernel/tests/*.rs` 命中。于是两条今天**无法被判**的命题：

1. 一次被判**可重建**的装配，**会不会照样**被写进链？（没有调用方 ⇒ 没有答案）
2. 真写进链的那份载荷，**能不能只靠链**重排出同一段片段序列，还是有人只存了指向当时内存的指针？

这正是本项目反复出现的「**判定面存在、执行体不存在**」缺口——与 W54（桌面壳）、W68（讲品循环）、
W103（观测接线）同型。本轮把它补上，判据就从「人读代码」变成「测试能判」。

## 2 为什么落在 `crates/nai-runtime`

接线需要同时看见**判定**与**只追加哈希链**，而这两个能力分属两个 crate：

| 需要的面 | 位置 | 谁依赖它 |
|---|---|---|
| `rebuild_assembly` / `AssemblyRecord` / `LoggedFragment` | `crates/nai-kernel/src/evaluate/assembly.rs` | 由 `nai-kernel` 导出 |
| `Store::append_decision` / `Store::decision_entries` / `DecisionKind` | `crates/nai-storage/src/decision.rs` | 由 `nai-storage` 导出 |

`crates/nai-runtime/Cargo.toml` 同时持有 `nai-kernel`（path）与 `nai-storage`（path）——**这是唯一
既能看到判定、又能看到追加的落点**，并且它本来就是「把存储、会话、决策日志、媒体门控装配成一个受监督
整体」的那一层。放在 kernel 里会引入 kernel→storage 的越界依赖，放在 storage 里会让存储层认识 kernel 的
领域类型；两者都违反既有依赖方向（`cargo xtask check-deps` 强制）。**零新增依赖**。

## 3 存储面：新增 `DecisionKind::Assembly`

W103 复用既有的 `DecisionKind::Observation`；H02 没有对应种类，故新增一个变体：

```rust
pub enum DecisionKind { …, Assembly, … }   // as_str() ⇒ "assembly"
```

- **归 Telemetry**：`criticality()` 的 Critical 表（`Cue`/`Approval`/`Cancellation`/`DutyLease`）**不动**
  ——装配日志是可丢的遥测，不是授权/审批类不可丢决定（INV-15）。
- `DecisionKind` 枚举**不在任何 OpenAPI/proto 契约里** ⇒ 新增变体不改任何契约名。这正是 H02 满足
  「可自主推进」的关键——H17 的落点 `CapabilityView` 是 protobuf 生成类型，改它要动 `.proto`
  并再生，触发 CI-only 的 `cargo xtask schema --check`，故 H17 本轮避开。

### 3.1 种类是**两处定义**：枚举 + 表的 `CHECK`（本轮实测发现）

`migrations/sqlite/0005-decision-log.sql` 把种类写成了一个闭世界：

```sql
kind TEXT NOT NULL CHECK(kind IN('event','observation','decision','guard','cue','approval',
  'synthesis','playback_ack','cancellation','expiry','usage','duty_lease'))
```

**只改 Rust 枚举而不改它，插入会被 schema 拒绝**——而且**不是响亮地失败**：`append_decision` 对 telemetry
类别的策略是「写失败 ⇒ 进有界缓冲」，于是 `ack` 回的是 `persisted: false`，条目静静躺在内存缓冲里。
W104 的**首跑就是这条**：三个用例红在 `ack.persisted` 与「日志里没有 `assembly` 行」，而不是红在断言缺失。
这正是本项目反复出现的形状（「集合的补集」「同一件事两处定义」）在**存储 schema** 上的又一实例：
**把 `assembly` 加进枚举，只完成了闭世界的一半**。

处置：**新增迁移** `0007-decision-log-assembly-kind.sql`。SQLite 不能就地改 `CHECK`，故

1. 建新表（`CHECK` 含 `assembly`），
2. `INSERT … SELECT` **逐字复制**既有行（含 `seq`、`payload_hash`、`prev_hash`、`entry_hash`），
3. `DROP` 旧表、`ALTER … RENAME` 回原名，
4. 重建索引与两个只追加触发器。

链**不受影响**：`entry_hash` 的 preimage 不含表名，`verify_decision_chain` 在重建后重算同一串哈希。
`migration::VERSION` 6 → 7；`SQL` 数组加 1 段。

**升级路径必须被实测**（否则 `INSERT … SELECT` 只在空表上跑过）：新增
`migration::tests::version_six_upgrade_preserves_the_decision_chain_and_accepts_the_assembly_kind`——
先在 v6 库上**证明旧 CHECK 拒绝 `assembly`**，再 `migrate()` 到 v7，然后断言既有行逐字保留、
新种类被接受、`DELETE` 仍被触发器拒绝。

### 3.2 版本号的连带面：`db_schema_version`

`backup.schema.json` 的 `db_schema_version` 是 `{"const":6}`，与 `migration::VERSION` 成对（ADR 0045）。
`VERSION` 变 7 就必须同步：schema const、其 **TS 生成物**、本地契约测试样本、以及清单断言。
⚠️ **一处本机不可再生**：`db_schema_version` 属 `TS_GENERATED_SOURCES`，再生入口
`cargo xtask schema --check` → `scripts/contracts/generate.mjs`（内部 `spawnSync(process.execPath)`）
在宿主机 `EBUSY`。**如实披露**：生成物由手工同步一个 token，理由与依据是**已提交的一对**
（`schemas/config/backup.schema.json` 的 `const: 6` 与 `backup.ts` 的 `db_schema_version: 6;` 逐字对应）
——即「生成物 == schema const」这条不变量在本仓**当前是成立的**，手工同步保持它成立；CI 的
`schema --check` 复核。这不是把本机验不了的改动当成已达标，而是**把不可再生面压缩到一个 token 并明说**。

## 4 接口形状：判定 + 追加合并为一处

```rust
// crates/nai-runtime/src/assembly.rs
pub struct AssemblyRefusal {
    pub rejections: Vec<&'static str>,   // 逐条拒绝名，顺序同判定输出
    pub unlogged_ids: Vec<String>,       // 引用了却不在日志里的 id
    pub contentless_ids: Vec<String>,    // 有日志条目但正文只存在当时内存的 id
    pub credential_ids: Vec<String>,     // 正文含明文凭据的 id
}

pub enum AssemblyOutcome {
    Logged(DecisionAck),
    Refused(AssemblyRefusal),
}

pub struct AssemblyRebuild {             // 只读真实链重建的**拥有所有权**结果
    pub rebuildable: bool,
    pub rejections: Vec<&'static str>,
    pub rebuilt: Vec<(String, String)>,  // (id, source_ref)，按日志自身顺序
    pub unlogged_ids: Vec<String>,
    pub contentless_ids: Vec<String>,
    pub credential_ids: Vec<String>,
}

impl Composition {
    pub fn record_assembly(
        &mut self,
        record: &AssemblyRecord<'_>,
        fragments: &[LoggedFragment<'_>],
    ) -> Result<AssemblyOutcome>;

    pub fn rebuild_assembly_from_log(
        &self,
        record: &AssemblyRecord<'_>,
        limit: u32,
    ) -> Result<AssemblyRebuild>;
}
```

**为什么把「判定」和「追加」放在同一个方法里**，而不是暴露「先 `rebuild_assembly` 再 `append`」两个出口：
后者把「不许把不可重建的装配塞进链」这条不变量的责任**推给了每个调用方**——只要有一个人忘了先判定，
不变量就破了，而测试还看不出来（W92/W93「把门槛交给被审者自己填」的形状）。合并成一个入口后，
**没有**一条路径能不经判定而追加装配记录。

## 5 载荷的形状：runtime 是传输者，不是加工者

判定为可重建时，追加的载荷**只有三项**：

```json
{
  "fragments": [ {"id": "<id>", "source_ref": "<ref>", "content": "<正文>"}, … ],
  "model_version": "<v>",
  "prompt_version": "<v>"
}
```

不变式：

- **载荷 MUST 恰为判定放行的那一份材料**。`fragments` 按**记录里的进入顺序**落盘，每项带 `source_ref`
  与**字面正文**；`content` MUST NOT 为空、MUST NOT 退化成指向当时内存的指针——这正是 H02 的判据
  （`FragmentContentNotInLog` 明写「只存了指向当时内存的指针」）。
- **runtime MUST NOT 添加任何判定没审过的字段**。判定审的是「成员齐备、正文在、无凭据、顺序一致、
  版本非空」，故载荷恰为这几项之和，不加别的。
- **顺序 MUST 落成记录顺序**：`rebuild_assembly` 的比较基准是「日志自身顺序 vs 记录顺序」，
  若 runtime 把片段按 id 排序落盘，则「日志顺序 ≠ 记录顺序」将**恒真**，`RebuildOrderDiffers` 就成了一句空话。

## 6 重建的执行体：只读真实链

`rebuild_assembly_from_log` 的形状：

1. 用装配根既有的单 owner 只读入口 `Store::decision_entries`（**只读**，按 seq DESC 取再 reverse
   ⇒ **oldest first**）取回最近 `limit` 条，过滤 `kind == "assembly"`；
2. 按**日志自身顺序**逐行解出 `LoggedFragment`（`id`/`source_ref`/`content`），拼成
   `Vec<LoggedFragment>`，**不按记录排序**；
3. 调用**既有的** `rebuild_assembly(record, &logged)`——判定是**调用**，不是复写；
4. 把借用型 `RebuildVerdict<'log>` 映射为拥有所有权的 `AssemblyRebuild`（日志行是本函数的局部量，
   返回后即释放，故映射为 owned 是**传输**而非第二个判定）。

**为什么必须真读链**：若 `rebuild_assembly_from_log` 只把调用方给的 `fragments` 转手判一遍，那它证明的
只是「判定函数在手工切片上能跑」——W69 已经证过。H02 的命题是「**决策日志**能重建」，所以重建的**输入
必须来自链**。

⚠️ **顺序比较 gated on completeness**（既有判定事实，写测试须记住）：`rebuild_assembly` 只在
`rebuilt.len() == record.fragment_ids.len()`（片段全在）时才比较顺序；缺片段时只报 `FragmentNotLogged`。
故 `RebuildOrderDiffers` 的用例 MUST 让片段**齐全**而顺序不同，MUST NOT 同时断言「缺内容 + 顺序不同」。

## 7 非空洞守卫（迭代 `RebuildRejection::ALL`，五组注入）

沿用 W54/W66/W103 的做法：**真改 `crates/nai-runtime/src/assembly.rs` + 真跑 + 逐字节还原 + `sha256` 一致**。

**非空洞守卫**：一个测试遍历 `crates/nai-kernel` 的 `RebuildRejection::ALL`，对每个拒绝**构造一个 runtime
场景**并断言拒绝名确实出现在运行时返回的拒绝里。守卫的机械性来自**穷尽 match**：样本构造函数 `match
rejection { … }` 覆盖全部变体 ⇒ kernel 往 `ALL` 加一个变体而 runtime 未处理，**编译即失败**；
反过来遍历 `ALL` 使「加了变体却漏样本」不可能静默通过。这比手写一张拒绝清单强：**手写清单的盲区
恰是清单的补集**（W95 教训），而遍历 `ALL` + 穷尽 match 把覆盖面钉在**枚举自身**上。

五组注入**各自**必须让某个用例变红：

| # | 注入 | 拦的是什么 |
|---|---|---|
| I1 | `content` 落成 `null`（只存指针） | 「正文真落盘」与「只存指针」两种实现（H02 的要害） |
| I2 | 拒绝分支照旧追加 | 把「已拒绝」与「已落日志」混成同一个 `Ok` |
| I3 | 片段按 id 排序落盘 | 「日志顺序 ≠ 记录顺序」恒真 ⇒ 顺序判据失效 |
| I4 | 载荷漏 `model_version` | 只钉「进没进链」而不钉「以何形态进」的实现 |
| I5 | 迁移的 `kind` CHECK 去掉 `assembly` | 「枚举 + 表 CHECK 两处定义」的守卫是否承重（§3.1） |

各注入的样本定位 MUST NOT 按「态」定位（W92 教训）：样本按**被断言对象的属性**（如「记录里出现了日志
没有的 id」）构造，而非遍历一张按拒绝名分组的清单。

## 8 诚实边界

- 本轮的判据是「**判定之后**的那一步真的发生，且只靠链可重建」。装配器的**真实产生者**（把九段片段喂给
  模型、执行 §41.2 的分段预算初值标定、§47.2 六类回归的执行体属 H16）**仍未接线**。
- 故 H02 的 `status` 保持 `pending`：判定面 + 落链路径 + 只读重建已可测，但「本系统里真实产生一次装配并
  只靠链重建」还不是事实。**如实披露不是缺陷，不实声明才是。**
- §41.2 的分段预算**初值**仍须由 §47.2 黄金集回放实测确定并记入 ADR（§48.7 禁止采外部观察值）——属 H01/H16，
  本轮不碰。
