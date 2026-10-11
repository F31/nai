# W57：记忆读写面（V3.8 §42.2–§42.6；验收 H03/H04）

- 日期：2026-10-11
- 变更：`openspec/changes/w57-memory-read-write-surface/`（归档后为 `openspec/changes/archive/2026-10-11-w57-memory-read-write-surface/`）
- 实现：`crates/nai-kernel/src/evaluate/memory.rs`（**只增不改**）、`crates/nai-kernel/tests/memory.rs`（**21 项**，新增第 **69** 个测试目标）
- 登记：`tests/registry.yaml` 的 **H03／H04**（**状态保持 `pending`**）
- 承接：W56 把 Harness 写进 V3.8 基线并把 H01–H18 登记为 `pending`；W57 是这条轨道上的**第一个实现轮**

## 一、判断：记忆的判定面此前只存在一半

先说结论，再说依据：**仓库里已经有记忆实现，但不是 §42 要的那一半。**

`crates/nai-kernel/src/evaluate/memory.rs` 在 W57 之前**非空**——`working_memory`（只保留已播完、有界）、
`session_summary`（确定性去重）、`viewer_record_allowed`（默认关闭、作用域 ID、用途白名单）、
`viewer_record`（字段/保留期/敏感内容），来自 **D10（V3.5 §23.5）**。所以本轮**不新建模块**，
而是先量出 §42 相对 §23.5 的缺口（实测读码，非印象）：

| §42 条款 | 要求 | 既有实现 | 缺口 |
| --- | --- | --- | --- |
| §42.2 | 只有**已批准且已播完**可写为事实；工具/外部/评论/草稿 MUST NOT 写成事实或承诺 | `working_memory` 按 `played_ids` 过滤 | **没有「写入来源」这一维**——工具返回、模型草稿与正常内容**不可区分** |
| §42.2 | 每次写入必带作用域、来源标识、时间与「叙述/权威事实」标注 | `validate_record` 校验 id/内容/保留期 | 无来源标识、无时间、**无标注**、权威事实无 §6 发布引用约束 |
| §42.3 | 先作用域、后时效；过期/撤销/权限不足不得进入；**不得用近似条目顶替** | 仅 `viewer_record` 的字段级校验 | **没有读取路径**：无作用域过滤、无撤销、无权限、无「近似顶替」概念 |
| §42.3 | 记忆段独立配额；超配额按「场次 > 工作 > 观众」固定优先级裁剪 | 无 | 未实现 |
| §42.4 | 四类触发共用删除链：先阻断读取、再异步清除；删除可验证；第三方留存须明示 | 无 | 未实现 |
| §42.6 | 四项操作 `write`/`read`/`forget`/`scopes` 的语义落进一致性套件 | `MemoryRequest.operation` 字段存在但**无取值约束** | 未实现 |

⇒ 既有实现覆盖的是 §23.5 的**子集**：§42.2 的**来源门槛**、§42.3 的**读取过滤链与配额**、
§42.4 的**删除链**、§42.6 的**四操作语义**在仓库里**零落点**。

## 二、方法：扩展而不是新建，且只增不改

`device.rs`／`demo.rs`／`realistic.rs` 那一类判定骨架都是**新建顶层模块**。本轮**刻意不这么做**：
既有 `evaluate::memory` 已经实现了 §42 的一部分，若再建一个顶层 `memory.rs`，读者会面对
「哪个才是记忆？」的歧义，而未实现的那一半本就是同一件事的剩余部分。故形状是：

- **既有四个函数与 `MemoryPolicy` 一字不改**（原签名、原语义、原内联单测全部保留）；
- 只**增**四块新内容与一个具名拒绝类型。

实测（`git diff --numstat`）：**+748 / −4**，三处 hunk —— 模块文档注释重写（`-4/+21`）、
新增 `SEMANTIC_KIND`（`+2`）、追加新面（`+725`）。**唯一的 −4 就是文件头注释**，
四个既有函数与 `MemoryPolicy` 的函数体**零改动**。

代价（如实登记）：`evaluate` 的模块注释说它是「纯逻辑评估」，而新增的 `admit_write` 更像**接缝函数**。
两者其实一致——都不做 I/O、都只依赖 contracts、都由调用方接入真实子系统。

## 三、交付：四个面与 23 个具名拒绝

**写入（H03）**——`WriteSource`（`approved_played`／`approved_unplayed`／`tool_return`／`peer_return`／
`comment_text`／`model_draft`）、`ClaimKind`（叙述／权威事实）、`MemoryCandidate`、`admit_write`。
判据顺序刻意固定为「先补必填项、再判来源、最后判标注」：**「缺了字段」与「来源不可信」是两件事**，
混成一句「不合规」会让调用方不知道该补字段还是该换来源。

