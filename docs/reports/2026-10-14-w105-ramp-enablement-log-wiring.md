# W105 轮次报告：H18 的 runtime 接线——判定面**自己算好了记录**，却没人落盘

- 日期：2026-10-14
- 变更：`openspec/changes/2026-10-14-w105-ramp-enablement-log-wiring/`
- ADR：[0083](../adr/0083-ramp-enablement-log-wiring.md)
- 新增模块：`crates/nai-runtime/src/ramp.rs`
- 新增测试：`crates/nai-runtime/tests/ramp.rs`（**9** 项）
- **零迁移、零契约改动**（无新增 `DecisionKind` 变体、无 `schemas/**`/生成物改动）
- 前置：W67（判定面）、W103／W104（同型接线）、R1 自主项排序

## 一、目标

R1 的未达成验收项里，按「瓶颈是否在仓库外」拆出一组**可自主推进**项。W103 做了 **H15**、W104 做了 **H02**，
本轮做与之**完全同型**的 **H18（爬坡门控）**：H18 的 `notes` 自己写着「**仍未接线**——按阶段真正启用机制、
把记录追加进哈希链是 runtime 工作」。

**为什么本轮先做 H18 而不做 H01**：同为候选的 H01 其 runtime 步是「把九段片段**喂给模型**」——那是**真实产生者**，
属 H16/外部。H18 的 runtime 步只是「把判定**已经算好**的记录追加进链」，而判定面**自证**无能力包也保持可判定
（`ramp.rs:28-30` 的 INV-33 原话：*the gate only consumes a declared `passed` set and never runs a case, so
**H18 stays decidable with no capability pack installed***）⇒ **H18 严格更自主**。

## 二、缝在哪

### 2.1 零调用方的直接证据（这一轮更刺眼）

W67 的 `evaluate_promotion` 把边界写在**模块文档**里：

> **Scope boundary.** This module decides *whether a promotion is allowed*. Enabling a mechanism per
> stage at runtime, and appending the record to the hash chain in `nai-storage`, is wiring and is
> **not** claimed here.

实测这句的分量 —— 而 W105 的缝比 W103/W104 **更刺眼**，因为判定面**已经把要落盘的对象构造好了**：

```
$ grep -n 'record_to_append' crates/nai-kernel/src/evaluate/ramp.rs
257:    pub record_to_append: Option<EnablementRecord>,
322:    let record_to_append = match (allowed, req.record) {
$ grep -rn 'record_to_append' crates/ apps/ | grep -v '^crates/nai-kernel'
crates/nai-kernel/tests/ramp.rs        ← 只有测试
```

`EnablementRecord` 是**拥有所有权**的类型（`mechanism: String`、`stage: RampStage`、
`fields: BTreeMap<String, String>`）⇒ `record_to_append` 是一个**可直接序列化进链**的值，
而 `:322` 那个 `match` 分支**专为落盘而写**（`(true, Some(record)) if req.target == AutoSupervised`）。
它的**唯一消费者是测试**。⇒ 两条今天无法被判的命题：

1. 一次被判**放行**的晋升，它算出来的那份启用记录**会不会照样**进链？
2. `record_to_append == None` 的**两条**原因（**被拒** / **放行但阶段不在门控内**）在运行期**不可区分**——
   把 ③ 折进「拒绝」的实现与把它折进「已落日志」的实现，**都能通过全部 13 项 kernel 测试**。

### 2.2 缝型归属

**判定面存在、执行体不存在**——本仓**第八次**（W54 桌面壳 / W68 讲品循环 / W103 观测 / W104 装配之后）。
W105 是它的**加强版**：不是「判定面给了个名字」，而是「判定面**已经把要落盘的东西算成一个 owned 值**」。
这给出一条更锋利的**识别判据**：`grep` 该字段 ⇒ 生产代码零命中 **且** 字段类型是 **owned 而非借用**
**且** 存在一个**只为它而写的构造分支**。

### 2.3 落点是推出来的，不是偏好

| 需要的面 | 位置 | 谁依赖它 |
|---|---|---|
| `evaluate_promotion` / `PromotionRequest` / `RampVerdict` / `RampRejection` | `crates/nai-kernel/src/evaluate/ramp.rs` | `nai-kernel` 导出 |
| `Store::append_decision` / `DecisionKind` / `DecisionAck` | `crates/nai-storage/src/decision.rs` | `nai-storage` 导出 |

`nai-runtime` 同时依赖两者，且本来就是「把存储、会话、决策日志、媒体门控装配成一个受监督整体」的那一层。
放进 kernel ⇒ kernel→storage 越界依赖；放进 storage ⇒ 存储层认识 kernel 领域类型。两者都被
`cargo xtask check-deps` 拦。⇒ **零新增依赖**。

