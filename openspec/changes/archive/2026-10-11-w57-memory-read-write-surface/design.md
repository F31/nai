# 设计（w57 记忆读写面）

## 1 为什么是「扩展 `evaluate::memory`」而不是新建模块

仓库里已经有两个可能的落点：新建 `crates/nai-kernel/src/memory.rs`（与 `device.rs`／`demo.rs` 同形），
或扩展 `crates/nai-kernel/src/evaluate/memory.rs`。

选后者，理由是**避免两个都叫 memory 的模块**。既有 `evaluate::memory` 已经实现了 §42 的一部分语义
（工作记忆只保留已播完、场次摘要确定性去重、观众长期记忆默认关闭且需作用域/用途/授权），
并且 `docs/kernel.md` 的 D10 一节把它登记为「内存记忆评估」。若新建一个同名的顶层 `memory.rs`，
读者会面对「哪个才是记忆？」的歧义，而未实现的那一半（读取过滤链、配额、删除链）其实**是同一件事的剩余部分**。

因此本轮的形状是：**既有函数一字不改**（`working_memory`／`session_summary`／`viewer_record_allowed`／
`viewer_record`／`MemoryPolicy` 全部保留原签名与原语义），**只增**四块新内容。既有内联单测不受影响。

代价（如实登记）：`evaluate` 模块的文档注释说它是「纯逻辑评估」，而新增的 `admit_write` 更像一个**接缝函数**。
两者其实一致——它们都不做 I/O、都只依赖 contracts、都由调用方把结果接入真实子系统。

## 2 写入面（H03）：缺的是「来源」这一维

既有 `working_memory` 的判断依据是 `played_ids`：**内容有没有播完**。但 §42.2 的门槛有两个正交维度——
**播没播完**，以及**从哪来**。工具返回、外部智能体返回、评论原文、模型草稿这四类来源，即使「看起来已经播过」，
也 MUST NOT 写成事实或承诺（INV-09／INV-24／INV-31 都指向同一件事）。既有类型里**没有表达来源的字段**，
`MemoryRecord` 是生成的 proto，加字段会破坏契约冻结——所以来源必须挂在**候选写入**这个新类型上：

```
MemoryCandidate { record: MemoryRecord, source: WriteSource, claim: ClaimKind,
                  source_ref: String, written_at_utc_ms: u64, publish_ref: Option<String> }
```

`admit_write` 的判据（每条对应一个具名拒绝）：

1. `source` ∈ {工具返回、外部智能体返回、评论原文、模型草稿} ⇒ `untrusted_source_as_fact`（INV-31：外部返回是数据，不是事实）。
2. `source == 已批准未播出` ⇒ `unplayed_candidate_as_fact`（§5.7 检查点纪律：未播出的生成文本不得写成事实）。
3. `source_ref` 空 ⇒ `write_without_source_ref`；`workspace_id`/`session_id` 空 ⇒ `write_without_scope`；
   `written_at_utc_ms == 0` ⇒ `write_without_timestamp`（§42.2 的「每次写入必带」）。
4. `claim == 权威事实` 且 `publish_ref` 为空 ⇒ `fact_without_publish_ref`（§42.2：要成为事实必须走 §6）。
5. `claim == 权威事实` 且 `kind != 语义记忆` ⇒ `fact_outside_semantic_memory`
   （工作记忆是「最近播完的内容」、场次记忆是「本场流水」，两者都不承载权威事实）。

## 3 读取面（H04）：顺序链 + 具名拒绝 + 可见裁剪

§42.3 的关键词是**顺序**（「先按作用域过滤，再按时效过滤」）与**不得静默**（「MUST NOT 用近似条目顶替」）。
既有代码没有读取路径，所以这一段是从零设计，但仍受三条纪律约束：

- **请求级拒绝**（在碰记录之前就失败）：作用域不完整 ⇒ `invalid_scope`；观众长期记忆未启用 ⇒ `viewer_memory_disabled`；
  观众读取缺授权引用 ⇒ `viewer_read_without_authorization`；缺用途记录 ⇒ `viewer_read_without_purpose`；
  请求方要求「允许近似顶替」⇒ `approximate_substitution_requested`（把「不许顶替」写成**可拒绝的请求**，
  而不是一句注释）。
- **逐条拒绝**（写在 `refused` 里，带记录引用与名字）：越作用域 ⇒ `out_of_scope`；过期 ⇒ `expired`；
  已撤销 ⇒ `revoked`；权限不足 ⇒ `permission_denied`；本身是顶替条目 ⇒ `approximate_substitution`；
  超出配额被裁 ⇒ `over_budget`。

