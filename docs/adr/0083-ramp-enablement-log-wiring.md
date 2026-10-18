# ADR 0083：爬坡判定之后的**那一步**必须有执行体（H18 runtime 接线）

- 状态：已接受
- 日期：2026-10-14
- 轮次：W105
- 关联：[ADR 0082](0082-assembly-log-wiring.md)（H02 的同型接线，本轮的模板）、
  [ADR 0081](0081-observation-log-wiring.md)（H15 的同型接线）、
  ~~ADR 0027~~（trace 生成器一处定义；原链接 `0027-trace-id-single-definition.md` 从未存在，见文末「勘误（W151，指针）」）、
  [ADR 0019](0019-composition-root-boundary.md)（critical 与 telemetry 的分级）

## 背景

V3.11 §47.3：harness 机制晋升到受监督自动 MUST 过 §47.2 回归，且 MUST 有一条启用记录。
W67 把这条判定落成了 `crates/nai-kernel/src/evaluate/ramp.rs::evaluate_promotion`，并在该模块的
**模块文档**里写死了边界：

> **Scope boundary.** This module decides *whether a promotion is allowed*. Enabling a mechanism per
> stage at runtime, and appending the record to the hash chain in `nai-storage`, is wiring and is
> **not** claimed here.

这句话当时是**诚实**的。问题是它也是**不可核实**的——而且比 W103/W104 那次更刺眼，因为**判定面自己
把要落盘的记录算好了**：

```rust
// ramp.rs:244
pub struct RampVerdict { …, pub record_to_append: Option<EnablementRecord> }
// ramp.rs:322
let record_to_append = match (allowed, req.record) {
    (true, Some(record)) if req.target == RampStage::AutoSupervised => Some(record.clone()),
    _ => None,
};
```

`EnablementRecord` 是**拥有所有权**的类型（`mechanism: String`、`stage: RampStage`、
`fields: BTreeMap<String, String>`），故 `record_to_append` 是一个**可直接序列化进链**的值，
且构造它的那个 `match` 分支是**专为此写的**。它的唯一消费者是 `crates/nai-kernel/tests/ramp.rs`：

```
$ grep -rn 'record_to_append' crates/ apps/        # 生产代码零命中
```

于是两条今天**无法被判**的命题：

1. 一次被判**放行**的晋升，它算出来的那份启用记录**会不会照样**进链？
2. `record_to_append == None` 的**两条**原因（被拒 / 放行但阶段不在门控内）今天**无法在运行期被区分**——
   一个把后者折进「拒绝」的实现和一个把它折进「已落日志」的实现，**都能通过全部 13 项 kernel 测试**。

这与 W103（H15）、W104（H02）**完全同型**：**判定面存在、执行体不存在**。本 ADR 是这条区分在本仓的
**第八次**现身（W54 桌面壳 / W68 讲品循环 / W103 观测 / W104 装配之后）。

## 决策

### 决策 1：接线落在 `crates/nai-runtime`

接线必须同时看见**判定**（`nai-kernel`）与**只追加哈希链**（`nai-storage`）。`nai-runtime` 同时依赖两者，
且它本来就是「把存储、会话、决策日志、媒体门控装配成一个受监督整体」的那一层。

- 放进 `nai-kernel` ⇒ 引入 kernel→storage 依赖（`cargo xtask check-deps` 会红）；
- 放进 `nai-storage` ⇒ 让存储层认识 kernel 的领域类型（`PromotionRequest`/`RampVerdict`）。

⇒ 落点是**推出来的**，不是偏好。**零新增依赖**。

### 决策 2：判定与追加**合并为一个入口**

```rust
pub fn record_promotion(&mut self, request: &PromotionRequest<'_>) -> Result<RampOutcome>;
```

若提供「先 `evaluate_promotion`、再 `append`」两个出口，「不许把未放行的晋升落进链」这条不变量的责任就
**推给了每个调用方**——一个人忘了先判定，不变量就破了，而**测试看不出来**。这是 W92/W93「把门槛交给
被审者自己填」在**接口层**的又一实例：**不变量要由结构保证，而不是由纪律保证**。

### 决策 3：结局是**三态**，不是两态