### 2.4 本轮**避开**了 W104 那种「本机不可再生」的面

W104 为装配记录**新增**了 `DecisionKind::Assembly`，代价是一次迁移 + `db_schema_version` 连带面，
而其中的 TS 生成物再生入口 `cargo xtask schema --check` 在宿主机 `EBUSY`、**本机跑不了**
（只能手工同步一个 token 交 CI 复核）。本轮**不产生**这样的面——见 §三决策 2 与 §四。

## 三、设计（详见 [ADR 0083](../adr/0083-ramp-enablement-log-wiring.md)）

### 决策 1：判定 + 追加合并为**唯一**入口

```rust
pub fn record_promotion(&mut self, request: &PromotionRequest<'_>) -> Result<RampOutcome>;
```

两个出口会把「不许把未放行的晋升落进链」的责任**推给每个调用方**（W92/W93「把门槛交给被审者自己填」）。

### 决策 2：结局是**三态**，且**必须**三态（本轮的实质设计决定）

```rust
pub enum RampOutcome {
    Logged(DecisionAck),                                  // 放行且产出了启用记录
    NoRecordRequired { from: RampStage, to: RampStage },  // 放行，但目标阶段在门控之前
    Refused(RampRefusal),                                 // 拒绝
}
```

| 情形 | `allowed` | `record_to_append` | 应当报成 |
|---|---|---|---|
| ① 判定拒绝 | `false` | `None` | **具名拒绝** |
| ② 放行，目标＝受监督自动 | `true` | `Some(rec)` | **已落记录** |
| ③ 放行，目标在门控之前 | `true` | `None` | **无需记录**（非拒绝、非已落） |

把 ③ 塌进两态，无论塌向哪边都**说谎**：报「已落日志」是谎（一个字节没落），报「已拒绝」也是谎
（它没被拒——`ramp.rs:255-256` 的注释原文就是「other stages carry no enablement requirement」）。
**两态能通过全部 kernel 测试**，所以判据里必须有「三态可区分」这一条。

⚠️ **实现纪律**：MUST NOT 用 `record_to_append.is_some()` **反推** `allowed`——那正是 `ramp.rs:11-13`
开头警告的折叠（*Folding the two into one predicate makes "was this refused because it is not enabled by
default, or because a condition was missed" undecidable again*）。运行期 MUST **各读一次**两个字段。
⇒ 注入 **I5** 实测该反推让 **5** 个用例变红（见 §七）。

### 决策 3：复用 `DecisionKind::Approval` ⇒ **零迁移**，且它是**正确**类别

- `approval` 在**两个闭世界**里**都已存在**：Rust 枚举（`decision.rs:31`）与
  `migrations/sqlite/0005-decision-log.sql:1` 的 `kind CHECK(kind IN(…))` ⇒ 不新增变体、不加迁移、
  `migration::VERSION` 保持 **7**、不改 `db_schema_version` 与任何生成物。
- **选它而非 telemetry 类**：`criticality()` 把 `Approval` 归 **Critical** ⇒ 写入失败**原样上报**，
  而不是像 telemetry 那样**静默进有界缓冲**（`ack.persisted == false`）。§47.3 要的是「启用记录已备」，
  一条**可以被静默丢弃**的记录会让这句话不可核实。⇒ 「要么落地、要么被报告，绝不静默跳过」（铁律二）。
- §47.3 的晋升门控本身就是一次**授权**判定 ⇒ 落成 `approval` 类**语义相符**，不是硬塞。

### 决策 4：载荷恰为判定**放行**的那一份材料

```json
{ "mechanism": …, "from": …, "to": …, "fields": { … } }
```

- `mechanism`/`to`/`fields` 取自判定产出的 `record_to_append`（`ramp.rs:297-302` 已断言
  `record.mechanism == req.mechanism`、`record.stage == req.target`）⇒ 这三项是**判定审过**的。
- `from` 取自 `verdict.from`（`NotAnAdvance` 已审过这次转移确为前进）；「前进」是二元关系，
  只写「现在在 auto_supervised」而不写「从 assisted 来」是不完整的证据。
- **MUST NOT 夹带 `req.touched` / `req.passed`**：判定**消费**它们是为了**决定**，它**没有**为落盘审过它们。

### 决策 5：拒绝 ⇒ 具名，且**一个字节都不落盘**

