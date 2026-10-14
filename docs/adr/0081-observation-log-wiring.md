# ADR 0081：观测判定之后的**那一步**必须有执行体（H15 runtime 接线）

- 状态：已接受
- 日期：2026-10-14
- 轮次：W103
- 关联：[ADR 0064](0064-openapi-document-set-closure.md)（一处定义、两处使用）、
  [ADR 0068](0068-config-surface-key-consumption.md)（有读取点才算被消费）、
  [ADR 0073](0073-workflow-step-closure.md)（集合必须有站点）

## 背景

V3.11 §47.1 要求 harness 的五类观测「进入只追加的哈希链决策日志**之前**」先被判定。W66 把判定落成了
`crates/nai-kernel/src/evaluate/harness.rs::evaluate_observation`，并在该模块的文档里写死了一句边界：

> Appending it to the actual hash chain in `nai-storage` is runtime wiring and is **not** claimed here.

这句话当时是**诚实**的。问题是它也是**不可核实的**：

```
$ grep -rn 'evaluate_observation\|ObservationRecord\|ObservationPolicy' crates/ apps/
crates/nai-kernel/tests/harness.rs:...     ← 只有测试
```

生产代码**零调用方**。于是两条今天**无法被判**的命题，无论仓库里有多少测试：

1. 一条被判**不可落日志**的记录，会不会**照样**被写进链？（没有调用方 ⇒ 没有答案）
2. 真写进链的那份载荷，是判定输出的**脱敏材料**，还是有人顺手塞了 `record.fields`？

这与 W54（桌面壳只有配置没有壳）、W68（讲品循环只有判定没有循环）同型：**判定面存在、执行体不存在**。
「有判定面」与「有执行体」是两件事，前者是后者的必要条件，不是充分条件——本 ADR 是这条区分在本仓的
第六次现身。

## 决策

### 决策 1：接线落在 `crates/nai-runtime`

接线必须同时看见**判定**（`nai-kernel`）与**只追加哈希链**（`nai-storage`）。`nai-runtime` 同时依赖两者，
且它本来就是「把存储、会话、决策日志、媒体门控装配成一个受监督整体」的那一层。

- 放进 `nai-kernel` ⇒ 引入 kernel→storage 依赖（`cargo xtask check-deps` 会红）；
- 放进 `nai-storage` ⇒ 让存储层认识 harness 的领域类型。

两者都违反既有依赖方向。⇒ 落点是**推出来的**，不是偏好。**零新增依赖**。

### 决策 2：判定与追加**合并为一个入口**，MUST NOT 暴露两个出口

```rust
pub fn record_observation(&mut self, record: &ObservationRecord, policy: &ObservationPolicy)
    -> Result<ObservationOutcome>;
```

若提供「先 `evaluate`、再 `append`」两个出口，「不许把未判定的记录塞进链」这条不变量的责任就
**推给了每个调用方**——一个人忘了先判定，不变量就破了，而**测试看不出来**（没有任何判据的对象存在）。
合并后，**没有**一条路径能不经判定而追加观测记录。

这是本仓反复出现的形状（W92/W93：「把门槛交给被审者自己填」「守卫 MUST NOT 自我满足」）在**接口层**的
又一实例：**不变量要由结构保证，而不是由纪律保证**。

### 决策 3：runtime 是**传输者**，不是**加工者**——载荷恰为判定输出

追加的载荷只有三项：

```json
{ "layer": "<ObservationLayer::label()>", "fields": <verdict.redacted>, "viewer": <verdict.redacted_viewer> }
```

- `fields` MUST 取 `verdict.redacted`（凭据已换 `[redacted]`、超长已换 `[truncated]`），
  **MUST NOT** 取 `record.fields`；
- MUST NOT 添加**任何判定没审过**的字段。故载荷**不带** `record.subject`。

⚠️ **为什么「不带 subject」是决策而非省略**：`subject` 是一个**未经 INV-11 值扫描**的原始字符串。
把它塞进载荷，就开了一条「凭据从 `subject` 溜进日志」的旁路——而判定**永远发现不了**它，因为它根本
不看 `subject`（判定只扫 `fields`）。标识性材料（`fragment_ref`／`skill_id`／`key`／`enum_value`／
`reused_entry_version`）本来就在 `fields` 里，所以不损失信息。

⇒ 一般化：**当 A 只审了对象的一部分，B 不许把未审的那部分补进结果**——那不是「补全」，是把判定
**扩不到的地方**变成**默认可信**。

### 决策 4：不可落日志 ⇒ **具名拒绝**，且**一个字节都不落盘**

`ObservationOutcome::Refused(ObservationRefusal{layer, rejections, redacted})`：层标签 + 逐条拒绝名 +
判定输出的脱敏材料（供查看，**不落盘**）。返回类型是枚举 ⇒ 「已落日志」与「已拒绝」**类型上**可分，
不可能被读成同一个 `Ok`（铁律二：**「没看到」与「出错了」类型上分开**）。

