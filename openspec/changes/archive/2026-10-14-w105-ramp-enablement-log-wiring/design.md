# Design — W105 爬坡启用记录接上真实哈希链（H18 runtime 接线）

## 1 缝在哪里

W67 交付了 `evaluate_promotion`（判定：一次晋升是否被允许），并在 `ramp.rs` 的**模块文档**里写死了边界：

> **Scope boundary.** This module decides *whether a promotion is allowed*. Enabling a mechanism per
> stage at runtime, and appending the record to the hash chain in `nai-storage`, is wiring and is
> **not** claimed here.

这一轮的缝比 W103/W104 更刺眼，因为**判定面自己把要落盘的记录算好了**：

```rust
// crates/nai-kernel/src/evaluate/ramp.rs:244
pub struct RampVerdict { …, pub record_to_append: Option<EnablementRecord> }
// crates/nai-kernel/src/evaluate/ramp.rs:322
let record_to_append = match (allowed, req.record) {
    (true, Some(record)) if req.target == RampStage::AutoSupervised => Some(record.clone()),
    _ => None,
};
```

`EnablementRecord` 是**拥有所有权**的类型（`mechanism: String`、`stage: RampStage`、
`fields: BTreeMap<String, String>`），所以 `record_to_append` 是一个**可以被直接序列化进链**的值,
且构造它的分支（`ramp.rs:322`）是专门为此写的。它的唯一消费者是 `crates/nai-kernel/tests/ramp.rs`。

实测这条缝的分量：全仓 `grep -rn 'record_to_append' crates/ apps/` 在**生产代码**里零命中。
于是两条今天无法被判的命题：

1. 一次被判**放行**的晋升，它算出来的那份启用记录**会不会照样**进链？
2. `record_to_append == None` 的两条原因（**被拒** vs **放行但阶段不在门控内**）今天**无法在运行期被区分**。

这正是本项目反复出现的「**判定面存在、执行体不存在**」缺口——与 W54（桌面壳）、W68（讲品循环）、
W103（观测接线）、W104（装配接线）同型。本轮补上。

### 1.1 为什么本轮比 H01 更自主

同批「可自主推进」候选里还有 **H01**（装配顺序接 runtime）。两者的 runtime 步性质不同：

| | runtime 步是什么 | 卡不卡外部前置 |
|---|---|---|
| H01 | 把九段片段**喂给模型**（真实装配器） | **卡** —— 模型是 H16/外部 |
| H18 | 把判定**已经算好**的记录**追加进链** | **不卡** —— 判定面自证「无能力包也保持可判定」（INV-33） |

`ramp.rs:28-30` 的原话就是这条依据：「the gate only consumes a declared `passed` set and never runs
a case, so **H18 stays decidable with no capability pack installed (INV-33)**」。故 H18 的
「判定之后那一步」完全落在仓库内、本机可跑、可注入验证 ⇒ 严格比 H01 更自主。

## 2 为什么落在 `crates/nai-runtime`

接线需要同时看见**判定**与**只追加哈希链**，而这两个能力分属两个 crate：

| 需要的面 | 位置 | 谁依赖它 |
|---|---|---|
| `evaluate_promotion` / `PromotionRequest` / `RampVerdict` / `RampRejection` | `crates/nai-kernel/src/evaluate/ramp.rs` | 由 `nai-kernel` 导出 |
| `Store::append_decision` / `DecisionKind` / `DecisionAck` | `crates/nai-storage/src/decision.rs` | 由 `nai-storage` 导出 |

`crates/nai-runtime/Cargo.toml` 同时持有 `nai-kernel`（path）与 `nai-storage`（path）——**这是唯一
既能看到判定、又能看到追加的落点**（W103/W104 已在此落过两次同型接线）。放在 kernel 里会引入
kernel→storage 的越界依赖，放在 storage 里会让存储层认识 kernel 的领域类型；两者都违反既有依赖方向
（`cargo xtask check-deps` 强制）。**零新增依赖**。

## 3 存储面：复用 `DecisionKind::Approval`（零迁移）

### 3.1 为什么不新增种类