```rust
pub enum RampOutcome {
    Logged(DecisionAck),                                  // 放行且产出了启用记录
    NoRecordRequired { from: RampStage, to: RampStage },  // 放行，但目标阶段在门控之前
    Refused(RampRefusal),                                 // 拒绝
}
```

`record_to_append == None` 有**两条**必须区分的原因（`ramp.rs:255-256` 的注释原文：
「other stages carry no enablement requirement」）：

| 情形 | `allowed` | `record_to_append` | 应当报成 |
|---|---|---|---|
| ① 判定拒绝 | `false` | `None` | **具名拒绝** |
| ② 放行，目标＝受监督自动 | `true` | `Some(rec)` | **已落记录** |
| ③ 放行，目标在门控之前 | `true` | `None` | **无需记录**（非拒绝、非已落） |

把 ③ 塌进两态，无论塌向哪一边都**说谎**：报「已落日志」是谎（一个字节没落），报「已拒绝」也是谎
（它没被拒）。两态**能通过全部 kernel 测试**——这就是为什么判据必须加上「三态可区分」这一条。
本决策沿用本项目已记过的形状：「**「记成什么」答不出 ⇒ 先问分类是否少一格**」。

⚠️ **实现纪律**：MUST NOT 用 `record_to_append.is_some()` 在运行期**反推** `allowed`。
反推会把「判定放行」与「判定拒绝」折成一个谓词——正是 `ramp.rs:11-13` 开头警告的那种折叠。
运行期 MUST **各读一次**：先 `verdict.allowed`，再 `verdict.record_to_append`。
（注入 I5 实测：反推会让 **5** 个用例变红。）

### 决策 4：复用 `DecisionKind::Approval`，**零迁移**

W104 给装配记录新增种类，代价是**一次迁移**——因为种类是**两处定义**（Rust 枚举 **和** `0005` 的
`kind CHECK`），只改枚举不改 CHECK ⇒ 插入被 schema 拒绝，且**不响亮**（telemetry 落进有界缓冲、
`ack.persisted == false`）。本轮**不付这个代价**：`approval` 在两个闭世界里**都已存在**。

**为什么 `approval` 是正确类别，不只是「恰好能用」**：`criticality()` 把它归 **Critical**
（与 `Cue`/`Cancellation`/`DutyLease` 同组，文档原话「Authorization, **approval**, cue and duty decisions
are never dropped」，INV-15）。这正好对上 §47.3 的「启用记录已备」：telemetry 的写入失败是**静默进缓冲**，
一条能**被静默丢弃**的启用记录会让「已备」无法核实；critical 的写入失败**原样返回错误** ⇒
「要么落地、要么被报告，绝不静默跳过」（铁律二）。且 §47.3 的晋升门控本身就是一次**授权**判定。

**代价如实披露**：一个种类承载两种载荷形状（审批后端的 `candidate_hash` 组 与 本轮的
`mechanism`/`from`/`to`/`fields`）。这是本轮唯一的语义代价，可接受，因为（实测）没有读者按 kind 解析载荷：

```
crates/nai-storage/src/store.rs:708  approval_valid         ← 读 approvals 表
crates/nai-storage/src/store.rs:734  list_pending_approvals ← 读 utterances ⨝ approvals
crates/nai-storage/src/store.rs:802  read_approval          ← 同上
$ grep -rln 'DecisionKind' scripts/ tests/contracts/        ← 空：没有守卫枚举种类
```

`docs/storage.md` 已就此补一句，不让这个事实只存在于代码里。

### 决策 5：载荷恰为判定**放行**的那一份材料

```json
{ "mechanism": …, "from": …, "to": …, "fields": { … } }
```

- `mechanism`/`to`/`fields` 直接取自判定产出的 `record_to_append`——`ramp.rs:297-302` 已断言
  `record.mechanism == req.mechanism` 且 `record.stage == req.target`，故这三项是**判定审过**的。
- `from` 取自 `verdict.from`——判定通过 `NotAnAdvance`（`target.index() > current.index()`）**审过**
  这次转移确为前进，而「前进」是二元关系：只写「现在在 auto_supervised」而不写「从 assisted 来」
  是不完整的证据。
