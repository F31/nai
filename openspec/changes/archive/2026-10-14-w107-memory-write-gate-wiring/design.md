# Design — W107 记忆写入判定接上 runtime 与真实存储（H03 runtime 接线）

## 1 缝在哪里

W57 同时交付了两半，但两半之间**没有接线**：

| 半 | 位置 | 状态 |
|---|---|---|
| 判定（§42.2 写入来源门槛） | `crates/nai-kernel/src/evaluate/memory.rs` 的 `admit_write` | 存在，**零生产调用方** |
| 存储（九列记忆表） | `migrations/sqlite/0001-foundation.sql:29` 的 `memory_records` | 存在，**零生产读写** |

实测（计数为**现测**，不是推断）：

```
$ grep -rn '\badmit_write\b' --include=*.rs crates apps | grep -v /tests/ | grep -v "src/evaluate/memory.rs"
$ grep -rn 'MemoryCandidate\|MemoryPolicy' --include=*.rs crates apps | grep -v /tests/ | grep -v "src/evaluate/memory.rs"
$ grep -rn 'memory_records' --include=*.rs crates apps | grep -v /tests/
crates/nai-storage/src/tests.rs:375
crates/nai-storage/src/tests.rs:432
```

前两条**零命中**（`admit_write`、`MemoryCandidate`、`MemoryPolicy` 在生产代码里没有调用方/构造方）；
第三条命中的两处都在测试里，且是**目录级**断言：`tests.rs:375` 把 `memory_records` 列进「表都在」
的清单，`tests.rs:432` 断言新建库这张表 `COUNT(*) == 0`。**没有一条生产路径碰过这张表。**

于是「一次被判定拒绝的记忆写入有没有落库」在 runtime 上**不可表达**——只能靠读源码回答。

## 1.1 与前四轮同型，但落点不同

| 轮次 | 判定面 | 执行体（本轮的落点） |
|---|---|---|
| W103 / H15 | `evaluate_observation` | 追加进只追加哈希链 |
| W104 / H02 | `rebuild_assembly` | 追加进只追加哈希链 |
| W105 / H18 | `evaluate_ramp_enablement` | 追加进只追加哈希链 |
| W106 / H01 | `evaluate_assembly` | 追加进只追加哈希链 |
| **W107 / H03** | **`admit_write`** | **写进领域表 `memory_records`** |

前四轮的执行体都是**决策日志**（哈希链），本轮是**领域表**。这带来一个额外的选择面——
执行体到底是「落链」还是「落库」——见 §3 的裁定。

## 2 为什么落在 `crates/nai-runtime/src/memory.rs`

接线需要同时看见**判定**与**真实存储**，两者分属两个 crate：

| 需要的面 | 位置 |
|---|---|
| `admit_write` / `MemoryCandidate` / `MemoryPolicy` / `MemoryRejection` | `crates/nai-kernel/src/evaluate/memory.rs` |
| `Store::record_memory` / `Store::memory_records` / `MemoryRecord` | `crates/nai-storage/src/store.rs` + `nai-contracts` |

`crates/nai-runtime/Cargo.toml` 同时持有 `nai-kernel`（path）、`nai-storage`（path）与
`nai-contracts`（workspace）——**这是唯一既能看到判定、又能看到存储的落点**（W103–W106 已在此
落过四次同型接线）。放在 kernel 里会引入 kernel→storage 的越界依赖，放在 storage 里会让存储层
认识 kernel 的判定类型；两者都违反既有依赖方向（`cargo xtask check-deps` 强制）。**零新增依赖。**

## 3 执行体落在 `memory_records` 表，**不是**决策日志

§42.2 的拒因后果原文是「MUST NOT 落库」（`openspec/specs/agent-harness/spec.md:288` 的 scenario
`An approved but unplayed candidate is offered as a fact`）——**落库**指向领域表，不是哈希链。