W104 给装配记录新增了 `DecisionKind::Assembly`，代价是**一次迁移**——因为种类是**两处定义**：
Rust 枚举 **和** `0005-decision-log.sql` 的 `kind CHECK(kind IN(…))`。只改枚举不改 CHECK ⇒ 插入被
schema 拒绝，且**不响亮地失败**（telemetry 类别落进有界缓冲，`ack.persisted == false`）。

本轮的启用记录**不需要**付这个代价：`approval` 在**两个闭世界里都已存在**：

- Rust：`crates/nai-storage/src/decision.rs:31`（`DecisionKind::Approval`，`as_str() == "approval"`）；
- SQL：`migrations/sqlite/0005-decision-log.sql:1`（`CHECK(kind IN('event','observation',…,'approval',…))`）。

⇒ **不新增变体、不新增迁移、不改 `migration::VERSION`、不改 `db_schema_version`。**

### 3.2 为什么 `approval` 是**正确**的类别，不只是「恰好能用」

`DecisionKind::criticality()` 把 `Approval` 归 **Critical**（与 `Cue`/`Cancellation`/`DutyLease` 同组），
其文档原话是「Authorization, **approval**, cue and duty decisions are never dropped」（INV-15）。
这正好对上 §47.3 的要求「启用记录已备」：

- **Telemetry 类**的写入失败是**静默进缓冲**（`ack.persisted == false`）——一条启用记录如果可以被
  静默丢弃，§47.3 的「已备」就成了一句无法核实的空话。这正是 W104 差点踩的坑（它的 `assembly` 是
  telemetry，首跑即被 `ack.persisted == false` 抓出）。
- **Critical 类**的写入失败**原样返回错误**（`decision.rs` 的 `append_decision`：`Criticality::Critical`
  走独立事务，失败即 `Err`）⇒ 「**要么落地、要么被报告，绝不静默跳过**」（铁律二）。

且 §47.3 的晋升门控本身就是一次**授权**判定（「这次晋升是否被允许」）⇒ 落成 `approval` 类**语义相符**，
不是把不相关的东西塞进一个桶。

### 3.3 复用会不会污染既有读取面？——**实测：不会**

`approval` 类条目的既有读者是审批后端，它们读的是 **`approvals` 表**，**不**按 `decision_log.kind`
过滤：

```
crates/nai-storage/src/store.rs:708  approval_valid        ← FROM approvals a JOIN sessions … JOIN utterances …
crates/nai-storage/src/store.rs:734  list_pending_approvals ← FROM utterances u … LEFT JOIN approvals a …
crates/nai-storage/src/store.rs:802  read_approval          ← 同上
```

全仓 `grep -rn '"approval"' crates/ apps/ --include=*.rs` 的其余命中都在**与本种类无关**的枚举：
`monitoring.rs` 的移动端能力名、`multiplex.rs` 的通道名、`nai-control-api` 的 `category` 串、
`nai-contracts` 测试里的一个 `approval_ref` 字面量。**没有任何守卫枚举 `DecisionKind`**
（`grep -rln 'DecisionKind' scripts/ tests/contracts/` 空结果）。

### 3.4 代价：一个种类承载两种载荷形状（如实披露）

诚实起见：复用之后，`approval` 类的载荷有两种形状——审批后端的
`{candidate_hash, approval_id, epoch, actor_id}`（由 `decision::approval_entry` 写）与本轮的
`{mechanism, from, to, fields}`。**这是本轮唯一的语义代价**。它可接受，因为：① 没有读者按 kind 解析
载荷（§3.3 实测）；② 两条路径的 `trace_id` 前缀可区分（审批用 `approval_id`，本轮用
`self.next_trace("ramp")` 的 `ramp-N`）。`docs/storage.md` 会就此补一句，**不**让这个事实只存在于代码里。

## 4 接口形状：判定 + 追加合并为一处，且**三态**

