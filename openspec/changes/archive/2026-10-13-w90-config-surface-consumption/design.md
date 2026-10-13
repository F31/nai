# w90 设计：配置面的键级消费状态

## 一、问题的形状

不是「有人写错了配置」，也不是「实现有缺陷」，而是**一个面被声明了、却没有一件东西负责核对谁在看它**：

| 面 | 载体 | 实测 |
| --- | --- | --- |
| 声明面 | 基线 V3.11 line 1170–1210 的配置样例 | 完整（`[session_defaults]`/`[scheduler]`/`[media]`/`[knowledge]`/… 逐键列出） |
| 契约面 | `schemas/config/runtime.schema.json` | 45 个 `required` 键，`additionalProperties:false` |
| 校验面 | `crates/nai-storage/src/store.rs` 的 `validate_json`（`jsonschema` crate） | ✓ 真的接上电（`load_config` 每次都调） |
| **消费面** | 运行时读取点 | **37 个受判键里只有 10 个** |

⇒ 配置的**形状**被强制，配置的**语义**基本没人看。W89 的 F1 只看到了其中一个键。

## 二、实测（扫描非测试、非生成、已跟踪的源码，形状 `EXPR["<key>"]`）

```
受判键 37（= 45 个 required − 8 个「有 required 子键的容器」）
[A] 有读取点 10：
  edition, deployment, workspace_id                <- crates/nai-storage/src/store.rs
  runtime.max_active_sessions                      <- crates/nai-runtime/src/composition.rs
  session_defaults.allow_auto_without_platform_grant <- apps/engine/src/pipeline.rs
  media.lease_renew_ms, media.lease_expire_ms      <- composition.rs（+ store.rs 的区间校验）
  media.startup_buffer_ms, media.max_buffer_ms     <- store.rs
  budgets.per_session_limit_minor                  <- store.rs
[B] 无读取点、值被 const 钉死 4：
  schema_version, media.model_audio_format, budgets.hard_stop_new_paid_requests,
  cache.invalidate_on_fact_change
[C] 无读取点、也未被钉死 23（成因见三）：
  runtime.plugin_catalog / control_listener / http_enabled
  bindings
  session_defaults.mode / viewer_memory_enabled
  scheduler.max_pending_utterances / max_pending_speech_ms / qa_ttl_ms / welcome_ttl_ms / thanks_ttl_ms
  media.output_sample_rate
  knowledge.mode / fts_enabled / semantic_enabled / remote_source_refs / index_during_live
  budgets.soft_threshold_ratio / currency
  cache.tts_max_bytes
  cloud.control_enabled / relay_enabled / render_enabled
```

## 三、[C] 的成因（每条都实测过，不编理由）

1. **`constructor-injected`**（12 条：`scheduler.*` 5、`knowledge.*` 5、`media.output_sample_rate`、
   `budgets.currency`）：值以构造器/参数由调用方注入，而调用方**在生产代码里不存在**——
   `Book::new(workspace, limits)`（`crates/nai-budget-core/src/lib.rs:71`）、
   `EventIngress::new(…, capacity, state_capacity, dedup_capacity)` / `Scheduler::new(…, policy)`
   （`crates/nai-kernel/src/scheduler.rs:70/271`）、`RemoteGateway::new(policy)`
   （`crates/nai-knowledge-core/src/remote.rs:513`）的调用方**只有 `tests/`**。
2. **`hardcoded-default`**（1 条：`session_defaults.mode`）：`Session::new` 把初始模式写成
   `SessionMode::Assisted`（`crates/nai-kernel/src/session.rs:61`）。
3. **`surface-absent`**（5 条：`runtime.{plugin_catalog,control_listener,http_enabled}`、
   `cache.tts_max_bytes`、`budgets.soft_threshold_ratio`）：非测试代码里**连键名都不出现**
   （命中 0 次）；`budgets-core` 的 `Ceiling`/`Limits` 里也没有软阈值字段。
4. **`same-name-other-object`**（3 条：`cloud.*`）：**同名不同物**——唯一带 `cloud` 的生产代码是
   `crates/nai-upgrade-core/src/cloud.rs`（A10 云断连依赖方向），读的是**升级清单**的字段。
5. **`binding-consumer-absent`**（1 条：`bindings`）：`ConfigKind` 只有 `Runtime`/`Provider` 两种；
   `capability_bindings` 表在生产代码里只被测试提及。
6. **`awaiting-decision`**（1 条：`session_defaults.viewer_memory_enabled`）：接线本身是**未决的产品决策**
   ——ADR 0067 裁定的是「判定面收口」，把配置面的键接到 `MemoryPolicy` 会**改变运行时行为**。

## 四、决策

1. **判据落在键级、落在同一份既有守卫文件里**：`tests/contracts/schema.test.mjs` 已有 W73 的文件级
   守卫与全部基础设施（`codeFiles`/`stripCommentLines`/`CODE_LANGUAGES`/`readTrackedPaths`），
   `xtask` 里该文件已登记 ⇒ **零门控改动**。
2. **受判键集合派生，「容器」由结构判定**：`type === 'object'` 且有非空 `required` ⇒ 不受判
   （JSON 索引链决定：读 `config["media"]["x"]` 必先读 `config["media"]`）。
   **无 `required` 子键的 object 键 MUST 自己受判**（`bindings`）——这条边界是**派生**的，
   不是「我记得把 bindings 加进去」。
