## 1. 参考答案是必填字段，形态取最小

### D1：每个用例必须声明 `expected`

题集 schema 修订 2 要求每题带 `expected`，形态固定为：

```json
"expected": { "kind": "reference", "must_include": ["...", "..."], "must_exclude": ["..."] }
```

- `kind` 目前只有 `reference` 一个取值，保留 kind 是因为既有 `expectation` 也这样做，且将来若引入别的判据形态
  不必再改结构。
- `must_include` **至少一条**。允许为空会让「没有任何要求」自动判为正确或永远未判定，两种都是在把判定悄悄关掉。
- `must_exclude` 可为空（缺省即空）。

### D2：修订从 1 升到 2，**不做双读**

`case_set_schema` 的语义是「本构建认识的唯一修订」，不是「至少能认识的修订」。若同时接受 1 与 2，一份没写 `expected`
的旧题集会被读成「没有参考答案要求」而不是「结构不合规」——静默改变用例的含义，正是 ADR 0033 决策 2 拒绝未知字段的理由。
因此修订 1 在修订 2 落地后**被拒**，夹具同步升级。

## 2. 判定是口径，落在 kernel

### D3：`judge_answer` 是纯函数

```rust
pub fn judge_answer(expected: &ExpectedAnswer, answer: &str) -> AnswerVerdict
```

- **归一化**：Unicode 小写折叠 + 连续空白折叠为单个空格 + 去首尾空白。对中文，小写折叠无副作用，空白折叠
  解决换行/缩进造成的假不匹配。
- **判定顺序**：任一 `must_exclude` 命中 ⇒ `Incorrect`（安全侧优先）；否则全部 `must_include` 命中 ⇒ `Correct`；
  否则 ⇒ `Incorrect`。
- 子串匹配而非相似度阈值：阈值需要选一个数，那个数本身就是判据，且不可复现。子串是确定性的。

### D4：判定产出既有的 `AnswerVerdict`，`evaluate` 签名不变

`judge_answer` 返回 `AnswerVerdict`，因此 `Submission`/`evaluate`/`QualityReport` **一个字段都不用改**，
`Unknown` 从正确率分母排除的既有语义也原样继承。这样判据的引入不会改变 W30 已交付的判定口径，
`crates/nai-kernel/tests/golden.rs` 的既有断言不受影响（只是构造点要补新字段）。

## 3. 执行体是编排，落在 xtask 的 example

### D5：`xtask/examples/golden_eval.rs`，不入 lib

对照既有先例 `crates/nai-provider-core/examples/live_provider_smoke.rs`：真实链路（需要凭据/网络）在仓库里
一律放 `examples/`，不进测试、不进工程门控。

三条理由：

1. **解析器只有一个真相**。题集读取在 `xtask/src/golden.rs`，同 crate 的 example 直接 `use`，不必为「跑评估」
   再造第二个解析器（ADR 0033 否决过「两个真相」）。
2. **不进 lib ⇒ portable 门控的覆盖与耗时都不变**。真实接线要 `nai-provider-core`（reqwest/async）与
   `nai-storage`，这些都是 `[dev-dependencies]`，只在构建 example 时才编译。
3. **零新增包**。已有的 tool 层无依赖限制（ADR 0033 决策 1），不需要再造一个 app 层包。

### D6：缺前置报 `CAPABILITY_NOT_READY`，**拒绝用桩替代**

| 情况 | 行为 |
| --- | --- |
| `tests/rag-golden` 无 `*.json` | `CAPABILITY_NOT_READY`，退出 1，**不发起任何调用** |
| 结构不合规 | 逐条报出缺陷，退出 1 |
| 缺少 LLM 环境变量 | `CAPABILITY_NOT_READY`，列出变量**名**（不打印值），退出 1 |
| 真实调用失败 | 退出 2 |

一个让冒烟变绿的演练桩，正是 §30.1 禁止的东西。凭据只从环境读取，**绝不打印、绝不写进证据文件**——
与 `live_provider_smoke.rs` 同一纪律。

### D7：检索侧不得伪造

执行体**不**在没有真实知识库时填一个空的 `retrieved` 再声称「召回为 0」。真实知识库属外部条件（④），
此时按既有口径记为**未测量**：`evidence_recall_at_5` 的分母为 0 ⇒ 该字段进 `QualityReport::unmeasured`，
`retrieval_p95_ms` 的样本不足 ⇒ `None`。「没测过」与「测到 0」类型不同，这一点 W30 已经钉住，执行体不得绕过。

### D8：`xtask` 新增的 `[dev-dependencies]` 不改变分层

`tool` 层在 `check-deps` 的规则里没有禁止项（`_ => false`），且 `nai-provider-core`（provider）与
`nai-storage`（shared）本身都不禁止被 tool 依赖。`check-deps` 的读数应从 18 包不变——
**不新增 workspace 成员**。

## 4. 边界与不做

### D9：B01 仍是 `blocked`

本轮把①④之外的两段补齐，达标仍不可能达成（题集内容与真实知识库未提供）。registry 记明缺口从四段降到两段，
**状态不变**。冒烟与「执行体就位」都不是达标（§30.1 与 `live-provider-evidence` 规格对此已有明条）。

### D10：不代造题、不新增编号

夹具仍是 8 例构造数据并如实标注不是真值。不新增 `B01a/B01b`（id 正则要求恰好两位数字，且等于改写 §655 口径）。

## 否决方案

| 方案 | 否决理由 |
| --- | --- |
| 判定放 example（不进 portable） | 判据是口径不是编排；放进 example 会失去工程门控覆盖 |
| schema 1 与 2 双读 | 会静默改变旧题集含义（ADR 0033 决策 2 的同一理由） |
| `must_include` 允许为空 | 「没有要求」会自动判正确或永远未判定，等于关掉判定 |
| 用 LLM-as-judge 判正确率 | 用待评模型评自己，效力需要另一套论证；本轮取确定性规则，不引入新凭据 |
| 参考答案用相似度阈值 | 阈值本身是判据且不可复现；子串匹配确定性 |
| 新建 `apps/golden-eval` | 第 19 个包，且解析器会成为第二真相 |
| 执行体并入 `check --profile portable` | 业务题集未建不应让工程基础门控变红（与 ADR 0033 决策 7 同理） |
| 造 100 题夹具当样例 | 臆造真值，会让人把构造数据读成质量水平 |
| 在评测里临时构造一份知识库充当「真实检索」 | 那是夹具，不是真实知识库；会让人把演练读成质量水平 |