并且：记忆**观测**落链**已经**由 W103 的 `Composition::record_observation` 承担
（`agent-harness` 的「五类观测」需求明写其落链由 `runtime-composition` 的
「A judged-loggable harness observation reaches the hash chain」承担）。本轮若也去写一条
决策日志条目，就是**同一件事的第二处定义**（本仓明令禁止），且需要一个新的 `DecisionKind` 变体
⇒ 一次迁移 ⇒ 不再是「可自主推进」。

⇒ **落点 = `memory_records` 表，一次写入，一条路径。**

## 4 九列映射：逐字对应，零新增列

`MemoryRecord`（`crates/nai-contracts/src/generated/nai.v1.rs:1283`，9 字段）与
`memory_records`（`0001-foundation.sql:29`，9 列 + 复合主键）**逐字对应**：

| `MemoryRecord` 字段 | `memory_records` 列 |
|---|---|
| `record_id` | `record_id` |
| `workspace_id` | `workspace_id` |
| `session_id` | `session_id` |
| `viewer_scoped_id` | `viewer_scope` |
| `kind` | `kind` |
| `purpose` | `purpose` |
| `content` | `content` |
| `expires_at_utc_ms` | `expires_at_ms` |
| `authorization_ref` | `authorization_ref` |

⇒ **零迁移**：表已付费，列已齐备，无一处需要 `ALTER TABLE`。这正是不必付 W104 那种迁移代价的原因。

### 4.1 不落库的三项，以及为什么

`MemoryCandidate` 上还有 `source`、`claim`、`source_ref`、`written_at_utc_ms`、`publish_ref`。
**`memory_records` 没有对应列** ⇒ 它们**不落库**。这是**如实**的：§42.2 把这些字段定为
**判定输入**（门槛据此判断放行与否），而不是 §42.2 要求持久化的字段。runtime 是**传输**
不是**加工**——为了「看起来更完整」而在写入路径上多造列，就是**第二处定义**该表。
⇒ 该缺口与理由**如实登记**在报告与 `docs/storage.md`，不静默吞掉。

### 4.2 判定输入的「时间」不参与落库，但**参与判定**

`written_at_utc_ms` 只被 `admit_write` 用来判「有没有带时间」（`WriteWithoutTimestamp`），
不写进表里。表里唯一的时间列是 `expires_at_ms`，它来自 `record.expires_at_utc_ms`（判定**审过**的
输入的一部分）。两者**不是**同一个时间，MUST NOT 互相顶替。

## 5 判定**一次**，落库侧不二次判定

### 5.1 存储层不做记忆门槛

`Store::record_memory` 只做**存储层自己的事**：校验作用域标识（`id()`）、查会话权限
（`access()`，与 `record_candidate` 同族）、写九列、把主键冲突映射成具名 `Conflict`。
它**不**重跑「来源可信吗」「是事实还是叙述」——那是 `admit_write` 的判定，重跑就是
「一条开关答两次」（W88 的形状）。

### 5.2 也不抢 `expires_at_ms` 的时效判定

`record_candidate` 会在存储层拒掉已过期的候选（`DeadlineExceeded`）。`record_memory`
**不**做这一步：§42.3 的时效判定发生在**读取**面（`Expired` 是条目级拒因），
写入面没有这条规则。在存储层加一条写入面不存在的未成文规则，会把「判定面声明了什么」
与「存储层实际做了什么」搞成两个口径。

### 5.3 权限名复用 `session.manage`

写入口的权限检查**复用**既有权限名 `session.manage`（`record_candidate` 同款），
**不新增** `memory.write`。理由：本轮补的是「判定 → 执行体」的接线；新增一个权限名要连带改
`Composition::bootstrap_owner` 的授权表与测试夹具 `grant()`，那是一处**与判定无关**的授权面扩张。
记忆记录本就是**会话作用域**内的产物 ⇒ 复用的语义也站得住。
⚠️ 这一条**如实披露**在报告里：它是**选择**，不是「本来如此」。

## 6 测试与非空洞守卫

新增 `crates/nai-runtime/tests/memory.rs`（新测试目标 ⇒ Rust 目标数 +1，由 `cargo test --workspace`
自动覆盖、**无需**在 `engineering_checks` 登记）：

- 放行 ⇒ `memory_records` **恰一行**，九列与判定输入**逐字相等**（含 `viewer_scope` 从
  `viewer_scoped_id` 的映射）。