**读取（H04）**——`MemoryScope`／`MemoryQuery`／`ScopedRecord`／`MemoryReadOutcome`／`read_records`。
固定链是「**作用域 → 时效 → 撤销 → 权限 → 近似顶替 → 配额**」。**拒绝不静默**：每一条没进上下文的记录
都在 `refused` 里带着**记录引用与名字**，与「本来就没有」可区分。**裁剪可见**也是刻意的——
`over_budget` 同样是具名拒绝，这是 §44 对工具循环「无静默截断」的同一条纪律在记忆段的落地。

**遗忘（§42.4）**——`ForgetTrigger`（TTL／事实失效／权限撤销／用户请求）／`PurgeState`／`ForgetOutcome`／`forget`。
四类触发共用同一条链；判的是**陈述是否如实**：未清完不得宣称擦除、有第三方留存不得宣称擦除、
受阻态**必须**给出留存说明（否则「说不清」会被读成「已清完」）。

**四操作（§42.6）**——`MemoryOperation` 只接受 `write`/`read`/`forget`/`scopes`，其余是 `unknown_operation`。

`MemoryRejection` 收录 **23** 个具名拒绝，`label()` 互不相同：

| 面 | 拒绝名 |
| --- | --- |
| 写入（7） | `untrusted_source_as_fact`、`unplayed_candidate_as_fact`、`write_without_scope`、`write_without_source_ref`、`write_without_timestamp`、`fact_without_publish_ref`、`fact_outside_semantic_memory` |
| 请求级（6） | `invalid_scope`、`viewer_memory_disabled`、`viewer_read_without_authorization`、`viewer_read_without_purpose`、`approximate_substitution_requested`、`unknown_operation` |
| 条目级（6） | `out_of_scope`、`expired`、`revoked`、`permission_denied`、`approximate_substitution`、`over_budget` |
| 遗忘（4） | `read_side_not_blocked`、`erased_claim_without_complete_purge`、`erased_claim_with_third_party_retention`、`third_party_retention_undisclosed` |

## 四、关键取舍（逐条给理由）

**1. 不可信来源不按 `claim` 放行。** §42.2 的原话是这些来源「MUST NOT 直接写成事实或承诺」。
若按 `claim` 放行，一条 `tool_return` 只要自称「叙述」就能进库，等于把门槛交给被审对象自己填。
故实现取**更严**的一侧：这些来源既非「已批准且已播完」，也非事实来源 ⇒ **一律拒绝**。代码注释里写明这是刻意的从严。

**2. 「不许用近似条目顶替」写成可被拒绝的**请求**。** 若只把它写成注释，请求方永远拿不到反馈。
现在 `MemoryQuery.allow_approximate` 存在且**为真即被拒**（`approximate_substitution_requested`），
条目侧再对「本身就是顶替条目」的候选拒（`approximate_substitution`）——两层都具名。

**3. 配额裁剪**保留原序**，只决定谁被裁。** §42.3 说的是「裁剪的优先级」，不是「装配的排序」。
故保留下来的条目仍按输入序返回，优先级只决定落选者。测试用 budget=2 的四条
（`w1, v1, s1, w2`）证明这不是同义反复：**按优先级**保 `s1`/`w1`、裁 `v1`/`w2`；
**若按输入序取前二**则会保 `w1`/`v1` —— 该用例能区分两者。

**4. 给新面单独一个拒绝类型，不动 `EvaluateError`。** 既有函数继续返回 `EvaluateError`（签名不变）；
新面返回 `MemoryRejection`。理由是把「拒绝」做得**细到可分类**：`EvaluateError` 只有 6 个变体，
若把 23 种情形都压进 `Unauthorized`，「具名」就退化成一句空话（§40 INV-32 要求每层失败须具名）。

**5. `SEMANTIC_KIND` 不参与记忆段读取。** §42.4 的配额优先级只列了「场次 > 工作 > 观众长期记忆」三类；
语义记忆（已发布知识）经 §6 检索，不走记忆段预算。故 `MemoryKind` 只有三类，
测试断言 `MemoryKind::parse_label("semantic") == None`——把这个边界**钉住**，而不是默默放进枚举。

## 五、验收侧：为什么 H03/H04 保持 `pending`

判定机制交付 **≠** 验收达标。H03 的判据是「工具返回、评论原文、模型草稿**不得写成**事实或承诺」，
H04 是「跨作用域读取**被拒**；观众长期记忆默认关闭」——两条描述的**都是运行时行为**，
而**真实的记忆子系统并不存在**：没有持久化、没有控制面接线、没有 Pro 档位的跨会话启用（§42.5 本轮未覆盖）。