- **MUST NOT 夹带 `req.touched` / `req.passed`**：判定**消费**它们是为了**决定**，它**没有**为落盘审过
  它们；把它们一并落账就是把 runtime 从**传输者**改成**加工者**，也把会随修订变化的回归状态钉进链。
- `fields` 的**值**已由判定按 `CREDENTIAL_MARKERS`（INV-11）扫过 ⇒ 落盘行必不含凭据字面量。

### 决策 6：拒绝 ⇒ **具名**，且**一个字节都不落盘**

`Refused(RampRefusal{rejections, missing_categories, leaked_fields})`：逐条拒绝名 + **全部**缺失回归类别
（§47.3「MUST 列出全部缺失项」）+ 值内含凭据的**字段名**（只报名字、不报值）。返回类型是枚举 ⇒
「已拒绝」与「已落记录」**类型上**可分（铁律二：**「没看到」与「出错了」类型上分开**）。
拒绝路径**不调用** `self.record(...)`。

### 决策 7：为什么不先做 H01——**本机验不了的改动不算「可自主推进」**

沿用 ADR 0081 决策 5 的同一判据。同为「可自主推进」候选的 **H01**（装配顺序接 runtime）其 runtime 步是
**把九段片段喂给模型**——那是**真实产生者**，属 H16/外部。H18 的 runtime 步只是「把判定**已经算好**的记录
追加进链」，而判定面**自证**「无能力包也保持可判定」（`ramp.rs:28-30` 的原话：
「the gate only consumes a declared `passed` set and never runs a case, so **H18 stays decidable with no
capability pack installed (INV-33)**」）⇒ H18 严格更自主。

## 后果

- 新增 `crates/nai-runtime/src/ramp.rs`：公开面只有 `RampOutcome`（**三态**）/ `RampRefusal` 两型
  + 一个方法，**不加**便利谓词（枚举本身已可区分，加谓词＝同一件事两处定义）；
  `crates/nai-runtime/tests/ramp.rs`（**9 项**）；`lib.rs` **+2 行**；
  `composition.rs` **一行未改**（`record()` 与 `next_trace()` 直接复用，ADR 0027「一处定义」）。
- **零新增依赖、零迁移、零契约改动**：`migration::VERSION` 保持 **7**，不改 `schemas/**`、不改任何生成物、
  不改 `db_schema_version` ⇒ **不产生** W104 那种「本机不可再生的 TS 生成物」面。
- 规格：`runtime-composition` **+2 added**；`agent-harness` **1 处 modified**（W67 那条爬坡需求**末句**
  1:1 追加一句指向新落点，其余正文与全部 **5** 个场景逐字不动）。改前正文 **436** 字符、改后 **446**
  （strict 的 `>500` 规则在 W104 归档当场红过一次，故本轮**改前就实测**）。
- `evaluate_promotion` 的**判定规则一行未改** ⇒ 判定面与执行面**同轮可分别验证**。
- **非空洞注入五组**（真改文件、真跑、逐字节还原、`sha256` 一致，复跑基线 **9/9** 全绿）：
  I1 拒绝分支照旧追加 ⇒ **4 红**（非空洞守卫／缺失类别／非空链／凭据）；
  I2 载荷漏 `record.fields` ⇒ **2 红**（键集／干净值命名）；
  I3 门控之前的阶段也追加 ⇒ **2 红**（`NoRecordRequired`／三态可区分）；
  I4 载荷夹带 `touched`/`passed` ⇒ **1 红**（键集）；
  I5 把 `allowed` 折进 `record_to_append.is_some()` ⇒ **5 红**（决策 3 的纪律）。
- **宿主机实测**：`LNK1104`（`link.exe` exit 1104）在本轮注入期间命中 **4** 次；判据＝**重试即可过**
  ⇒ **非代码缺陷**，处置是**重试、绝不改代码**（与 KNOWLEDGE §11 既有记录一致）。
- **诚实边界**：H18 保持 **`pending`**。本轮可测的是「判定**之后**那一步真的发生」；
  **按阶段真正启用/停用机制的机制侧**（属 H16/H01）与 **§47.2 六类回归的执行体**（**H16**，能力包）
  **仍未接线**。⇒ 报告 MUST NOT 出现「爬坡机制已接入运行期」「六类回归已执行」这类运行期断言。

## 替代方案

