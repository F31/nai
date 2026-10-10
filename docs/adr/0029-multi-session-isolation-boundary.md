# ADR 0029：多会话隔离的边界——以属主为键的 lane、封闭只读共享、两阶段分族准入

- 日期：2026-10-10
- 状态：Accepted
- 基线：V3.6 §30.4 A11、§12.3（隔离 / 共享面 / 分族准入）；§31.4；承接 ADR 0027（部署生命周期）、ADR 0028（本地自治）
- 变更：`openspec/changes/w27-multi-session-isolation/`
- 判据：`tests/registry.yaml` A11「Pro 多会话隔离」

## 背景

A11 是一个**结构性**要求而不是一个功能：§30.4 说「任务、音频、审批、预算、知识无串场；资源不足时拒绝新增会话」，
§12.3 把它展开成三句——每会话状态以 `session_id` 隔离并各有独立 epoch；共享只限不可变模型、只读知识快照与受控连接池；
GPU 吞吐、显存、音频时序、API 配额**分别准入**，新会话容量不足时拒绝，**不能让已开播会话 OOM**。

这三句都在要求「某件坏事**不会**发生」：不会串场、不会被写、不会半提交。这种要求最容易在文档里被写成承诺，
而在代码里退化成一个可以被漏写的检查。所以本轮的重点是**让坏事在类型上不可表达**。

四个问题必须先回答：放**哪个 crate**；「无串场」靠**什么**成立；「共享只限三类」靠**什么**成立；
「拒绝不伤已开播」靠**什么**成立。

## 决策

### D1 放进 `crates/nai-kernel`，不放进 shared 层，不新开 crate

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 放进 `nai-upgrade-core`（shared） | 与 W25 驻留账本同住 | **否决**：kernel **不允许**依赖 shared 层（`xtask/src/deps.rs`：`kernel => target.layer != "contracts"`）；反过来把会话语义塞进 shared 又要 shared 依赖 kernel，同样违规。 |
| B. 新增 `nai-session-core` | 多会话独立成包 | **否决**：改动 workspace 组成与依赖方向（18 包），而本模块只依赖 `ErrorCode` 与 `std`。为一段纯逻辑让依赖图变长，成本大于收益。 |
| C. 放进 `nai-kernel` 的 `multiplex.rs` | 与 `session.rs` 并列，只依赖 contracts | **采用**：18 包不变，依赖方向不变，`xtask test --suite kernel` 在 portable 下可测。 |

代价：kernel 只能依赖 contracts，故自带 `MultiplexError { code, reason }`。它与 `nai-upgrade-core` 的 `UpgradeError` 同形——
分层约束下一处可接受的重复，形状一致因而报告仍可统一分类。

### D2 lane 访问以**属主**为键

`access(requester, owner, lane)`：requester ≠ owner 即 `CONFLICT("lane belongs to another session")`。

| 方案 | 结论 |
|---|---|
| 全局 lane 表 + 进入时检查 owner | **否决**：检查会被漏写在新调用点上，且漏写不会让任何测试变红。 |
| 类型级隔离（lane 带会话范型参数） | **过度**：在 kernel 里引入大量范型噪声，而运行时键已让跨会话不可达。 |
| lane 不设属主（只做会话内寻址） | **否决**：「跨会话读取被拒」这条判据没有落点，无串场退化成设计意图。 |

### D3 共享面是**封闭枚举 + 无写路径**

`SharedKind` 三值即白名单；`SharedPool::write` 永远拒绝；连接池有持有者上限。

| 方案 | 结论 |
|---|---|
| `allow_share: bool` / 运行时白名单检查 | **否决**：布尔与检查都会被绕过或漏写；枚举让「第四类可共享资源」**表示不出来**。 |
| 允许写共享快照 | **否决**：规格说只读知识快照与不可变模型；没有写路径才是「只读」的实现。 |
| 连接池不设上限 | **否决**：规格说**受控**连接池；上限为零在构造时即拒，避免首次 acquire 才发现池不可用。 |

### D4 准入是**分族 + 两阶段**

先按族汇总、逐族对照全机总量检查，**全部通过才写入**。

| 方案 | 结论 |
|---|---|
| 四族合成一个总量 | **否决**：直接违反「分别准入」——显存告急没有理由让 API 配额也停摆。 |
| 逐族 check-then-commit | **否决**：可满足的族会先写入，另一族失败后拒绝留下副作用，已开播会话会看到机器被一个「失败的请求」占了。 |
| 未声明族按不限制 | **否决**：保守读法是零；按不限制处理会让一次配置遗漏变成一次静默放行。 |

### D5 与 W25 驻留账本是两个维度，不是两条真相

W25 的 `Residency`（`Process | Port | Vram | CloudRequest`）回答能力**开不开、驻留占什么**；本模块的 `AdmissionFamily`
（`GpuThroughput | Vram | AudioTiming | ApiQuota`）回答会话**能不能进、进来占多少配额**。一个能力可以已经驻留（占着显存），
而新会话仍因显存配额满被拒。`Vram` 重合是物理世界的重合（都是显存），不是模型重复；两者键与语义都不同（声明式持有 vs 请求-准入）。

## 后果

- 「无串场」「共享只限三类」「拒绝不半提交」三条分别落成：以属主为键的签名、封闭枚举 + 无写路径、两阶段准入。
- 代价：准入需要宿主提供真实的上限表与请求量；本模块不探测硬件。这是有意的——把「机器还剩多少」与「这次要多少」显式化，才谈得上审计。
- 会话数上限（64）与控制面既有上限（审批 200、命令 256）同属有界队列的取向。

## 验证

- `cargo test -p nai-kernel --test multiplex` = 24 passed。
- `cargo xtask test --suite kernel`、`cargo xtask check --registry`、`cargo xtask check --profile portable`（fmt + clippy `-D warnings` + workspace 测试）。
- `openspec validate w27-multi-session-isolation --strict --no-interactive`。

## 未做边界

- 准入用**宿主给定的上限表**，不接真实 GPU/显存/配额探针。
- `automation: integration` 的真实多路会话（同机并发写实会话、真实音频时序）属设备里程碑。
- 不做跨节点会话路由与多会话 UI。