```rust
// crates/nai-runtime/src/ramp.rs
pub struct RampRefusal {
    pub rejections: Vec<&'static str>,        // RampRejection::label()，顺序同判定输出
    pub missing_categories: Vec<&'static str>,// 声明了却没通过的回归类别（全部，不只第一个）
    pub leaked_fields: Vec<String>,           // 值内带明文凭据的字段名
}

pub enum RampOutcome {
    Logged(DecisionAck),                      // 放行且产出了启用记录 ⇒ 已追加 approval 条目
    NoRecordRequired { from: RampStage, to: RampStage }, // 放行，但目标阶段在门控之前 ⇒ 不追加，且非拒绝
    Refused(RampRefusal),                     // 拒绝 ⇒ 一个字节都不落盘
}

impl Composition {
    pub fn record_promotion(&mut self, req: &PromotionRequest<'_>) -> Result<RampOutcome>;
}
```

### 4.1 为什么把「判定」和「追加」放在同一个方法里

与 W103/W104 同理由：暴露「先 `evaluate_promotion` 再 `append`」两个出口，会把「不许把未放行的晋升
落进链」这条不变量的责任**推给每个调用方**——只要有一个人忘了先判定，不变量就破了，而测试看不出来
（W92/W93「把门槛交给被审者自己填」的形状）。合并成一个入口后，**没有**一条路径能不经判定而追加。

### 4.2 为什么必须是**三态**（本轮的实质设计决定）

`record_to_append == None` 有**两条**必须区分的原因，`ramp.rs:255-256` 的注释原文写明了：

> The record to append to the decision log. Produced only when a promotion into supervised automatic
> is allowed — **other stages carry no enablement requirement**.

| 情形 | `allowed` | `record_to_append` | 应当报成 |
|---|---|---|---|
| ① 判定拒绝（如回归未过、记录缺失、值含凭据） | `false` | `None` | **具名拒绝** |
| ② 放行，目标 = 受监督自动 | `true` | `Some(rec)` | **已落记录** |
| ③ 放行，目标在受监督自动**之前** | `true` | `None` | **无需记录**（既非拒绝、也非已落） |

把 ③ 塌进两态，无论塌向哪一边都会说谎：报「已落日志」是**谎**（一个字节没落），报「已拒绝」也是
**谎**（它没被拒，§47.3 对非门控阶段根本没有记录要求）。故入口**必须**三态，且三态**必须**互相可区分。
这正是本项目已记过的形状：「**「记成什么」答不出 ⇒ 先问分类是否少一格**」。

⚠️ 一个**实现纪律**：三态**不能**靠 `record_to_append.is_some()` 在运行期**反推** `allowed`。
反推会把「判定放行」与「判定拒绝」混成一个谓词——正是 kernel 模块文档开头警告过的折叠
（「Folding the two into one predicate makes 'was this refused because it is not enabled by default,
or because a condition was missed' undecidable again」，`ramp.rs:11-13`）。运行期 MUST 先读
`verdict.allowed`，再读 `verdict.record_to_append`，**两个字段各自读一次**。

## 5 载荷的形状：runtime 是传输者，不是加工者

判定放行时，追加的载荷**只有四项**：

```json
{
  "mechanism": "<机制名>",
  "from": "<起始阶段 label>",
  "to": "<目标阶段 label>",
  "fields": { "<键>": "<值>", … }
}
```

不变式：

- **载荷 MUST 恰为判定放行的那一份材料**。`mechanism`/`to`/`fields` 三项直接取自判定产出的
  `record_to_append`（`EnablementRecord`）；`ramp.rs:297-302` 已断言 `record.mechanism == req.mechanism`
  且 `record.stage == req.target`，故这三项是**判定审过**的。`from` 取自 `verdict.from`——判定通过
  `NotAnAdvance`（`target.index() > current.index()`，`ramp.rs:272`）**审过**这次转移确为前进，
  而「前进」是个二元关系：只写「现在在 auto_supervised」而不写「从 assisted 来」是不完整的证据。
- **MUST NOT 夹带判定没审过的字段**，尤其 `req.touched` / `req.passed`（两份 §47.2 回归类别集）。
  判定**消费**它们是为了**决定**，判定**没有**为落盘审过它们；把它们一并落账，就是把「runtime 是传输者」
  改成「runtime 是加工者」，也把回归类别这份可能随修订变化的状态钉进链。测试用**键集断言**钉这一点。
