# Design — W103 观测判定接上真实哈希链（H15 runtime 接线）

## 1 缝在哪里

W66 交付了 `evaluate_observation`（判定），并在模块文档里写死了一句边界：

> Appending it to the actual hash chain in `nai-storage` is runtime wiring and is **not** claimed here.

实测这句话的分量：`grep -rn 'evaluate_observation\|ObservationRecord\|ObservationPolicy' crates/ apps/`
在 **生产代码**里零命中，只有 `crates/nai-kernel/tests/harness.rs` 命中。于是两条今天**无法被判**的命题：

1. 一条被判不可落日志的记录，**会不会照样**被写进链？（没有调用方 ⇒ 没有答案）
2. 真写进链的那份载荷，**是不是**判定输出的脱敏材料，还是有人顺手塞了 `record.fields`？

这正是本项目反复出现的「**判定面存在、执行体不存在**」缺口——与 W54（桌面壳）、W68（讲品循环）同型。
本轮把它补上，判据就从「人读代码」变成「测试能判」。

## 2 为什么落在 `crates/nai-runtime`

接线需要同时看见**判定**与**只追加哈希链**，而这两个能力分属两个 crate：

| 需要的面 | 位置 | 谁依赖它 |
|---|---|---|
| `evaluate_observation` | `crates/nai-kernel/src/evaluate/harness.rs` | 由 `nai-kernel` 导出 |
| `Store::append_decision` / `DecisionKind::Observation` | `crates/nai-storage/src/decision.rs` | 由 `nai-storage` 导出 |

`crates/nai-runtime/Cargo.toml` 同时持有 `nai-kernel`（path）与 `nai-storage`（path）——**这是唯一
既能看到判定、又能看到追加的落点**，并且它本来就是「把存储、会话、决策日志、媒体门控装配成一个受监督
整体」的那一层（`crates/nai-runtime/src/lib.rs` 头注释）。放在 kernel 里会引入 kernel→storage 的越界依赖，
放在 storage 里会让存储层认识 harness 的领域类型；两者都违反既有依赖方向（`cargo xtask check-deps` 强制）。

**零新增依赖**：`nai-kernel` 与 `nai-storage` 都已在 `nai-runtime` 的依赖表里。

## 3 为什么不先做 O05（升级数据源）

建议顺序里 O05 排在第一（小、解锁 O02）。但勘察后判**本轮避开**，理由是**本机验不了**：

- O05 要给 `/upgrades/plan|execute|rollback`（`schemas/openapi/control.yaml:158/160/162`，已声明、control-api
  零实现）接上数据源 ⇒ 必然改 OpenAPI 契约 ⇒ 必然触发 `scripts/contracts/generate.mjs` 再生三语言产物；
- 该再生的校验入口是 `cargo xtask schema --check`，而它在宿主机**跑不了**（`spawnSync(process.execPath)`
  报 `EBUSY`，managed/system node 均复现）——**CI-only**。

按本项目纪律，「改一个本机无法验证的契约」不能算「可自主推进」。H15 则**无契约改动、本机可全程验证**，
所以本轮先做它。O05 留给后续轮次（CI 观测面在岗，契约再生可在 CI 兜底）。

## 4 接口形状：判定 + 追加合并为一处

```rust
// crates/nai-runtime/src/observation.rs
pub struct ObservationRefusal {
    pub layer: &'static str,                 // verdict.layer.label()
    pub rejections: Vec<&'static str>,       // 逐条拒绝名，顺序同判定输出
    pub redacted: BTreeMap<String, String>,  // 判定输出的脱敏材料（**未被落盘**）
}

pub enum ObservationOutcome {
    Logged(DecisionAck),
    Refused(ObservationRefusal),
}

impl Composition {
    pub fn record_observation(
        &mut self,
        record: &ObservationRecord,
        policy: &ObservationPolicy,
    ) -> Result<ObservationOutcome>;
}
```