`RampRefusal{rejections, missing_categories, leaked_fields}`：逐条拒绝名 + **全部**缺失回归类别
（§47.3「MUST 列出全部缺失项」）+ 值内含凭据的**字段名**（只报名字、不报值）。返回类型是枚举 ⇒
「已拒绝」与「已落记录」**类型上**可分。拒绝路径**不调用** `self.record(...)`。

## 四、复用一个既有种类：代价与**实测**边界

复用 `approval` 的代价是一个种类承载**两种载荷形状**（审批后端的 `candidate_hash` 组 与 本轮的
`mechanism`/`from`/`to`/`fields`）。两点**实测**「无」才可复用：

**① 有无读者按该 kind 解析载荷** —— 审批读者读的是 `approvals` **表**：

```
crates/nai-storage/src/store.rs:708   approval_valid          ← FROM approvals a JOIN sessions … JOIN utterances …
crates/nai-storage/src/store.rs:734   list_pending_approvals  ← FROM utterances u … LEFT JOIN approvals a …
crates/nai-storage/src/store.rs:802   read_approval           ← 同上
$ grep -rn '"approval"' crates/ apps/ --include=*.rs          ← 其余命中都在无关枚举
  nai-kernel/src/monitoring.rs:60  移动端能力名
  nai-kernel/src/multiplex.rs:75   通道名
  nai-control-api/src/lib.rs:40,96,121  category 串
```

**② 有无守卫枚举该枚举** —— `grep -rln 'DecisionKind' scripts/ tests/contracts/` **空**。

⇒ 无污染。代价已写进 `docs/storage.md`（不让它只存在于代码里）。

## 五、实现

- `crates/nai-runtime/src/ramp.rs`（新增）：`RampOutcome` / `RampRefusal` 两型 + `record_promotion` 一个方法
  + 一个 `refusal_of` 私有映射。**不加**便利谓词（枚举本身已可区分，加谓词＝同一件事两处定义）。
- `crates/nai-runtime/src/lib.rs`：`mod ramp;` + `pub use ramp::{RampOutcome, RampRefusal};`（**+2 行**）。
- `crates/nai-runtime/src/composition.rs`：**一行未改**——直接复用既有的 `record()` 与
  `next_trace("ramp")`（ADR 0027「一处定义」，不新增第二条写入路径与第二个 trace 生成器）。
- `crates/nai-runtime/tests/ramp.rs`（新增，**9** 项）。
- `crates/nai-kernel/src/evaluate/ramp.rs`：**判定规则一行未改** ⇒ 判定面与执行面**同轮可分别验证**。

## 六、测试与非空洞守卫

`crates/nai-runtime/tests/ramp.rs` 九项：

1. 放行晋升落成**恰一条** `approval` 条目，`persisted == true`（critical 不缓冲）、链上恰 +1 行、
   `verify_chain` 通过、无凭据字面量；
2. 载荷**键集恰为** `{fields, from, mechanism, to}`，且**不含**请求携带的回归类别集（`regression.` 零命中）；
3. 目标阶段在门控之前 ⇒ `NoRecordRequired{Rehearsal, Assisted}`，链上零新增；
4. **三态可区分**：同一入口产出三种结局，两两 `assert_ne!`；
5. `ramp.regression_missing` 列出**全部**三个缺失类别（不只第一个），链上零新增；
6. 字段名像密钥（`api_key_name`/`token_label`）而值干净 ⇒ **照旧落盘**（INV-11 按值判、不按名）；
7. 字段值含凭据字面量 ⇒ `Refused` 且 `leaked_fields == ["detail"]`（只报名），链上零新增；
8. **非空链**上的拒绝零新增（让「不变」与「本来就没有」可区分）；
9. **非空洞守卫**：遍历 `RampRejection::ALL`（**8** 条）逐个构造运行期场景，断言拒绝名出现在返回值里
   **且**链上零新增。

⚠️ 守卫的机械性来自**穷尽 `match`**：样本构造函数 `match rejection { … }` 覆盖全部变体 ⇒ kernel 往枚举
加一个变体而 runtime 未处理，**编译即失败**；反向遍历 `ALL` 使「加了变体却漏样本」不可能静默通过。
（**手写清单的盲区恰是清单的补集**，W95 教训。）

## 七、非空洞注入验证

**真改 `crates/nai-runtime/src/ramp.rs` + 真跑 + 备份拷回 + `sha256` 逐字节还原**
（基准＝**过完 `cargo fmt --all` 之后**的那一份：`ramp.rs` `cadb1667…`、`tests/ramp.rs` `419613d2…`、
`lib.rs` `df7c8743…` —— 三份 `OK`）。