### 决策 5：不先做 O05（升级数据源）——**本机验不了的改动不算「可自主推进」**

R1 的自主项里 O05 体量更小、还能解锁 O02。但 O05 要给 `/upgrades/plan|execute|rollback`
（`schemas/openapi/control.yaml:158/160/162`，已声明、control-api 零实现）接数据源 ⇒ **必然改 OpenAPI 契约**
⇒ 必然触发 `scripts/contracts/generate.mjs` 再生三语言产物；而该再生的校验入口 `cargo xtask schema --check`
在宿主机**跑不了**（`spawnSync(process.execPath)` 报 `EBUSY`，managed/system node 均复现）——**CI-only**。
⇒ 按「交付判定面 ≠ 验收达标」的同一精神：**改一个本机无法验证的契约**不满足「可自主推进」的定义。

H15 则**零契约改动、本机可全程验证**，故本轮先做它。O05 留给后续轮次（CI 观测面在岗，契约再生可由
CI 兜底）。

### 决策 6：`next_trace` 放宽到 `pub(crate)`，**不**新增第二个 trace 生成器

新增模块要复用装配根既有的 trace 序号。选项是「复制一份格式」或「放宽可见性」。后者是本仓既有的
「**一处定义、两处使用**」纪律（ADR 0064）的直接应用：复制一份就会漂移，而漂移的 trace 格式
**没有任何判据会发现**。

## 后果

- 新增 `crates/nai-runtime/src/observation.rs`：公开面只有 `ObservationOutcome` 与 `ObservationRefusal`
  两个类型加一个方法，**不加**便利谓词（见替代方案 6）；
  `crates/nai-runtime/tests/observation.rs`（**8 项**）；`composition.rs` **+2 行注释 + 1 处可见性**；
  `lib.rs` **+2 行**（`mod` 与 `pub use`）。
- 规格：`runtime-composition` **+2 added**；`agent-harness` **1 处 modified**（W66 那条需求末句
  1:1 行替换，其余四段场景逐字不动）。
- `evaluate_observation` 的**判定规则一行未改** ⇒ 判定面与执行面**同轮可分别验证**。
- **非空洞注入四组**（真改 `observation.rs`、真跑、逐字节还原、`sha256` 一致）：
  I1 载荷改取 `record.fields` ⇒ **1 条红**；I2 拒绝分支不再早退 ⇒ **4 条红**；
  I3 只接一类 layer ⇒ **5 条红**；I4 载荷被 `record.subject` 富化 ⇒ **1 条红**。
- **诚实边界**：H15 保持 **`pending`**。本轮可测的是「判定**之后**那一步真的发生且形态正确」；
  **五类观测的真实产生者**（装配裁剪、意图判定、记忆读写、工具调用、缓存命中）**仍未接线**，
  属 H01/H02/H03/H04/B07。⇒ 报告 MUST NOT 出现「五类观测已进决策日志」的措辞。
- 降级侧（`evaluate_degradation` → `CapabilityView`）**不做**，属另一轮呈现接线。

## 替代方案

1. **只补「判定 → 追加」的 happy path，不管拒绝**：`ObservationRejection` 已是具名的，看起来「拒绝那半
   判定函数已经给了」。但判定**给名字**与 runtime**真的不追加**是两件事——W66 的判定函数对**任何**调用方
   都存在，而**没有任何调用方**，所以「拒绝后是否照样落盘」在过去**无从回答**。
2. **暴露 `evaluate` + `append` 两个出口**：更「灵活」，但把不变量交给调用方（决策 2）。
3. **载荷带上 `subject` 以便日志能定位记录**：损失可读性换安全，看起来划算——但 §47.1 的判定
   **从不扫 `subject`**，于是这等于引入一个**永不被判定覆盖**的字节来源（决策 3）。
4. **顺手把 H17 的呈现接线一起做**：那是 `CapabilityView` 与界面，另一层对象；混进来会让本轮的
   证据面变糊（W102 替代方案 6 的同一条理由）。
5. **先做 O05 因为它更小**：见决策 5——「小」不是判据，「本机可验证」才是。
6. **给新模块加 `#[must_use]`/`is_logged()` 等便利 API**：便利 API 会引入**第二个**「判断这条记录
   落没落」的口径；枚举本身已经是可区分的，再加一个谓词就是**同一件事两处定义**。⇒ 不加。

## 关联

- 规格：`openspec/specs/runtime-composition/spec.md` 的两条 added 需求；`agent-harness` 的 1 处 modified
- 变更：`openspec/changes/archive/2026-10-14-w103-observation-log-wiring/`
- 报告：[`docs/reports/2026-10-14-w103-observation-log-wiring.md`](../reports/2026-10-14-w103-observation-log-wiring.md)
- 前序：W66（判定面）、W54（桌面壳）、W68（讲品循环）——同型的「缺失对象」轮次
