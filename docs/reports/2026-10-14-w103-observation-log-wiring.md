# W103 轮次报告：H15 的 runtime 接线——判定面之后的**那一步**必须有执行体

- 日期：2026-10-14
- 变更：`openspec/changes/2026-10-14-w103-observation-log-wiring/`
- ADR：[0081](../adr/0081-observation-log-wiring.md)
- 新增模块：`crates/nai-runtime/src/observation.rs`
- 新增测试：`crates/nai-runtime/tests/observation.rs`（8 项）
- 前置：W66（判定面）、R1 自主项排序（用户指令「按顺序执行可自主推进任务」的第一条）

## 一、目标

R1 的 16 条未达成验收项里，按「瓶颈是否在仓库外」拆出一组**可自主推进**项，并按工作量排序。
本轮执行排序建议的第一条——**H15（harness 观测五类记录进决策日志）**。选它的理由是**可验证性**，
不是体量：它**零契约改动、本机可全程验证**（对比见 §2.5 为何 O05 被避开）。

## 二、缝在哪

### 2.1 零调用方的直接证据

V3.11 §47.1 要求五类观测「进入只追加的哈希链决策日志**之前**」先被判定。W66 把判定落成了
`crates/nai-kernel/src/evaluate/harness.rs::evaluate_observation`，并在该模块的文档里写死：

> Appending it to the actual hash chain in `nai-storage` is runtime wiring and is **not** claimed here.

这句话当时**诚实**。问题是它同时**不可核实**：

```
$ grep -rn 'evaluate_observation\|ObservationRecord\|ObservationPolicy' crates/ apps/
crates/nai-kernel/tests/harness.rs:…        ← 只有测试
crates/nai-kernel/tests/…                   ← 只有测试
（生产代码零命中）
```

⇒ 两条今天**无法被判**的命题，无论仓库里有多少测试：

1. 一条被判**不可落日志**的记录，会不会**照样**被写进链？
2. 真写进链的那份载荷，是判定输出的**脱敏材料**，还是有人顺手塞了 `record.fields`？

### 2.2 缝型归属

与 **W54**（桌面壳只有声明式配置、没有壳）、**W68**（讲品循环只有判定、没有循环）**同型**：
**判定面存在、执行体不存在**。「有判定面」是「有执行体」的**必要条件**，**不是**充分条件——
本 ADR 是这条区分在本仓的又一次现身。

### 2.3 落点是推出来的，不是偏好

接线必须**同时**看见两样东西：

| 需要的面 | 位置 |
|---|---|
| `evaluate_observation` | `crates/nai-kernel/src/evaluate/harness.rs` |
| `Store::append_decision` / `DecisionKind::Observation` | `crates/nai-storage/src/decision.rs` |

`crates/nai-runtime/Cargo.toml` 同时依赖 `nai-kernel`（path）与 `nai-storage`（path），且它本来就是
「把存储、会话、决策日志、媒体门控装配成一个受监督整体」的那一层（`src/lib.rs` 头注释）。
放 `nai-kernel` 会引入 kernel→storage 越界依赖；放 `nai-storage` 会让存储层认识 harness 的领域类型；
两者都会让 `cargo xtask check-deps` 红。**零新增依赖**。

### 2.4 为什么这是「R1 自主项」里该先做的一条

H15 满足「可自主推进」的全部条件：Rust、无契约改动、判定逻辑一行不用改、本机可跑完整门控。
对比 R1 里体量更小的一些项（O02/O05），它们都要动契约或界面，取证面在仓库外。

### 2.5 本轮**避开**先做 O05（升级数据源）

建议顺序里 O05 排在首位（体量小、还能解锁 O02）。勘察后判**避开**，理由是**本机验不了**：

- O05 要给 `/upgrades/plan|execute|rollback`（`schemas/openapi/control.yaml:158/160/162`，已声明、
  control-api 零实现）接数据源 ⇒ **必然改 OpenAPI 契约**；
- 该再生的校验入口 `cargo xtask schema --check` 在宿主机**跑不了**
  （`generate.mjs` 的 `spawnSync(process.execPath)` 报 `EBUSY`，managed/system node 均复现）——**CI-only**。

⇒ **「本机验不了的改动」不满足「可自主推进」的定义**（「小」不是判据，「本机可验证」才是）。

## 三、设计（详见 [ADR 0081](../adr/0081-observation-log-wiring.md)）