- **`fields` 里的凭据已由判定挡掉**：`ramp.rs:303-316` 按 `CREDENTIAL_MARKERS`（INV-11）**只扫
  `record.fields` 的 value**，命中即 `RecordContainsCredential` ⇒ `allowed == false` ⇒ 本轮**拒绝**，
  故任何落盘行的 `fields` 值都不含凭据字面量。

## 6 拒绝的形态：具名、且一个字节都不落盘

`RampOutcome::Refused` 携带判定给出的三类信息（`ramp.rs:250-254` 的三个输出字段）：

| 字段 | 来源 | 用途 |
|---|---|---|
| `rejections` | `verdict.rejections` 的 `label()` 序列 | 逐条具名，**不**只报第一个 |
| `missing_categories` | `verdict.missing_categories` | §47.3「MUST 列出**全部**缺失类别」的兑现 |
| `leaked_fields` | `verdict.leaked_fields` | 值内含凭据的字段名（**字段名**可以报，**值** MUST NOT） |

拒绝路径**不调用** `self.record(...)` ⇒ 链上零新增。测试对每条拒绝都断言「链上没有新条目」，
MUST NOT 只断言返回值的形状。

## 7 非空洞守卫（遍历 `RampRejection::ALL`，五组注入）

沿用 W54/W66/W103/W104 的做法：**真改 `crates/nai-runtime/src/ramp.rs` + 真跑 + 逐字节还原 + `sha256` 一致**。

**非空洞守卫**：一个测试遍历 `crates/nai-kernel` 的 `RampRejection::ALL`（**8** 条），对每个拒绝
**构造一个运行期场景**并断言拒绝名确实出现在 `RampOutcome::Refused` 的 `rejections` 里。守卫的机械性
来自**穷尽 match**：样本构造函数 `match rejection { … }` 覆盖全部变体 ⇒ kernel 往 `ALL` 加一个变体而
runtime 未处理，**编译即失败**；反过来遍历 `ALL` 使「加了变体却漏样本」不可能静默通过。这比手写一张
拒绝清单强：**手写清单的盲区恰是清单的补集**（W95 教训），而遍历 `ALL` + 穷尽 match 把覆盖面钉在
**枚举自身**上。

五组注入**各自**必须让某个用例变红：

| # | 注入 | 拦的是什么 |
|---|---|---|
| I1 | 拒绝分支照旧追加（判定不许时也写链） | 「已拒绝」与「已落日志」混成同一个 `Ok` |
| I2 | 载荷漏 `record.fields`（只落机制与阶段） | 「载荷恰为判定放行的那份材料」 |
| I3 | 目标阶段在门控**之前**时也追加（把 ③ 折进 ①/②） | 三态塌成两态（§4.2 的要害） |
| I4 | 载荷夹带 `touched`/`passed` | 传输者 vs 加工者；键集断言是否承重 |
| I5 | 运行期自带校验漏掉凭据扫描（把 `RecordContainsCredential` 从拒绝集里滤掉） | 「调用既有判定、不另写第二份」与 INV-11 是否承重 |

各注入的样本定位 MUST NOT 按「态」定位（W92 教训）：样本按**被断言对象的属性**构造（如「值里出现了
已知凭据标记」），而非遍历一张按拒绝名分组的清单。

## 8 诚实边界

- 本轮的判据是「**判定之后**的那一步真的发生」：放行的晋升其启用记录真的进链、拒绝的真的一字节不落、
  「无需记录」被具名而不被折叠。
- **§47.2 六类回归的执行体仍未接线**（能力包 **H16**）——本轮只消费调用方**声明的** `passed` 集合，
  `evaluate_promotion` 从不跑任何用例（这正是它能在无能力包时保持可判定的原因，INV-33）。
- **「按阶段真正启用机制」的机制侧仍未接线**（属 H16/H01）。
- 故 H18 的 `status` 保持 `pending`：判定面 + 落链路径已可测，但「本系统里真实产生一次晋升并落链」
  还不是事实。**如实披露不是缺陷，不实声明才是。**