「裁剪可见」这一点是刻意的：§44 对工具循环要求「无静默截断」，记忆配额同理——一条记录被裁掉是因为
**它被裁掉了**，这应当是一个可审计的事实，而不是「本来就没有」。故 `over_budget` 也进 `refused`，
且裁剪按固定的 `场次记忆 > 工作记忆 > 观众长期记忆` 优先级发生（先保场次，最后才动观众长期记忆）。

## 4 遗忘链（§42.4）：可验证的意思是不许宣称超过实际

§42.4 把「可验证」落在两件事上：**读取侧先失效**，**清理侧给出完成状态**。再加一条 §24.4 借来的纪律：
**第三方留存不可清除时必须明示政策，MUST NOT 宣称已完全擦除**。

`forget(outcome)` 的判据：

- `read_side_blocked == false` ⇒ `read_side_not_blocked`（先阻断检索与读取，顺序不能反）。
- `claims_fully_erased == true` 且 `purge_state != Complete` ⇒ `erased_claim_without_complete_purge`。
- `claims_fully_erased == true` 且存在第三方留存 ⇒ `erased_claim_with_third_party_retention`。
- `purge_state == BlockedThirdParty` 且未给出留存说明 ⇒ `third_party_retention_undisclosed`
  （阻断态**必须**带说明，否则「说不清」会被读成「已清完」）。

## 5 四项操作（§42.6）：把冻结的语义钉在操作名上

`memory.access@1` 的 `stage` 已是 `frozen`（§42.6 已登记它与 §20.6 正文的不一致，以目录为准）。
§42.6 要求四项操作 `write`/`read`/`forget`/`scopes` 的语义落进一致性套件。故本轮加 `MemoryOperation`
与 `parse`，把**操作名**变成受约束的取值：四项以外的操作名 ⇒ `unknown_operation`，
而不是「静默当作未知操作」。`scopes` 是四项里唯一没有专门函数的一个，本轮以「作用域完整性」的判据表达
（`invalid_scope`），并如实说明它不是独立实现。

## 6 H03/H04 为什么保持 `pending`

判定机制交付 **≠** 验收达标。H03 的判据是「工具返回、评论原文、模型草稿**不得写成**事实或承诺」，
H04 是「跨作用域读取**被拒**；观众长期记忆默认关闭」。本轮交付的是**判定这些事情的纯函数**，
而**真实的记忆子系统并不存在**——没有持久化、没有控制面接线、没有 Pro 档位的跨会话启用（§42.5 本轮未覆盖）。
这与仓库既有先例一致：W32–W36 的判定骨架轮令 B02／O03／O04 保持 `blocked`；A12（桌面壳边界）与 O01（演示包判定机制）
交付了机制却**均保持 `pending`**；A13 之所以是 `implemented`，是因为它的判据（矩阵与源码一致）**自足**，
不需要任何运行时。

H03/H04 不自足：它们描述的是运行时行为。故状态不变，`notes` 记下本轮交付了什么、还缺什么。

## 7 验证命令与环境边界

```bash
cargo xtask check --profile full      # 工程门控 + 前端门控（含注册表家族断言、契约目录、tests/ui、tests/platform）
cargo xtask check-deps                # 期望：19 workspace packages（不变）
node --test tests/registry.test.mjs   # 家族在册断言的非空洞守卫
openspec validate w57-memory-read-write-surface --strict
openspec validate --all --strict      # W56 教训：两条口径互不替代，都要跑
openspec validate --all --strict --archived
```

环境边界（如实）：本机为 Windows；Rust 测试目标需先**预热两个构建目标**以避开既有的间歇性 `LNK1104`；
前端门控依赖仓库根层的依赖副本。两者都是**主机事实**，不读作变更的通过/失败依据。
本轮无网络、无凭据、无外部服务，全部判据在纯逻辑层可复现。

## 8 未做与外部阻塞

- **不实现持久化存储**：`MemoryStore` 不在本轮——契约是消息面，落地存储属运行时/存储层。
- **§42.5 跨会话边界未覆盖**：授权/审批/租约/设备身份不跨场继承这一条，等 W58 之后的轨道推进。
- **阈值不定夺**：`MemoryPolicy` 的既有默认（内容长度、保留期上限、敏感标记）沿用，不新造取值。
- **H03/H04 保持 `pending`**：机制交付，真实子系统与生产接线仍是外部条件。
- **无新增依赖、无能力目录改动、无生成物漂移**。