这与仓库既有先例完全一致：W32–W36 的判定骨架轮令 `B02`／`O03`／`O04` 保持 `blocked`；
`A12`（桌面壳边界）与 `O01`（演示包判定机制）交付了机制却**均保持 `pending`**；
`A13` 之所以是 `implemented`，是因为它的判据（平台矩阵与源码一致）**自足**，不需要任何运行时。
H03/H04 **不自足**，故状态不变，`notes` 里记下本轮交付了什么、还缺什么。

## 六、验证

| 检查 | 结果 |
| --- | --- |
| `cargo test -p nai-kernel --test memory` | **21 passed / 0 failed** |
| `cargo test -p nai-kernel --lib` | **43 passed / 0 failed** |
| `cargo clippy -p nai-kernel --all-targets --locked -- -D warnings` | **EXIT=0** |
| `cargo fmt --all --check` | **EXIT=0**（先 `cargo fmt --all` 应用，仅动 2 个新文件） |
| `node --test tests/registry.test.mjs` | **3/3 pass** |
| `tests/registry.yaml` | **87 条**（14 A / 6 B / **18 H** / 18 M / 5 O / 1 P / 11 R / 12 S / 2 X）——**与 W56 相同条数**（本轮只改 H03/H04 的 notes/evidence，未增删条目） |
| `openspec validate w57-memory-read-write-surface --strict` | **valid** |
| `cargo xtask check --profile full` | **EXIT=0**；测试目标 **87（69 `Running` + 18 `Doc-tests`）**、**761 passed / 0 failed**（W56 为 86 / 740 ⇒ **+1 目标、+21 项**，正是本轮的集成测试目标） |
| `cargo xtask check-deps` | **19** workspace packages（不变） |
| `openspec validate --all --strict`（归档**前**） | **43/43**（42 specs + 1 活动变更） |
| `openspec validate --all --strict`（归档**后**） | **42/42**（42 specs；活动变更 0） |
| `openspec validate --all --strict --archived` | **62/62**（**只含已归档变更目录、不含 specs**——两条口径互不替代，故本轮两条都跑；W56 教训） |
| `openspec archive -y` 输出 | `+ Task status: ✓ Complete`；`agent-harness: +4 added, ~0, -0`；归档为 `2026-10-11-w57-memory-read-write-surface` |
| CI（推送后匿名复核） | run `—`：`head_sha` 与推送一致、`conclusion: success`、两 job（`ubuntu-24.04` / `windows-latest`）均 success |

**零 Rust 业务改动之外的部分**：本轮**没有**新增依赖、**没有**改能力目录、**没有**改生成物，
workspace 仍 **19 包**；唯一的非测试代码改动就是 `evaluate/memory.rs` 的追加。

## 七、未做与遗留

- **不实现持久化存储**：`MemoryStore` 不在本轮——契约是消息面，落地存储属运行时/存储层。
- **§42.5 跨会话边界未覆盖**：授权/审批/租约/设备身份不跨场继承这一条，等后续轮次。
- **阈值不定夺**：`MemoryPolicy` 的既有默认（内容长度、保留期上限、敏感标记）沿用，不新造取值。
- **H03/H04 保持 `pending`**；H01/H02、H05–H18 仍未开始（W58–W64）。
- **未改 §1–§48 基线正文**：V3.8 已定界，本轮是其落地，不是重新设计。

## 八、方法论教训

**「先量再判」在本轮省掉了一次返工。** 最初的设计是**新建** `crates/nai-kernel/src/memory.rs`
（照搬 `device.rs` 的形状）。动手前先读了一遍既有 `evaluate/memory.rs`，才发现
`working_memory`／`session_summary`／`viewer_record_allowed` 早已存在——若按原设计施行，
就会产出**两个都叫 memory 的模块**，且既有实现会被架空（新增的判定函数与它并存、语义重叠）。
**结论**：判定骨架轮之前，先搜一遍「这件事是不是已经有人做了一半」；`docs/kernel.md` 的模块清单
（D10 一节明写 `memory::working_memory/session_summary/viewer_record_allowed`）就是现成的索引。

**「机制交付 ≠ 验收达标」必须写进 `notes`，否则会被下一个人读成升级。** 沿用 A12／O01／O05 的写法：
`status` 不动、`evidence` 指向本轮报告、`notes` 开头即写明「判定机制已交付，验收保持 `pending`」并给出理由。