### 3.1 判定与追加合并为一个入口

```rust
pub fn record_observation(&mut self, record: &ObservationRecord, policy: &ObservationPolicy)
    -> Result<ObservationOutcome>;
```

不暴露「先 `evaluate` 再 `append`」两个出口——那会把「不许追加未判定的记录」**推给每个调用方**：
一个人忘了先判定，不变量就破了，而**测试看不出来**（没有任何判据的对象存在）。
⇒ **不变量 MUST 由结构保证，不是由纪律保证**（W92/W93「把门槛交给被审者自己填」在接口层的又一实例）。

### 3.2 runtime 是**传输者**，不是**加工者**

```rust
{ "layer": verdict.layer.label(), "fields": verdict.redacted, "viewer": verdict.redacted_viewer }
```

- `fields` MUST 取 `verdict.redacted`（凭据已换 `[redacted]`、超长已换 `[truncated]`），
  **MUST NOT** 取 `record.fields`；
- MUST NOT 添加**任何判定没审过**的字段 ⇒ 载荷**不带** `record.subject`。

⚠️ 判定只扫 `fields`，**`subject` 永不被审**。把 `subject` 塞进载荷就是开一条「凭据从 `subject`
溜进日志」的旁路，而判定**永远发现不了**它。标识性材料（`fragment_ref`／`skill_id`／`key`／
`enum_value`／`reused_entry_version`）本来就在 `fields` 里，不损失信息。
⇒ 一般化：**当 A 只审了对象的一部分，B 不许把未审的那部分补进结果**——那不是「补全」，
是把判定**扩不到的地方**变成**默认可信**。

### 3.3 不可落日志 ⇒ 具名拒绝，且一个字节都不落盘

```rust
pub enum ObservationOutcome { Logged(DecisionAck), Refused(ObservationRefusal) }
pub struct ObservationRefusal { layer: &'static str, rejections: Vec<&'static str>,
                                redacted: BTreeMap<String, String> }
```

返回类型是枚举 ⇒「已落日志」与「已拒绝」**类型上**可分，不可能被读成同一个 `Ok`（铁律二：
「没看到」与「出错了」类型上分开）。`redacted` 随拒绝返回**供调用方查看**，但**不落盘**。

### 3.4 不新增第二个 trace 生成器

`composition.rs` 的 `next_trace` 从私有**放宽**到 `pub(crate)`（+2 行注释），而不是复制一份格式：
复制会漂移，而漂移的 trace 格式**没有任何判据会发现**（ADR 0064「一处定义、两处使用」）。

## 四、实现

| 文件 | 改动 |
|---|---|
| `crates/nai-runtime/src/observation.rs` | **新增**：`ObservationRefusal`、`ObservationOutcome`、`Composition::record_observation` |
| `crates/nai-runtime/src/lib.rs` | `mod observation;` + `pub use observation::{ObservationOutcome, ObservationRefusal};`（+2 行） |
| `crates/nai-runtime/src/composition.rs` | `next_trace` 可见性 + 2 行注释（**无行为改动**） |
| `crates/nai-runtime/tests/observation.rs` | **新增** 8 项 |
| `openspec/specs/runtime-composition/spec.md` | +2 added（可落日志 ⇒ 追加脱敏载荷；不可落日志 ⇒ 具名拒绝且链不动） |
| `openspec/specs/agent-harness/spec.md` | 1 处 modified（W66 那条需求末句 **1:1 行替换**，其余四段场景逐字不动） |

`evaluate_observation` 的**判定规则一行未改** ⇒ 判定面与执行面**同轮可分别验证**。

**接线登记**：本轮的测试是 Rust 集成测试，**不需要**在 `xtask/src/lib.rs::engineering_checks` 或
`scripts/contracts/test-reachability.mjs` 登记——portable 面用 `cargo test --workspace --locked
--exclude nai-xtask`（`xtask/src/lib.rs:126`）覆盖全部 workspace 成员的集成测试目标。
（W95/W97 的「被测面集合守卫」会从 workspace 成员清单**派生**这一步，故**不引入第二处硬编码清单**。）

## 五、非空洞验证

### 5.1 四组注入（备份拷回 + 真跑 + `sha256` 逐字节还原）

