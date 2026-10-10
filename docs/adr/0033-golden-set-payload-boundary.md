# ADR 0033：黄金集载荷的边界

- 状态：接受
- 日期：2026-10-10
- 相关：[ADR 0032](0032-knowledge-quality-gate-boundary.md)、[docs/golden-set.md](../golden-set.md)、[tests/rag-golden/README.md](../../tests/rag-golden/README.md)

## 背景

W30 把 §6 的质量门槛与 §5.9 的黄金集生命周期落成 `nai-kernel::golden` 里可执行的判据，但题集**本身**没有落点：§30.1 指定的 `tests/rag-golden` 目录不存在，§6 的 100 题门槛标注为（设计值）且未指定生成方，§5.9 把黄金集列为场景包字段。结果是「100 题黄金集未建」在执行上等价于「什么都可以不做」。

同时暴露一个口径问题：§655 中 B01 的验收是「知识质量：**100 题集达到**第 6 章门槛」——这是**实测达标**。W30 把 registry 的 B01 标为 `implemented`，其准确含义只是「判定机制就位」，与「达标」之间存在落差。

## 决策

### 1. 载荷落点在 `xtask`（tool 层）

`xtask/src/golden.rs` 承担读取与就位门禁，复用 kernel 的 `GoldenSet`/`audit_coverage`/`MINIMUM_CASES`/类别表。

- kernel 只依赖 contracts 且被禁止接触 native 依赖，读文件会破坏分层；
- `nai-knowledge-core` 是 shared 层且不依赖 serde，放进去要么新增依赖、要么复制分位实现；
- tool 层无禁止规则，`xtask` 已依赖 `serde_json`，且**它的 lib 目标就在 portable 门控里**（`cargo test --package nai-xtask --lib`），所以这里的逻辑被工程门控覆盖。

代价是新增依赖边 `xtask → nai-kernel`（tool 层允许，`check-deps` 仍 18 包、方向不变），收益是判定逻辑只有一个真相。

### 2. 载荷格式是 JSON

`xtask` 有 `serde_json`、没有 `serde_yaml`；仓库里唯一的 YAML（`tests/registry.yaml`）由 Node 侧读取，Rust 侧无先例。选 JSON ⇒ **外部依赖零新增**。`schemas/config/*.schema.json` 已确立 JSON Schema 作为结构声明的惯例。

### 3. 「未提供」以文件数为唯一判据

`PayloadAudit::is_unsupplied()` 只看 `files == 0`，**不**由 `sets.is_empty()` 推断。后者在「有文件但全部被拒」时同样为真，会把「载荷是垃圾」误报成「载荷从未提供」。三种结论分开：

| 状态 | 结论 |
| --- | --- |
| 无 `*.json` | `CAPABILITY_NOT_READY`（未提供） |
| 有文件、读不出 | 缺陷，逐条报出 |
| 读出、结构不合 §5.9/§6 | finding，逐条报出 |

### 4. 确认人前移到载荷边界

`origin.kind = promoted` 必须带非空 `confirmer`，缺失或空白在**读取时**即被拒。W30 已在 `BadCasePool::promote` 上要求具名确认人；前移到载荷读取的理由是**失败归因**：若等到运行期，含未确认候选的题集会先参与召回统计，缺陷便可能被读成「系统在某类问题上表现差」。

### 5. B01 回调为 `blocked`，不新增编号

§655 的 B01 验收是「100 题集达到第 6 章门槛」，题集不存在时不可能达成 ⇒ `blocked`。已交付的判定机制与载体在 notes 中完整保留。

**不新增 B01a/B01b**：registry 的 id 正则 `^(S|R|A|B|O|P|X)[0-9]{2}$` 要求恰好两位数字，拆分会直接使 `check --registry` 失败；且拆分等于改写规格 §655 的验收口径，超出本仓库授权。

### 6. 不代造题内容

本变更**不产生**任何题。构造 100 题示例会让人把构造数据读成质量水平。`tests/fixtures/rag-golden/example-min.json` 是**夹具**（8 例最小集，用于验证加载器本身），明确标注不是真值，且不在默认位置故不被默认命令读取。

### 7. 不纳入 portable 门控

工程基础门控不应因业务题集未建而变红——那会混淆两类不同的缺失。

## 否决方案

| 方案 | 否决理由 |
| --- | --- |
| 新增 B01a/B01b 编号 | id 正则要求恰好两位数字，`check --registry` 会失败；且等于改写规格口径 |
| 载荷读取放 kernel | kernel 只依赖 contracts，读文件破坏分层 |
| 载荷读取放 `nai-knowledge-core` | shared 层不依赖 serde；要么新增依赖要么复制分位实现 |
| 题集写进 crate 常量 | 题集是数据不是代码；§30.1 指定了位置；写进 `crates/` 无法「逐场景各一份」 |
| 用 YAML 存题集 | 需新增外部依赖；Rust 侧读 YAML 无先例 |
| 在 xtask 重写覆盖审计 | 会与 kernel 产生两个真相 |
| 就位门禁并入 portable | 工程基础门控因业务题集未建而变红，混淆两类缺失 |
| 自动从日志生成题集 | §5.9 要求人工确认；自动生成会让黄金集自己给自己出题 |
| 造 100 题示例载荷 | 臆造真值，会让人把构造数据读成质量水平 |

## 后果

- 「100 题从哪里来」有了**单一**答案：`tests/rag-golden/*.json`，由场景包产出方按 §5.9 的三条来源填入；门禁按三种结论如实报告。
- 载荷的**结构**缺陷在读取阶段即被拒绝，不会污染运行期的召回统计。
- 仓库的工程门控不受影响（B01 的缺失被隔离在独立命令中）。
- 遗留：真实题集内容、真实模型评估与真实检索质量仍未接入；B01 的「达标」验收仍待外部投入。
