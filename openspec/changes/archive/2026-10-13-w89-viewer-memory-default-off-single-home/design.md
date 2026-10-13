# w89 设计：「默认关闭」的单一来源与写入面执行体

## 问题形状

「观众长期记忆默认关闭」在仓库里有**三处载体**，本轮实测它们的**执行体**分布：

| 载体 | 形态 | 执行体 |
| --- | --- | --- |
| 配置面 | `session_defaults.viewer_memory_enabled`（schema `required`） | **0 处**——`composition.config()["session_defaults"]` 全仓只被读一次（`pipeline.rs:874`），且只读同层的 `allow_auto_without_platform_grant` |
| 判定面 | `MemoryPolicy.viewer_memory_enabled` | 读取面 `read_records` ✓；请求准入 `viewer_record_allowed` ✓（但**无生产调用方**）；**写入面 `admit_write` ✗** |
| 台账 | `PD-31` 的推荐「空集 + 默认关闭」 | 「空集」与白名单**指代不同对象**（同形不同义） |

⇒ 一条能力开关**写了三遍、只有一处真的在判**。这正是 W88 在**分派层**（类别表）确立的
「**同一件事 MUST NOT 两处定义**」在**能力开关层**的复现。

## 决策

1. **`viewer_memory_gate(policy)` 是「默认关闭」的唯一判定处。**
   读取面、写入面与请求准入面 MUST 共用它，MUST NOT 各自复写 `if !policy.viewer_memory_enabled`。
   判据形状是 `Result<(), MemoryRejection>`（拒因只有一种：`viewer_memory_disabled`）。

2. **写入面 MUST 也判它**——这是本轮补的**执行体**，不是新增策略。
   「默认关闭」是**能力级**判定（不看具体条目内容就成立），因此 MUST 落在读取面之外**也**落在写入面。

3. **拒因复用 `viewer_memory_disabled`，MUST NOT 新造名字。**
   「写入被拒因为观众记忆默认关闭」与「读取被拒因为观众记忆默认关闭」是**同一个拒因**；
   同一件事有两个名字，读起来像两个不同的问题。⇒ `MemoryRejection::ALL` 仍为 **23**，
   既有总账断言（集合**恰为** `ALL`）**不需要放宽**。

4. **在 `admit_write` 里，能力开关放在「必填项之后、来源判定之前」。**
   位置与读取面的「**结构必填 → 能力开关 → 授权**」同序（`read_records` 先报 `invalid_scope`、再报
   `viewer_memory_disabled`）。理由：能力未启用是**不依赖条目内容**就成立的判定，而「来源不可信」是
   条目级判定；先报能力，调用方才知道该去开开关而不是去换来源。
   **顺序 MUST 有断言**（一条同时「未启用」且「来源不可信」的候选必须报 `viewer_memory_disabled`），
   否则「顺序」只是注释。

5. **`admit_write` 增加 `policy` 入参；MUST NOT 把开关塞进 `MemoryCandidate`。**
   让被审对象自带「能力是否启用」，就是 W57 §四.1 已经排除过的同一反模式（来源门槛「交给被审对象自己填」）。
   `admit_write` 全仓调用方只有本 crate 与其集成测试，签名变更不产生外部影响。

6. **legacy `viewer_record_allowed` 保留它的 `EvaluateError` 契约，只做错误类型映射。**
   它返回 `EvaluateError::Unauthorized` 是 D10 起的对外形状；本轮把**判定**改为调用 gate，
   把 gate 的错误**映射**成 `Unauthorized`。映射不是第二处定义——`if !enabled` 这个**判定**在仓库里只剩一处。
   既有内联单测 `viewer_memory_is_off_by_default` 正好覆盖这条映射。

7. **F1（配置面 required 键零消费）本轮只登记、不改。**
   修它要么给 `viewer_memory_enabled` 接线（改变运行时行为，超出本母题），要么立一条新类别的断言
   「配置面的 required 键 MUST 有消费者」——后者本身就是一个**独立母题**（与 W89 的「判定面收口」不同维）。
   ⇒ 登记为遗留，MUST NOT 用「本轮不动」伪装成「已处理」。

## 假阳性的方向

若只补读取面（现状）而写入面留空，坏值（默认关闭下写成的观众记忆）会**静默通过**；
若给写入面加了过严的判定（例如对所有类别都判开关），合规的非观众写入会**被误拒**。
两者不对称 ⇒ 取**只对 `VIEWER_KIND` 判、且默认拒**的一侧：假阴性（漏放）与假阳性（误拒）之间，
前者是**安全方向的失败**的反面，故本轮选择「未启用即拒」。

## 非空洞注入（F0–F7）

- **F0**：改前基线——`admit_write(&viewer_candidate)` 在默认策略下 **`Ok(())`**（这就是「写入面无执行体」的实测）。
- **F1**：改后同一输入 ⇒ **`Err(ViewerMemoryDisabled)`**。改前/改后**只差这一步**。
- **F2**：**承重证明**——把 gate 从 `admit_write` 移出（只留读取面）⇒ 同一个坏值**再次静默通过**。
- **F3**：**顺序断言**——同一候选「未启用 + 不可信来源」⇒ 报 `viewer_memory_disabled`，
  **不是** `untrusted_source_as_fact`（证明顺序不是注释）。
- **F4**：**反向注入不红**——启用后同一条合规 viewer 候选 ⇒ `Ok(())`；
  非 viewer 类别（`session`／`working`／`semantic`）在默认策略下 ⇒ `Ok(())`（开关 MUST NOT 外溢）。
- **F5**：**拒绝名集合不变**——`MemoryRejection::ALL` 仍 23，既有总账断言原样通过（复用而非新造的直接后果）。
- **F6**：`read_records` 的表现**不变**（默认关闭仍报 `viewer_memory_disabled`），证明收口是**重构 + 补口**、
  不是行为改写。
- **F7**：单一来源的可判性——把 gate 的实现反转（`if !enabled` 写成 `if enabled`）⇒ 读取面与写入面
  **同时**变红 ⇒ 证明两面确实共用一个判定，而不是各自写着看似一样的代码。

## 明确不做

- 不改基线正文（§5.5／§48.7 已定界，本轮是落地）。
- 不改 `tests/registry.yaml`（H03／H04 的 `pending` 由「无持久化、无接线」决定，本轮不改变这件事）。
- 不动运行时（F1 只登记）。
- 不给 `allowed_viewer_fields` 新增取值、也不清空它——它是 D10 沿用的**默认配置**，
  R1 内**不生效**（无接线），本轮只把台账的指代订正清楚。