| # | 注入 | 实测红数 | 拦的是什么 |
|---|---|---|---|
| I1 | 拒绝分支照旧追加（判定不许时也写链） | **4** | 非空洞守卫 / 缺失类别 / 非空链 / 凭据 |
| I2 | 载荷漏 `record.fields`（落成 `{}`） | **2** | 键集 / 干净值命名 |
| I3 | 目标阶段在门控之前也追加 | **2** | `NoRecordRequired` / 三态可区分 |
| I4 | 载荷夹带 `req.touched` / `req.passed` | **1** | 键集断言是否承重 |
| I5 | 把 `allowed` 折进 `record_to_append.is_some()` | **5** | 凭据 / 三态可区分 / 非空链 / 非空洞守卫 / 缺失类别 |

**复跑基线 9/9 全绿**；注入临时目录 `.w105-probe` 已删除（`git status` 无该目录）。

### 7.1 本轮的方法论产出

- **I5 是本轮最重要的注入**：它注入的不是「少写一行」，而是**§三决策 2 的那条实现纪律**。它让 5 个用例变红
  ⇒ 证明「各读一次两个字段」不是文风要求，而是**被测试钉住的结构**。若只写两态、或允许反推，
  这一整类错误会**静默通过**。
- **I4（1 红）证明键集断言承重**：若只断言「载荷含 mechanism」，夹带未审定字段会**零信号**。
- 各注入的样本定位**按被断言对象的属性**构造（如「值里出现了 `CREDENTIAL_MARKERS[0]` 的样本」），
  **不按「态」定位**（W92 教训）。

## 八、门控与 validate

### 8.1 全量门控 `cargo xtask check --profile full`

**末次 EXIT = 0**。计数**现测**（非推断）：

- **Rust = 94 个目标 / 892 passed / 0 failed**（= **76** `Running` + **18** `Doc-tests`）。
  ⚠️ `Running` 原始 **77** 须减 **1**：门控会先打印一行伪目标
  ``Running `E:/nai-build-target\debug\nai-xtask.exe check --profile full` ``（gate 自执行 xtask 二进制，
  不带 `(…exe)` 括号，且**没有**对应的 `test result:` 行）⇒ `77 - 1 = 76`。
  `test result:` 行数 **94** 与 `76 + 18` 一致。
- **与 W104（93/883）的差逐项对得上**：目标 **+1** 恰为新增的 `crates/nai-runtime/tests/ramp.rs`；
  `passed` **+9** 恰为该文件的 **9** 条 ⇒ **不是「口径变了」**。
- **Node：TAP 根 13 / 317 用例 / 0 fail**（`# pass 317`、`# fail 0`）。本轮**未新增/删除任何 Node 套件或用例**
  ⇒ 与 W104 逐项相同。
- `error[` / `LNK1104` / `out of memory` 命中 **0** 次。
- 门控步骤实测：`cargo fmt --all --check` → `cargo clippy --workspace --all-targets --locked -- -D warnings`
  → `cargo test --workspace --locked --exclude nai-xtask` → `cargo test --locked --package nai-xtask --lib`
  → `node scripts/frontend-gate.mjs`（typecheck + build + test ⇒ `vite` 构建 30 modules ⇒ 0.41 kB html）。

### 8.2 `cargo fmt --all --check` 与 `clippy -D warnings`

**均在门控内通过**（`clippy` 直接 `Finished` 无告警）⇒ 新增模块与测试**无 lint 问题**。

### 8.3 ⚠️ 门控**红了 4 次**，全部是宿主 `LNK1104`（**非代码缺陷**）

| 次序 | 结果 | 失败点 |
|---|---|---|
| 第 1 次 | EXIT=1 | `LNK1104` 打不开 `nai_engine-7bf437cb65f567a6.exe`（`bin "nai-engine" test`） |
| 第 2–4 次 | EXIT=1 ×3 | `LNK1104` 打不开 `nai_engine-c004587559bc4536.exe`（`lib test`） |
| 探针 | **EXIT=0** | `cargo test -p nai-engine --no-run` **链接成功**，且产出的**正是**上面那两个 exe |
| 第 5 次 | **EXIT=0** | 全量门控通过 |

**判据**（沿用 KNOWLEDGE §11）：**逐目标能过 ⇒ 非代码缺陷**。**处置**：**重试、绝不改代码**。
宿主条件已排除「磁盘满」：`E:` 余 **101G**、`E:\nai-build-target` 仅 **6.6G**。
（这也解释了一个现象：每次失败点名的 exe **哈希后缀都不同** ⇒ 是「链接产物打不开」而非「某个固定文件被占」。）

### 8.4 OpenSpec validate（归档前）