**为什么把「判定」和「追加」放在同一个方法里**，而不是暴露「先 `evaluate` 再 `append`」两个出口：
后者把「不许把未判定的记录塞进链」这条不变量的责任**推给了每个调用方**——只要有一个人忘了先判定，
不变量就破了，而测试还看不出来（这正是 W92/W93 那类「把门槛交给被审者自己填」的形状）。
合并成一个入口后，**没有**一条路径能不经判定而追加观测记录。

## 5 载荷的形状：runtime 是传输者，不是加工者

判定为可落日志时，追加的载荷只有三项：

```json
{ "layer": "<ObservationLayer::label()>", "fields": {<verdict.redacted>}, "viewer": <verdict.redacted_viewer> }
```

不变式：

- **载荷 MUST 恰为判定输出的已脱敏材料**。`fields` 取 `verdict.redacted`（凭据值已换成 `[redacted]`、
  超长值已换成 `[truncated]`），**MUST NOT** 取 `record.fields`。
- **runtime MUST NOT 添加任何判定没审过的字段**。故此形状**不带** `record.subject`——它是一个未经
  INV-11 值扫描的原始字符串，把它塞进载荷就开了一条「凭据从 subject 溜进日志」的旁路；而标识性材料
  （`fragment_ref`／`skill_id`／`key`／`enum_value`／`reused_entry_version`）本来就在 `fields` 里。
  一句话：**runtime 只搬它收到的、已审过的那一份，不做加法。**

## 6 非空洞守卫（四组注入，缺一不可）

沿用 W54/W55/W66 的做法：**真改 `crates/nai-runtime/src/observation.rs` + 真跑 + 逐字节还原 +
`sha256` 一致**。四条注入**各自**必须让某个用例变红（括号内为实测结果）：

| # | 注入 | 实测红的用例 | 拦的是什么 |
|---|---|---|---|
| I1 | 载荷改取 `record.fields.clone()` | 1 条：载荷形状 | 只钉「能不能进日志」而不钉「**以何形态**进」的实现 |
| I2 | 拒绝分支不再早退（拒绝的记录照旧追加） | 4 条：三条拒绝 + 可区分性 | 把「已拒绝」与「已落日志」混成同一个 `Ok` |
| I3 | 只接一类 layer（其余四类早退） | 5 条：含五类完整性 | 手写层名清单的盲区 |
| I4 | 载荷被 `record.subject` 富化 | 1 条：未审字段 | runtime 做加法 ⇒ 把判定**没审过**的字节送进链 |

**I3 的样本定位 MUST NOT 按「态」定位**（W92 的教训）：五类的断言遍历 `OBSERVATION_LAYERS` 自己，
而不是遍历一张手写的层名清单——否则往枚举里加一类而忘了加样本时，判据**静默地不认识它**，
I3 也就不会红。

**I4 是本轮的设计选择带出来的守卫**：既然 §5 决定 runtime **不做加法**，就必须有一条用例真的把
「加法」这条错误实现抓出来；否则 §5 那句「MUST NOT 添加判定没审过的字段」只是散文。故 I4 的
样本不是「凭据藏在字段里」（那会被判定拦住，与 runtime 无关），而是**凭据藏在 `subject` 里而字段干净**
——判定放行、runtime 若富化就泄漏，正是能区分两种实现的那一例。

## 7 诚实边界

- 本轮的判据是「**判定之后**的那一步真的发生了，且形态正确」。**五类观测的真实产生者**（装配裁剪、
  意图判定、记忆读写、工具调用、缓存命中）**仍未接线**，属 H01/H02/H03/H04/B07。
- 故 H15 的 `status` 保持 `pending`：判定面 + 落链路径已可测，但「五类观测在本系统里真实产生并落链」
  还不是事实。**如实披露不是缺陷，不实声明才是。**
- 降级侧（H17 的 `evaluate_degradation` → `CapabilityView`）本轮**不做**：那是呈现接线，另一轮。