1. **只补 happy path，不管拒绝**：判定已给出具名拒绝，看起来「拒绝那半已经做了」。但判定**给名字**与
   runtime**真的不追加**是两件事——W67 的判定对**任何**调用方都存在，而**没有任何调用方**。
2. **两态 `Logged`/`Refused`**（把「无需记录」折进任一边）：省一个枚举变体，但让运行期**说谎**
   （决策 3）。⇒ 采纳三态。
3. **用 `record_to_append.is_some()` 反推 `allowed`**：一行更短，但把两个问题折成一个谓词
   （`ramp.rs:11-13` 的原文警告），且注入 I5 实测 **5 红**。⇒ 不采纳。
4. **新增 `DecisionKind::RampEnablement` + 迁移 `0008`**：语义更干净，但带来 W104 那整条
   `db_schema_version` 连带面——`TS_GENERATED_SOURCES` 的再生入口 `cargo xtask schema --check` 在宿主机
   `EBUSY`、**本机跑不了**，只能手工同步一个 token 并交 CI 复核。**本机验不了的面**正是「可自主推进」要避开的，
   而复用 `approval` 同时拿到**正确的 critical 语义**（决策 4）。⇒ 不采纳。
5. **复用某种 telemetry 类（如 `Decision`）**：省下迁移，但写入失败会**静默进有界缓冲**，
   一条可被静默丢弃的启用记录让 §47.3 的「已备」不可核实。⇒ 不采纳。
6. **载荷连 `touched`/`passed` 一起落账**：看起来「证据更全」，但那是判定**消费**而非**审定**的材料，
   且把会随修订变化的状态钉进链（决策 5）。⇒ 不采纳。
7. **顺手把 H01 的装配产生者一起做**：另一层对象（真实产生者在仓库外），混进来会让证据面变糊
   （W102 替代方案 6 的同一条理由）。⇒ 不做。

## 关联

- 规格：`openspec/specs/runtime-composition/spec.md` 的两条 added 需求；`agent-harness` 的 1 处 modified
- 变更：`openspec/changes/archive/2026-10-14-w105-ramp-enablement-log-wiring/`
- 报告：[`docs/reports/2026-10-14-w105-ramp-enablement-log-wiring.md`](../reports/2026-10-14-w105-ramp-enablement-log-wiring.md)
- 前序：W67（判定面）、W103／W104（同型接线）、W54（桌面壳）、W68（讲品循环）——同型的「缺失对象」轮次

## 勘误（W151，指针）

关联列表里的 `ADR 0027` 原链接到 `0027-trace-id-single-definition.md`，**该文件从未存在**。
`git log --follow docs/adr/0027-deployment-lifecycle-boundary.md` 显示 ADR 0027 自创建起就是
「部署生命周期的边界」，与 trace 生成器无关。W151 新增的链接判据（ADR 0125）把它报成断链，
处置是**降级为纯文本**，不是改指一个编号：

- 「trace 生成器一处定义」的实际落点是 **ADR 0081 决策 6**——「`next_trace` 放宽到 `pub(crate)`，
  **不**新增第二个 trace 生成器」，它应用的是 ADR 0064「一处定义、两处使用」；
- **编号不可复原**：原文写的 `0027` 与任何 trace 相关的 ADR 都不对应 ⇒ MUST NOT 猜一个上去。
  猜错会把一条断链换成一个**看起来能点、内容不对**的指针，那比断链更坏；
- 正文里同源的「ADR 0027『一处定义』」写法（本 ADR 的正文段）指涉同一件事，同样不可复原，
  保留原文不改写——权威表述是本段。
- `ADR 0015` 原链接到 `0015-decision-log-criticality.md`，同样**从未存在**：ADR 0015 是
  「V3.6 升为唯一实现基线」。「critical 与 telemetry 的分级」的正文出处是 **ADR 0019 决策 5**
  （`docs/adr/0019-composition-root-boundary.md:25` 逐字：「决策日志分级：critical
  （cancellation/duty_lease 等）失败即错误；telemetry 允许缓冲」）⇒ 该处已**就地订正**为
  ADR 0019。这是两类处置的差别：**指错了但指得回来 ⇒ 订正**；**指的东西不存在 ⇒ 降级 + 勘误**。
