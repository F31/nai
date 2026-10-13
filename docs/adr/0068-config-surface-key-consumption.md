# ADR 0068：配置面的键级消费状态——受判 `required` 键的三分类闭世界

- 状态：接受
- 日期：2026-10-13
- 相关：[ADR 0067](0067-viewer-memory-single-answering-point.md)（其遗留 1 的母题「配置面 `required` 键
  MUST 有消费者」）；[ADR 0051](0051-*.md)／[ADR 0052](0052-*.md)／[ADR 0053](0053-repository-scoped-evidence.md)
  （W73–W75 的**文件级**消费守卫：具名例外、来源自负其责、证据来源限仓库内）；
  [ADR 0054](0054-pending-decisions-ledger.md) 遗留 5（「再写一份 schema 是同一件事的第二处定义」）；
  `schemas/config/runtime.schema.json`、`crates/nai-storage/src/store.rs`、
  `crates/nai-runtime/src/composition.rs`、`apps/engine/src/pipeline.rs`、
  `scripts/contracts/config-consumption.mjs`、`tests/contracts/schema.test.mjs`

## 背景

W89 的 F1 判据是「**同层字段一个有主、一个无主**」：schema 的 `session_defaults` 三个键里，
`allow_auto_without_platform_grant` 有读取点、`viewer_memory_enabled` 一个都没有。
W89 用了那条对照，但没有回答「**这个配置面上一共有多少个这样的键**」——因为仓库里的守卫只到**文件级**：
每份 `*.schema.json` 要么被执行体消费、要么具名例外（ADR 0051 决策 3 / W73）。**键**这一层没有守卫，
于是「这个键有人看吗」只能靠一次性人工核实，而**这类核实会过期**。

## 证据（本轮实测）

扫描面 = 非测试、非生成、已跟踪的源码；读取形状 = `EXPR["<key>"]`（`[` 前必须是表达式字符）。

| 分类 | 数量 | 成员 |
| --- | --- | --- |
| `required` 键 | 45 | 其中 8 个是**有 `required` 子键的容器**（消费由子键蕴含） |
| **受判键** | **37** | 36 叶键 + `bindings`（唯一「无 `required` 子键的 object 键」） |
| [A] 有读取点 | **10** | `edition`/`deployment`/`workspace_id`（store.rs）、`runtime.max_active_sessions`（composition.rs）、`session_defaults.allow_auto_without_platform_grant`（pipeline.rs）、`media.lease_renew_ms`/`lease_expire_ms`（composition.rs）、`media.startup_buffer_ms`/`max_buffer_ms`、`budgets.per_session_limit_minor`（store.rs） |
| [B] 无读取点、值被 `const` 钉死 | **4** | `schema_version`、`media.model_audio_format`、`budgets.hard_stop_new_paid_requests`、`cache.invalidate_on_fact_change` |
| [C] 无读取点、也未被钉死 | **23** | 见下 |

⇒ **`viewer_memory_enabled` 不是特例，是 23 分之 1**。改这 23 个键的值**不改变任何行为**。

### [C] 的成因（每条均实测，不编理由）

1. **`constructor-injected`（12 条）**——值以构造器/参数由调用方注入，而调用方在生产代码里**不存在**：
   `Book::new(workspace, limits)`（`nai-budget-core/src/lib.rs:71`）、`EventIngress::new(…, capacity, …)`
   与 `Scheduler::new(…, policy)`（`nai-kernel/src/scheduler.rs:70/271`）、`RemoteGateway::new(policy)`
   （`nai-knowledge-core/src/remote.rs:513`）的调用方**只有 `tests/`**。
2. **`hardcoded-default`（1 条）**——`session_defaults.mode`：`Session::new` 写死 `SessionMode::Assisted`
   （`nai-kernel/src/session.rs:61`）。
3. **`surface-absent`（5 条）**——非测试代码里**连键名都不出现**：`runtime.{plugin_catalog,control_listener,
   http_enabled}`、`cache.tts_max_bytes`、`budgets.soft_threshold_ratio`（`Ceiling`/`Limits` 里也无此字段）。
4. **`same-name-other-object`（3 条）**——`cloud.*`：唯一带 `cloud` 的生产代码是
   `nai-upgrade-core/src/cloud.rs`（A10），读的是**升级清单**的字段。**同名不同物**。
5. **`binding-consumer-absent`（1 条）**——`bindings`：`ConfigKind` 只有 `Runtime`/`Provider`；
   `capability_bindings` 表在生产代码里只被测试提及。
6. **`awaiting-decision`（1 条）**——`session_defaults.viewer_memory_enabled`：接线是**产品决策**（遗留 1）。

## 决策

1. **判据下推一层、落在同一份既有守卫文件里。** W73 的文件级守卫及其基础设施
   （`codeFiles`／`stripCommentLines`／`CODE_LANGUAGES`／`readTrackedPaths`）都在
   `tests/contracts/schema.test.mjs`，且该文件已在 `engineering_checks` 登记 ⇒ **门控零改动**。