`openspec validate 2026-10-14-w105-ramp-enablement-log-wiring --strict --no-interactive` ⇒
`Change '…' is valid`（EXIT=0）。**归档后**另跑两条（见 §8.5）。

### 8.5 ⚠️ W104 的归档教训已**前置**：改前就实测需求正文长度

W104 归档**当场**被 strict 的「需求正文 > 500 字符」规则红过一次，根因是**校验面错位**：
`openspec validate <change> --strict` **不施加** spec 级规则，而本仓 `pnpm spec:validate` 与
`openspec validate --archived` 属 **`not-in-profile`**（本机门控不跑、CI 跑）。
⇒ 本轮**在动笔时就**量：`agent-harness` 那条爬坡需求正文 **改前 436 → 改后 446** 字符（门限 500）。
（实测脚本：按 `### Requirement:` 到首个 `#### Scenario:` 之间**非空行拼接**计数。）

### 8.6 归档与两条 validate（**归档后实测**）

```
$ openspec archive 2026-10-14-w105-ramp-enablement-log-wiring --yes
  ⚠ Why section should not exceed 1000 characters     ← 非阻塞告警（W104 同）
Task status: ✓ Complete                                ← tasks 归档前已全勾
Specs to update: agent-harness: update / runtime-composition: update
  ~ 1 modified / + 2 added
Totals: + 2, ~ 1, - 0, → 0
```

**档案路径**：`openspec/changes/archive/2026-10-14-w105-ramp-enablement-log-wiring/`
（`openspec/changes/` 下**只剩 `archive/`**）。

**归档后立刻**跑四条 validate（**两条是 CI 独有的 `not-in-profile` 面，本机门控不跑 ⇒ 必须显式补跑**）：

| # | 命令 | 结果 |
|---|---|---|
| ① | `openspec validate --specs --strict` | **43 passed, 0 failed**（EXIT=0） |
| ② | `openspec validate --archived` | **103 passed, 0 failed**（EXIT=0；W104 为 102 ⇒ +1 为本次归档） |
| ③ | `openspec validate --all --strict`（= `pnpm spec:validate`） | **43 passed, 0 failed**（EXIT=0） |
| ④ | `openspec validate --changes` | `No items found to validate.`（EXIT=0，符合预期：无活跃变更） |

⚠️ **§8.5 的前置实测生效**：四条**一次全绿**，没有 W104 那种「归档当场 1 红」。

### 8.7 归档后**复跑门控**（归档改动了 `openspec/specs/**`）

按 W104 教训「改 `openspec/specs/**` 之后 MUST 复跑门控」（即便门控不读 spec ⇒ 让「EXIT=0」对应**真正的
提交树**）：复跑 `check --profile full` **attempt 1 EXIT = 0**，计数与 §8.1 **逐项一致**
（Running **77**、Doc-tests **18**、`test result:` **94**、passed **892**、failed **0**、
TAP 根 **13**、`# tests` **317**、`# fail` **0**、`error[`/`LNK1104`/OOM **0**）
⇒ **归档与被改的两份 spec 都没挪动计数**。

⇒ **本轮门控共跑 6 次**（①首轮 + ②③④ 三次重试全红在宿主 `LNK1104` → ⑤绿；⑥归档后复跑绿）。

## 九、CI

见 §十一 回填。

## 十、遗留与边界

- **接线核实**：Rust 集成测试由 `cargo test --workspace` 自动覆盖 ⇒ **无需**新增登记（不写第二处清单）；
  本轮未新增独立 Node 测试文件 ⇒ **无需** `engineering_checks` 登记。
- **诚实边界**：**H18 保持 `pending`**。本轮可测的是「判定**之后**那一步真的发生」；
  - **按阶段真正启用/停用机制的机制侧**（属 **H16/H01**）**仍未接线**；
  - **§47.2 六类回归的执行体**是能力包 **H16**，本轮只消费调用方**声明的** `passed` 集合，
    从不跑任何用例（这正是判定面能在**无能力包**时保持可判定的原因，INV-33）。
  ⇒ 报告 MUST NOT 出现「爬坡机制已接入运行期」「六类回归已执行」这类运行期断言。
  **如实披露不是缺陷，不实声明才是。**
- 下一轮候选（A 组「可自主推进」剩余）：**H01 装配顺序接 runtime**（中，尾部卡 H16）、
  **B07 讲品循环运行时**（中）、**O02 审批面收尾**（中）、**A12 桌面壳主体**（大）、
  **H03+H04 记忆读写子系统**（大）；**H16（能力包）是 H01/H18 的共同外部前置**。