方法：`cp` 备份 → 按**唯一锚点**（脚本断言 `count == 1`）做字节级替换 → `cargo test -p nai-runtime
--test observation` → `cp` 拷回 → 核对 `sha256`。基准是**已过 `cargo fmt --all` 的、即将提交的那一份**
（`observation.rs` = `ba2ae763ac5ed5d1c14a067508d452650a69d36fb1dae9979bcf611c998609d2`）。
基线：`8 passed / 0 failed`。

| # | 注入 | 实测红的用例 | 红数 | 拦的是什么 |
|---|---|---|---|---|
| I1 | 载荷改取 `record.fields.clone()` | `the_appended_payload_is_the_judged_material_not_the_raw_record` | **1** | 只钉「能不能进日志」而不钉「**以何形态**进」的实现 |
| I2 | `if !verdict.loggable` → `if false`（拒绝的记录照旧追加） | 三条拒绝用例 + `a_refusal_is_distinguishable_from_an_append` | **4** | 把「已拒绝」与「已落日志」混成同一个 `Ok` |
| I3 | 早退条件加 `\|\| verdict.layer.label() != "harness.assembly"` | 含 `each_of_the_five_layers_reaches_the_log_as_a_judged_entry` | **5** | 手写层名清单的盲区 |
| I4 | 载荷插入 `"subject": record.subject` | `the_runtime_does_not_enrich_the_payload_with_unvetted_fields` | **1** | runtime 做加法 ⇒ 把判定**没审过**的字节送进链 |

四次拷回后 `sha256` 全部与基准一致（脚本末尾 `sha256sum` 两行相同）。

### 5.2 两条方法论（本轮踩到并写进 ADR）

- ⚠️ **I3 的样本定位 MUST NOT 按「态」定位**（W92 的教训）：五类的断言**遍历 `OBSERVATION_LAYERS`
  自己**，MUST NOT 遍历手写的层名清单——否则往枚举里加一类而忘加样本时，判据**静默地不认识它**。
- ⚠️ **I4 是设计选择带出来的守卫**：样本**不是**「凭据藏在字段里」（那会被判定拦住、与 runtime 无关），
  而是**凭据藏在 `subject` 里而字段干净**（判定放行、runtime 若富化就泄漏）——正是能**区分两种实现**
  的那一例。若不把 §3.2 的「不做加法」落成这条守卫，它就是一句散文。

### 5.3 宿主边界（如实）

注入脚本用 3 次重试包裹每次 `cargo test`：I1／I2／I3 各命中一次宿主间歇 `LNK1104`
（`无法打开文件 …observation-*.exe`），重试即过；I4 一次通过。按既有判据
「**逐目标能过 ⇒ 非代码缺陷、别改代码**」处置（限并发、短重试、不 sleep 循环硬刷）。
⚠️ **`cargo fmt --all` 会改写 `observation.rs`** ⇒ 注入的基准 MUST 是**格式化之后**的那一份，
否则「逐字节还原」还原的是**一个不会被提交的版本**（本轮首次取基准时正踩到这点，故重跑了一遍）。

## 六、门控与 validate

### 6.1 全量门控

`cargo xtask check --profile full`（`CARGO_BUILD_JOBS=1` 限并发）：**EXIT=0**。

```
cargo fmt --all --check                                            ✓
cargo clippy --workspace --all-targets --locked -- -D warnings      ✓
cargo test --workspace --locked --exclude nai-xtask                 ✓
cargo test --locked --package nai-xtask --lib                       52 passed
（10 个 Node 契约套件 + workflow-triggers 15/15；前端 workspace 步骤）
```

**计数（现测；权威口径 = `grep -c '^test result:' <门控日志>`）**：
**92 目标 / 870 passed / 0 failed**（W102 = 91/862）。
目标 **+1** 恰为本轮新增的集成测试目标 `tests/observation.rs`；`passed` **+8** 恰为其 8 条用例
⇒ **计数变化逐项对得上**（不是「口径变了」）。

### 6.2 宿主瞬态（如实记录，非代码缺陷）

前三次门控尝试均在 `cargo test --workspace` 阶段撞 `LNK1104`，且三次命中的是 `nai-engine` 的
**三个不同**链接目标（`pipeline-*.exe` / `nai_engine-*.exe` / `engine-*.exe`）；
`tasklist` 查无残留 `nai-engine.exe`／`keeper.exe`／`render-worker.exe`。按既有判据
「**逐目标能过 ⇒ 非代码缺陷，别改代码**」处置：改为**先带重试地预热链接**
（`cargo test --workspace --locked --exclude nai-xtask --no-run`，第 1 次 `LNK1104`、第 2 次即过），
再跑门控 ⇒ **一次即绿**。**本轮未改任何代码来绕过它。**