3. **[B] 派生、[A] 与 [C] 手写、两侧双向**。若把 [C] 派生为「[A]∪[B] 的补集」，判据就是同义反复：
   新键会**自动**落进「未接线」而不需要任何人签名。手写 + 与派生结果双向相等，才是「有人为状态负责」。
4. **读取形状只认 `EXPR["<key>"]`**：`[` 前必须是表达式字符。理由实测——`xtask/src/deps.rs:441`
   的 `&["knowledge"]` 是**数组字面量**，宽松正则会把它算成读取点，让零读者的容器键凭空获得读者。
   代价是「将来若用反序列化到结构体读配置，会被报成未接线」，这是**假阳性方向**（安全）。
5. **扫描面按结构排除，每条排除付理由 + 陈旧即报**。测试面 MUST 按**路径段**排除：第一版按前缀，
   漏掉了 `crates/nai-runtime/tests/composition.rs`（实测）。
6. **定义面不靠具名排除**：`.json` 不在 `CODE_LANGUAGES` 里 ⇒ 它凭**语言闭包**已在扫描集之外；
   测试侧另有一条断言钉住「`.json` 不在扫描集」——一旦有人把它加进语言表，那条会开火。
   第一版把 schema 写进了排除表，**判据自己**报了「陈旧排除匹配不到任何文件」（它只扫代码扩展名）。
7. **台账具名 + 自清理**：[A] 的每条写**证据文件**（断言文件里真有读取形状、且被跟踪）；
   [C] 的每条写成因 id（成因表每条 MUST 至少被一个键引用）。缺失、陈旧、遮蔽、未知成因都报错。
8. **[A] 侧也要双向**：「有读取点的键 MUST 具名在册」。少了这一条，台账对闭世界**不承重**——
   闭世界只要求「落在三类之一」，而 [A] 由扫描自动成立 ⇒ 清空台账判据照旧全绿。
   本判据的第一版正是如此，被自己的非空洞断言抓到（见五·F13）。
9. **只登记不改行为**：23 个 [C] 键的接线是产品决策。判据要抓的是「**新增**死键」与「**在册状态过期**」，
   不是「存量死键立刻消失」——后者是产品轮次。

## 五、非空洞注入（全部真跑）

测试内共 12 条注入 + 3 条非空洞断言；另做了一次**手工真跑**（改真实文件）。

| 编号 | 注入 | 期望 | 结果 |
| --- | --- | --- | --- |
| F0a | 三类各自非空 + 受判键非空 + 扫描面 > 100 文件 | 非空洞 | ✓ |
| F0b | 定义面凭**语言闭包**在扫描集之外（`.json ∉ CODE_EXTENSIONS`、schema 不在 `files` 里） | 非空洞（把定义面当读者会让判据恒真） | ✓ |
| **F0c** | **清空任一台账都必须报错**（两张台账都承重） | 非空洞 | ✓ **本判据第一版的真洞**：清空 `WIRED` 时判据全绿 ⇒ 补决策 8 |
| F1 | 把**真实源码文本**里 `crates/nai-storage/src/store.rs` 的 `["max_buffer_ms"]` 挖掉 | 该键从 [A] 掉出、被指名；且台账那条也被抓 | ✓（`deepEqual` 钉死**唯一**一条消息，不是「非空就行」） |
| F1′ | **手工真跑改前/改后**：真的把 `store.rs` 那一行改成 `["max_buffer_ms_"]` | 改前 **30 pass / 0 fail**；改后 **29 pass / 1 fail** 且消息名指该键 | ✓（还原后 sha256 逐字一致、复测 30/0） |
| F2 | 把 `edition` 的证据文件指到一个不含该形状的文件 | 自清理开火 | ✓ |
| F3 | 把**有读者**的键登记为未接线 | 报「遮蔽真读者」 | ✓ |
| F4 | 在册一个 schema 里不存在的键 | 报陈旧 | ✓ |
| F5 | 在册项缺成因 / 成因不在成因表 | 两条各报 | ✓ |
| F6 | 把**被钉死**的键登记为未接线 | 报「值不可能不同」 | ✓ |
| F7 | schema 新增 `required` 键（不接线、不登记） | 指名该键 | ✓ |
| F8 | 取消钉死（`cache.invalidate_on_fact_change` 的 `const` → `boolean`） | 掉出 [B] 并被指名（证明 [B] 派生） | ✓ |
| F9 | 容器边界：`cloud` 不受判 / `bindings` 必须受判 / 无 `required` 子键的 object 不得靠「像容器」逃逸 | 三条 | ✓ |
| F10 | `&["knowledge"]`（数组字面量）MUST NOT 算读取点；真索引 MUST 算 | 双向 | ✓ |
| F11 | 整行注释里的读取 MUST NOT 算 | 不算 | ✓ |
| F12 | 陈旧排除（匹配不到任何文件） | 指名 | ✓ |

## 六、明确不做

- **不给 [C] 的 23 个键接线**（改变运行时行为，属产品轮次）。
- **不把配置面的键接到 `MemoryPolicy`**（ADR 0067 已裁定「判定面收口」）。
- **不新增 JSON Schema 来约束这份台账**：判据自己是执行体（ADR 0054 遗留 5 的同款判断——
  再写一份 schema 会在同一件事上出现第二处定义）。
- **不动 `tests/pending-decisions.yaml`**：本轮的缺口是**配置面**，不是待裁定项台账。