2. **受判键集合派生，「容器」由结构判定。** `type === 'object'` 且有非空 `required` ⇒ 不受判
   （JSON 索引链决定：读 `config["media"]["x"]` 必先读 `config["media"]`）。
   **无 `required` 子键的 object 键 MUST 自己受判**——这条边界是派生的，MUST NOT 写成键名清单。
3. **[B] 派生，[A]/[C] 手写，两侧双向。** 若把 [C] 派生为「[A]∪[B] 的补集」，判据就是同义反复：
   新键会**自动**落进「未接线」而不需要任何人签名。手写 + 与派生结果双向相等 = 有人为状态负责。
4. **读取形状只认 `EXPR["<key>"]`。** 实测 `xtask/src/deps.rs:441` 的 `&["knowledge"]` 是 Rust
   **数组字面量**，宽松正则会把它算成读取点，让零读者的容器键凭空获得读者。代价是「将来若改用
   反序列化到结构体读配置会被报成未接线」——这是**假阳性方向**（安全）。
5. **扫描面按结构排除，每条排除付理由、陈旧即报。** 测试面 MUST 按**路径段**排除：
   第一版按前缀，漏掉了 `crates/nai-runtime/tests/composition.rs`（实测）。
6. **定义面不靠具名排除。** `.json` 不在 `CODE_LANGUAGES` 里 ⇒ schema 凭**语言闭包**就在扫描集之外；
   测试侧另有一条断言钉住它。第一版把 schema 写进了排除表，**判据自己**报了「陈旧排除」。
7. **两张台账具名 + 自清理。** [A] 每条写**证据文件**（断言文件里真有读取形状、且被跟踪）；
   [C] 每条写成因 id（成因表每条 MUST 至少被一个键引用）。缺失、陈旧、遮蔽、未知成因都报错。
8. **[A] 侧同样双向：有读取点的键 MUST 具名在册。** 少了这一条，台账对闭世界**不承重**——
   闭世界只要求「落在三类之一」，而 [A] 由扫描自动成立 ⇒ 清空 `WIRED` 判据照旧全绿。
   本判据的第一版正是如此，被自己的非空洞断言抓到。
9. **只登记、不改行为。** 判据要抓的是「**新增**死键」与「**在册状态过期**」，不是「存量死键立刻消失」。
10. **不新增 JSON Schema 约束这份台账**（ADR 0054 遗留 5 的同款判断）：判据自己是执行体，
    再写一份 schema 会在同一件事上出现**第二处定义**。

## 后果与实施

- **新增** `scripts/contracts/config-consumption.mjs`：`requiredKeys`／`guardedKeys`／`readShape`／
  `readSites`／`inScan` + 两张台账（`WIRED` 10 条、`UNWIRED` 23 条）+ 成因表（6 条）+
  四条判据（`partitionErrors`／`evidenceErrors`／`causeErrors`／`scanExclusionErrors`）。
- **修改** `tests/contracts/schema.test.mjs`：新增一条 test（12 条注入 + 3 条非空洞断言）。
- **门控**：`tests/contracts/schema.test.mjs` **29 → 30**（W86 之后未动过；`tests/registry.test.mjs`
  仍 **50**）；`cargo xtask` **零改动**。
- **不改运行时行为**：23 个 [C] 键只登记。这是本 ADR 与「接线轮次」的分界线。

## 遗留

1. **[C] 的 23 个键的接线是产品决策**：`scheduler.*`（5）、`knowledge.*`（5）、`cloud.*`（3）、
   `runtime.{plugin_catalog,control_listener,http_enabled}`、`bindings`、`session_defaults.mode`、
   `media.output_sample_rate`、`budgets.{soft_threshold_ratio,currency}`、`cache.tts_max_bytes`。
   MUST NOT 读成「已处理」。
2. **读取形状只覆盖一种写法**：若将来用 serde 反序列化到结构体读配置，判据会报「未接线」（假阳性，安全）。
   真出现时，正确的修法是把那条读取路径**也**纳入派生（例如「结构体字段名 + `#[derive(Deserialize)]`
   的配置类型」），MUST NOT 把该键直接搬进 [A] 台账了事。
3. **`const` 被读取点的那个交叠**：`session_defaults.allow_auto_without_platform_grant` 既是 `const:false`
   又被 `pipeline.rs` 读（判 A 优先）。在**校验过**的配置上该键恒为 `false`，于是
   `bring_up` 的 `AutoSupervised` 分支**恒拒绝**。这是**产品语义**问题（平台授权尚未取得，基线 §5 亦有此陈述），
   本轮**未裁定**，MUST NOT 读成缺陷。
4. **基线的配置样例（V3.11 line 1170–1210）与消费面没有对账**：样例逐键声明，消费面 10/37。
   若要把「声明 ≠ 消费」升成基线级陈述，那是**升基线轮次**的事。
5. **`budgets` 段的价值在别处**：`budget-core` 的 `Limits` 由可信所有者注入（其 crate 文档首行即声明
   「A trusted owner supplies limits」）——所以 [C] 不等于「预算功能缺失」，只等于「配置面未接线」。