### 6.3 `cargo fmt --all --check`

第一次 `--check` 就报 `crates/nai-runtime/src/observation.rs` 与 `tests/observation.rs` 需要调整
（`use` 排序、`self.record(…)` 折行折叠、`assert_eq!` 换行）⇒ 跑 `cargo fmt --all` 后
`git status` 确认**只有本轮的 7 个文件**被改，无关文件零改动。
⚠️ **这一步改写了 `observation.rs`** ⇒ 注入的基准随之**重取**并重跑四组注入（见 §5.1／§5.3）。

### 6.4 OpenSpec validate

- 变更级：`openspec validate 2026-10-14-w103-observation-log-wiring --strict --no-interactive`
  ⇒ `Change '…' is valid`
- 归档后另跑 `--all --strict` 与 `--all --strict --archived`（结果见 §七）。

## 七、CI

单次低频观测（**唯一查询参数**破坏缓存 + `Cache-Control: no-cache` + **回读 `head_sha`**，**不轮询**；
全程 6 次请求，远低于匿名配额 60/h）。

- 推送：`edbd906fb552577d343d08f5c4abc32537299352`（`HEAD == origin/main`）
- run：**#139**（id `38526858347`，`Engineering foundation`，event `push`）
- **结论：`completed` / `success`**；回读 `head_sha` == 推送 SHA ✅
- 双平台：`foundation (ubuntu-24.04)` **success**；`foundation (windows-latest)` **success**
- 与本轮相关的关键步骤（两平台）：
  `Check generated contracts without modifying consumers` ✓、
  `Complete engineering foundation (portable steps + frontend workspace)` **✓ 两平台均过**
  （⇒ 本轮新增的 Rust 集成测试、`fmt --check`、`clippy -D warnings` 在 CI 侧同样通过）、
  `Validate schemas and API contract` ✓、`Full three-language contract suite (Linux C++ toolchain)` ✓、
  `pnpm spec:validate` ✓、`pnpm exec openspec validate --archived --no-interactive` ✓
- ⚠️ **本机绿 ≠ CI 绿**：CI 的两个步骤本机跑不了（`schema --check` 与 `contracts` 三语言套件，
  宿主 `node` 派生 `EBUSY`／不在 portable 面内），故上面以 **CI 为准**；本机的 `92 目标 / 870 passed`
  是 portable 面的口径，与 CI 的步集合**不同**，两者**不互相代替**。

## 八、遗留

1. **五类观测的真实产生者**（装配裁剪、意图判定、记忆读写、工具调用、缓存命中）仍未接线，
   属 H01/H02/H03/H04/B07。本轮交付的是「判定**之后**那一步」。
2. **降级侧**（`evaluate_degradation` → `CapabilityView` 与界面）未接线，属 **H17** 的另一轮。
3. **O05（升级数据源）**被本轮避开（§2.5）：需要动 OpenAPI 契约，而契约再生的校验入口
   `cargo xtask schema --check` 在宿主机 `EBUSY`（CI-only）⇒ 留给后续轮次。
4. 沿用既有的非本轮遗留（不主动清理）：W90 §八 1/3/4/5；W91 跨语言键级消费；W92 基线收录；
   W93 锚点唯一性；W94 `needs_external` 审计；W95 `ci-only` 成因；W96 五项；W97 六项；
   W98 §八 1/2/7；W99 §八 1/2/3；W100 §八 2/3/4；W101 §八 1/2/3/4/5；W102 §八 1/2/3/4/5/6/7。

## 九、如实披露

- **H15 保持 `pending`**。本轮可测的是「判定**之后**那一步真的发生且形态正确」；
  「五类观测在本系统里**真实产生**并落链」还不是事实（§八 1）。
  ⇒ 本报告 MUST NOT 出现「五类观测已进决策日志」的措辞；`tests/registry.yaml` 的 H15 只追加
  `notes`，**`status` 不动**。
- 同轮**不升级**任何验收项：交付判定面 ≠ 验收达标。
- 判定侧**一行未改**，故本轮的「判定面仍由 W66 承担」这一归属没有被改写；
  `agent-harness` 规格里那条需求的末句已就地 1:1 行替换为指向 runtime 侧新落点。
