## Context

A11 要成立，必须回答四个问题：这段逻辑**放哪**；「无串场」靠**什么**成立；「共享只限三类」靠
**什么**成立；「资源不足拒绝新增、不伤已开播」靠**什么**成立。四个答案都要能在 portable 下被证伪。

基线：V3.6 §30.4 A11、§12.3（隔离、共享面、分族准入）；与 W25 的 `deployment-lifecycle`（能力驻留与档位）相邻但**不同维度**。

### 与 W25 的关系

W25 的 `Residency`/`ResourceKind`（`Process | Port | Vram | CloudRequest`）回答的是**能力开不开、驻留占什么**；
本 capability 的 `AdmissionFamily`（`GpuThroughput | Vram | AudioTiming | ApiQuota`）回答的是**会话能不能进、进来占多少配额**。

一个能力可以已经驻留（占着显存），而新会话仍因显存配额满被拒——两者是两个层次的事实，不是同一条判断的两个写法。
名字上 `Vram` 重合是物理世界的重合（都是显存），不是模型的重复：一个是「持有」（声明式账本），一个是「请求」（准入两阶段）。

## Decisions

### D1 放进 `crates/nai-kernel`，不放进 shared 层

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 放进 `nai-upgrade-core`（shared） | 与 W25 的驻留账本同住 | **否决**：kernel 层**不允许**依赖 shared 层（`xtask/src/deps.rs`：`kernel => target.layer != "contracts"`），反过来把会话语义放进 shared 则要 shared 依赖 kernel，同样违规。且「多会话」是内核会话语义的直接延伸（`session.rs` 已定义单会话 epoch 与撤销）。 |
| B. 新增 `nai-session-core` crate | 多会话独立成包 | **否决**：新增包会改动 workspace 组成与依赖方向（18 包），而本 capability 只依赖 `ErrorCode` 与 `std`——为一段纯逻辑增加一个包并让依赖图变长，成本大于收益。 |
| C. 放进 `nai-kernel`（新增 `multiplex.rs`） | 与 `session.rs` 并列，只依赖 contracts | **采用**：18 包不变，依赖方向不变，全部判据在 portable 下可测（`xtask test --suite kernel`）。 |

kernel 只能依赖 contracts，因此本模块自带一个 `MultiplexError { code, reason }`（`code` 用 `ErrorCode`，`reason` 是稳定 token）。
它与 `nai-upgrade-core` 的 `UpgradeError` 同形，是分层约束下不可避免的一处重复——但形状一致，报告仍可统一分类。

### D2 lane 访问以**属主**为键

`access(requester, owner, lane)`：requester ≠ owner 即拒绝。

| 方案 | 结论 |
|---|---|
| 一张全局 lane 表 + 进入时检查 owner | **否决**：检查可以被漏写在一个新的调用点，且漏写不会让任何测试变红。以属主为键则**没有**绕过属主的路径。 |
| 把 lane 做成范型参数（类型级隔离） | **过度**：会在 kernel 里引入大量范型噪声，而运行时键已经让跨会话不可达。 |
| lane 只做「会话内寻址」不设属主 | **否决**：那样「跨会话读取被拒」这条判据没有落点，无串场退化成一句设计意图。 |

### D3 共享面是**封闭枚举 + 无写路径**

`SharedKind` 三个取值即白名单；`SharedPool::write` 永远拒绝。

| 方案 | 结论 |
|---|---|
| `allow_share: bool` 或运行时白名单检查 | **否决**：布尔与检查都会被绕过或漏写；枚举让「第四类可共享资源」**表示不出来**。 |
| 允许写入共享快照（「小心一点」） | **否决**：规格说的是**只读**知识快照与**不可变**模型；没有写路径，才是「只读」的实现。 |
| 连接池不设上限（「共享池自动扩容」） | **否决**：规格说的是**受控**连接池；上限为零在构造时即拒，避免第一次 acquire 才发现池不可用。 |

### D4 准入是**分族 + 两阶段**

先按族汇总请求、逐族对照全机总量检查，**全部通过才写入**。

| 方案 | 结论 |
|---|---|
| 把四族合成一个总量 | **否决**：直接违反「分别准入」——显存告急没有任何理由让 API 配额也停摆。 |
| 逐族 check-then-commit（边查边写） | **否决**：一次请求里可满足的族会先写入，随后另一族失败，于是拒绝留下了副作用；**已开播会话**会看到机器被一个「失败的请求」占了一部分。 |
| 未声明的族按「不限制」处理 | **否决**：保守读法是零；未声明预算按不限制处理，会让一次配置遗漏变成一次静默放行。 |

## Verification

- `cargo test -p nai-kernel --test multiplex`：24 项（命名空间 9、共享 5、准入 6、审计 2、lane 常量 1、其余边界）。
- `cargo xtask test --suite kernel`：`nai-kernel` + `nai-plugin-sdk` 全量（含本轮 `multiplex.rs`）。
- `cargo xtask check --registry`、`cargo xtask check --profile portable`、`openspec validate w27-multi-session-isolation --strict`。

## Environment boundaries

- 准入用的是**宿主给定的上限表**，本模块不探测真实 GPU/显存/配额；「机器还剩多少」由调用方负责测。
- `automation: integration` 的**真实多路会话**（同机并发写实会话、真实音频时序）属设备里程碑，本轮未做，故 registry 只到 `implemented`。
- 不做跨节点会话路由与多会话 UI。