- 拒绝 ⇒ **零行**，且返回的拒绝名在场。
- **非空洞守卫**：样本构造函数对写入面的**八条**拒因**穷尽 `match`**（七条 §42.2 写入拒因 +
  `ViewerMemoryDisabled` 这条能力开关拒因），逐个构造运行期候选，断言拒绝名出现在返回里。
  kernel 加变体而此处未覆盖 ⇒ **编译失败**，不静默通过。
- 读取面访问器：写入后按作用域读回，逐列**往返相等**；另一会话的记录**不出现在**结果里。
- 能力开关：观众长期记忆关闭时，viewer 候选在**写入面**同样被拒（PD-31 的写侧半边）。

## 7 为什么 `MemoryWriteOutcome` 恰**两态**

W106 的 `AssemblyOutcome` 是**四态**，因为 H01 有**两项**判定（可调用 / 可重建）⇒ 两侧四条组合。
H03 只有**一项**判定（§42.2 的来源门槛）——`admit_write` 返回 `Result<(), MemoryRejection>`，
只有一个「放行 / 不放行」的轴。

⇒ 两态（`Written` / `Refused`）。多造一态会是**没有对应事实的空壳**——这正是 W92 那条教训的反向应用：
「**「记成什么」答不出 ⇒ 先问分类是否少一格**」，而这里分类是**恰够**的，多一格就是**无对象的分类**。

## 8 为什么拒绝走 `Ok(Refused)` 而不是 `Err`

与 W103–W106 同形：**判定说「不」是一个结论，不是一个故障**。`Err` 留给真正的调用方错误与
基础设施故障：

| 情形 | 返回 |
|---|---|
| `admit_write` 拒绝（来源不可信、缺时间、开关关闭…） | `Ok(MemoryWriteOutcome::Refused(..))` |
| 主键冲突、缺权限、标识非法、存储故障 | `Err(RuntimeError)` |

## 9 `memory_records` 读回访问器**不是** H04 的读取面

`Store::memory_records` 是**存储层的取数入口**：逐列还原九个字段，按作用域过滤，顺序确定性。
它 MUST **NOT** 施加 §42.3 的读取判定（作用域越权、过期、撤销、权限、近似顶替、配额）。

那条判定是 `read_records`，其条目级输入是 `ScopedRecord` 的 `revoked` / `permitted` /
`approximate_of`。其中 **`revoked` 在 `memory_records` 里没有任何列**（§42.4 的遗忘链是一条
真实状态，不是可以省略的概念）⇒ 在读取面写死 `revoked: false` 是**凭空断言一条判定输入**。

⇒ **H04 的 runtime 接线不在本轮**，并作为**具名开放问题**登记（`tests/registry.yaml` 的 H04 notes
与报告 §遗留边界）。本轮的读回访问器**只服务于「落库了没有」这一问**，MUST NOT 被读作 H04 已接。

## 10 替代方案与取舍

1. **只加 `Store::record_memory`，runtime 侧不加入口** —— 不行：判定仍然零调用方，
   「不许把未放行的写入落库」的责任落到每个调用方，缝原样保留。
2. **runtime 侧分开 `evaluate_memory` 与 `persist_memory` 两个出口** —— 不行：与 W103–W106 同一条
   理由，两个出口把不变量推给调用方。
3. **落库前在存储层重跑一遍来源判定** —— 不行：一条开关答两次（W88），两处口径会分家。
4. **同时接线 H04 读取面** —— 不行：`revoked` 无来源，只能凭空断言 ⇒ 触犯「注入判定」禁令。
5. **新增 `memory.write` 权限名** —— 取舍见 §5.3：本轮刻意不扩张授权面，如实披露。
6. **为落库新增 `DecisionKind::Memory` 并同时落链** —— 不行：需要一次迁移，且与 W103 的记忆观测
   落链构成同一件事的第二处定义。
7. **`MemoryWriteOutcome` 用三态（多一个 `Neither`）** —— 不行：只有一项判定，第三态无对应事实（§7）。
