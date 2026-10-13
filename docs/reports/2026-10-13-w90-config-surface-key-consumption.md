# W90：配置面的键级消费状态（受判 `required` 键的三分类闭世界）

- 轮次：**W90**（承接 W89 遗留 1 的母题）
- 日期：2026-10-13
- 变更：[`w90-config-surface-consumption`](../changes/archive/2026-10-13-w90-config-surface-consumption/)
- 裁决：[ADR 0068](../adr/0068-config-surface-key-consumption.md)

## 一、判断

**不是**「有人写错了配置」，**也不是**「实现有缺陷」，而是**一个面被声明、却没有任何东西负责核对谁在看它**：

| 面 | 载体 | 实测 |
| --- | --- | --- |
| 声明面 | 基线 V3.11 line 1170–1210 的配置样例 | 完整（逐键列出 `session_defaults`／`scheduler`／`media`／`knowledge`／…） |
| 契约面 | `schemas/config/runtime.schema.json` | **45 个 `required` 键**，`additionalProperties:false` |
| 校验面 | `crates/nai-storage/src/store.rs::validate_json`（`jsonschema` crate） | ✓ **真的接上电**（`load_config` 每次都调） |
| **消费面** | 运行时读取点 | **37 个受判键里只有 10 个** |

⇒ 配置的**形状**被强制，配置的**语义**基本没人看。W89 的 F1 只看到了其中一个键
（`session_defaults.viewer_memory_enabled`）——**它不是特例，是 23 分之 1**。

## 二、核实（先测后设计；数字与事先的假设不同）

### 2.1 三分类实测

扫描面 = **非测试、非生成、已跟踪**的源码（201 文件）；读取形状 = `EXPR["<key>"]`
（`[` 前必须是表达式字符）。

| 分类 | 数量 | 成员 |
| --- | --- | --- |
| `required` 键总数 | 45 | 其中 **8 个是「有 `required` 子键的容器」**（消费由子键蕴含） |
| **受判键** | **37** | 36 叶键 + `bindings`（唯一「无 `required` 子键的 object 键」） |
| **[A] 有读取点** | **10** | `edition`／`deployment`／`workspace_id`（`store.rs`）、`runtime.max_active_sessions`（`composition.rs`）、`session_defaults.allow_auto_without_platform_grant`（`pipeline.rs`）、`media.lease_renew_ms`／`lease_expire_ms`（`composition.rs`）、`media.startup_buffer_ms`／`max_buffer_ms`、`budgets.per_session_limit_minor`（`store.rs`） |
| **[B] 无读取点、值被 `const` 钉死** | **4** | `schema_version`、`media.model_audio_format`、`budgets.hard_stop_new_paid_requests`、`cache.invalidate_on_fact_change` |
| **[C] 无读取点、也未被钉死** | **23** | `runtime.{plugin_catalog,control_listener,http_enabled}`、`bindings`、`session_defaults.{mode,viewer_memory_enabled}`、`scheduler.*`（5）、`media.output_sample_rate`、`knowledge.*`（5）、`budgets.{soft_threshold_ratio,currency}`、`cache.tts_max_bytes`、`cloud.*`（3） |

### 2.2 [C] 的成因（逐条实测，不编理由）

| 成因 | 条数 | 实测证据 |
| --- | --- | --- |
| `constructor-injected` | 12 | 值以构造器/参数由调用方注入，而调用方**在生产代码里不存在**：`Book::new(workspace, limits)`（`nai-budget-core/src/lib.rs:71`）、`EventIngress::new(…, capacity, state_capacity, dedup_capacity)`／`Scheduler::new(…, policy)`（`nai-kernel/src/scheduler.rs:70/271`）、`RemoteGateway::new(policy)`（`nai-knowledge-core/src/remote.rs:513`）的调用方**只有 `tests/`**；`nai-media-core` 的采样率由 `PlaybackSpec` 给 |
| `hardcoded-default` | 1 | `Session::new` 写死 `SessionMode::Assisted`（`nai-kernel/src/session.rs:61`） |
| `surface-absent` | 5 | 非测试源码里**连键名都不出现**（`plugin_catalog`／`control_listener`／`http_enabled`／`tts_max_bytes` 命中 0 次）；`budget-core` 的 `Ceiling`/`Limits` 里没有软阈值字段 |
| `same-name-other-object` | 3 | `cloud.*`：唯一带 `cloud` 的生产代码是 `nai-upgrade-core/src/cloud.rs`（A10 云断连依赖方向），读的是**升级清单**的字段 ⇒ **同名不同物** |
| `binding-consumer-absent` | 1 | `ConfigKind` 只有 `Runtime`／`Provider`（`store.rs`）；`capability_bindings` 表在生产代码里只被测试提及 |
| `awaiting-decision` | 1 | `viewer_memory_enabled`：接线是**产品决策**（ADR 0067 遗留 1） |

### 2.3 两处**假阳性**（收紧判据的直接动因）

1. **数组字面量**：`xtask/src/deps.rs:441` 的 `&["knowledge"]` 是 Rust 数组字面量，
   宽松正则 `\["key"\]` 会把它算成读取点 ⇒ 零读者的容器键凭空获得读者。修法：`[` 前必须是**表达式字符**。
2. **测试面按前缀排除漏掉 crate 内的测试**：`crates/nai-runtime/tests/composition.rs` 里也有
   `["lease_expire_ms"]`。修法：按**路径段**排除（含 `crates/*/tests/`、`src/tests.rs`、`*.test.mjs`）。

## 三、裁定

用户选定 **「判据 + 在册」**（三选项之一，另两个是「判据 + 真实接线」与「只做增量门」）：

> 立「配置面消费状态」三分类闭世界判据，逐键在册，**只登记、不改运行时行为**；
> 新增 `required` 键若既无读者、也未被钉死、又未在册 ⇒ **门控红**。

理由：接线 23 个键是**产品行为改变**（ADR 0067 已裁定「判定面收口」），而本轮的缺口是
**「没有任何东西负责核对」**——判据与台账才是那个被缺掉的东西。

## 四、实现

- **新增** `scripts/contracts/config-consumption.mjs`（纯函数）：`requiredKeys`／`guardedKeys`／
  `readShape`／`readSites`／`inScan` + 两张台账（`WIRED` 10 条含证据文件、`UNWIRED` 23 条含成因 id）
  + 成因表（6 条）+ 四条判据（`partitionErrors`／`evidenceErrors`／`causeErrors`／`scanExclusionErrors`）。
- **修改** `tests/contracts/schema.test.mjs`：新增一条 test（12 条注入 + 3 条非空洞断言）——
  与 W73 的文件级守卫**同住一份文件**，`cargo xtask` **零改动**。
- **四条判据**：
  1. **受判键集合派生**：`required` 里减去「有 `required` 子键的容器」；**容器由结构判定**
     （`type === 'object'` 且有非空 `required`），MUST NOT 写成键名清单；
     **无 `required` 子键的 object 键 MUST 自己受判**（`bindings`）。
  2. **三分类双向闭世界**：两两不相交、并集恰等于受判键集合；**[A] 与 [C] 两侧都双向**
     （有读取点的键 MUST 具名在册 ＝ **观测不能替人签名**；在册的键 MUST 真的没有读取点 ＝ **签名不能遮住观测**）。
  3. **读取形状只认 `EXPR["<key>"]`**（见 2.3）。
  4. **两张台账具名 + 自清理**：缺失 / 陈旧 / 遮蔽 / 未知成因 / 空理由都报错；成因表每条 MUST 被引用。
- **扫描面按结构排除、每条排除付理由、陈旧即报**：测试面（路径段）、生成面、判据自身。
  **定义面不靠具名排除**——`.json` 不在 `CODE_LANGUAGES` 里，schema 凭**语言闭包**就在扫描集之外；
  测试侧另有一条断言钉住这件事（第一版把 schema 写进了排除表，**判据自己**报了「陈旧排除」）。

## 五、非空洞注入（F0a–F12 全部成立）

| 编号 | 注入 | 结果 |
| --- | --- | --- |
| F0a | 三类非空 + 受判键非空 + 扫描面 > 100 文件 | ✓ |
| F0b | 定义面凭语言闭包在扫描集之外（`.json ∉ CODE_EXTENSIONS`、schema 不在 `files` 里） | ✓ |
| **F0c** | **清空任一台账必须报错** | ✓ —— **本判据第一版的真洞**（见 §九 1） |
| F1 | 挖掉**真实源码文本**里 `store.rs` 的 `["max_buffer_ms"]` | ✓ `deepEqual` 钉死**唯一**一条消息 |
| **F1′** | **手工真跑改前/改后**：真的改成 `["max_buffer_ms_"]` | 改前 **30 pass / 0 fail**；改后 **29 pass / 1 fail** 且名指该键；还原后 **sha256 逐字一致**、复测 **30/0** |
| F2 | 证据文件不含该读取形状 | ✓ 自清理开火 |
| F3 | 把**有读者**的键登记为未接线 | ✓ 报「遮蔽真读者」 |
| F4 | 在册一个 schema 里不存在的键 | ✓ 报陈旧 |
| F5 | 在册项缺成因 / 成因不在成因表 | ✓ 两条各报 |
| F6 | 把**被钉死**的键登记为未接线 | ✓ 报「值不可能不同」 |
| F7 | schema 新增 `required` 键（不接线、不登记） | ✓ 指名该键 |
| F8 | 取消钉死（`const` → `boolean`） | ✓ 掉出 [B] 并被指名（证明 [B] 派生） |
| F9 | 容器边界：`cloud` 不受判／`bindings` 必须受判／无 `required` 子键的 object 不得靠「像容器」逃逸 | ✓ 三条 |
| F10 | `&["knowledge"]`（数组字面量）MUST NOT 算；真索引 MUST 算 | ✓ 双向 |
| F11 | 整行注释里的读取 MUST NOT 算 | ✓ |
| F12 | 陈旧排除（匹配不到任何文件） | ✓ 指名 |

## 六、验证

### 6.1 分面

| 面 | 命令 | 结果 |
| --- | --- | --- |
| 契约（Node） | `node --test tests/contracts/schema.test.mjs` | **30 passed / 0 failed**（**29 → 30**） |
| `openspec validate <change> --strict`（归档前） | — | **valid** |

### 6.2 全量门控

`cargo xtask check --profile full` ⇒ **`EXIT=0`**：**91 个测试目标**（`test result:` 行 91 条）、
**862 passed / 0 failed**、`LNK1104` 命中 **0**。

- 该 profile **内含** `cargo fmt --all --check` 与 `cargo clippy --workspace --all-targets --locked -- -D warnings`
  （gate 输出第 647/648 行）⇒ 两者随 `EXIT=0` 一并通过；`rustfmt` 另作**独立复跑**亦 `EXIT=0`。
- 门控前先做 `--no-run` 预链接（`CARGO_BUILD_JOBS=1`）**14 轮**收敛至 `EXIT=0`
  （`LNK1104` **折在不同目标上** ⇒ 环境级，非代码缺陷；**重试收敛不算掩盖问题**）。
- 与 W89 同形：**91 目标 / 862 passed**。本轮新增的是 **Node 契约判据**（29 → 30），
  **不进 cargo 目标计数** ⇒ 两侧数字不变属预期，不是「门控没跑到新判据」。

### 6.3 两条 validate

| 命令 | 结果 |
| --- | --- |
| `openspec validate --all --strict`（归档前） | **44 passed / 0 failed**（`change/w90-config-surface-consumption` 在册） |
| `openspec validate --all --strict`（归档后） | **43 passed / 0 failed**（change 已移出在册面） |
| `openspec validate --all --strict --archived` | **88 passed / 0 failed**（87 → **88**，含 `change/2026-10-13-w90-config-surface-consumption`） |

## 七、CI 复核

**CI 复核**

- run **38379952857**（run_number **111**），`head_sha = 71efbf0405` = 本次推送 SHA
  `71efbf0405a3539fd10be11d470a9a604198ffe7` ⇒ 结论 **`success`**（09:56:56Z → 10:08:49Z，约 12 min）。
- 两平台均绿、各 **26 steps**：`foundation (ubuntu-24.04)` 09:57:00Z → 10:02:06Z；
  `foundation (windows-latest)` 09:57:00Z → 10:08:49Z。

### 观测纪律

- 查询带**唯一参数** `&cb=<ns>` + 请求头 `Cache-Control: no-cache`；**回读** `head_sha` == 推送 SHA；
  `curl --tlsv1.2 --tls-max 1.2`（本机 schannel 与 TLS 1.3 握手失败）。
- 本轮共 **8 次** GitHub API 调用（含 1 次 `/rate_limit`（不计 core 配额）、1 次 `runs/<n>/jobs` 因
  误用 run_number 得 404、run 109 的诊断 3 次）；单轮保持个位数、无轮询。
  `sleep 720` / `sleep 300` 均被 SIGTERM 截断（**无输出、退出码 1**，与 W88 同现象）⇒ 改为直接单次查询。

### 顺带观测（**上一轮**的回填提交）

- run **38374444778**（run_number **109**，`head_sha = 0b01ee35ee` = W89 的 CI 回填提交 `0b01ee3`）
  结论 **`failure`**：`foundation (ubuntu-24.04)` **success**（09:10:45Z→09:16:05Z）、
  `foundation (windows-latest)` **failure**，失败步为第 10 步合并门控
  `Complete engineering foundation (portable steps + frontend workspace)`（09:10:46Z→09:17:11Z）。
- **判据（实测，非推断）**：run 110（`54d892f`）的改动**包含** run 109（`0b01ee3`）的全部改动，
  二者在**同一 windows 作业**上结果相反 ⇒ **失败不由提交内容引起**（平台侧瞬时）。
- **未取得的证据**：作业日志端点 `/actions/jobs/<id>/logs` 对匿名请求返回
  **403 `Must have admin rights to Repository.`** ⇒ **未能取到失败细节**。故本条只到
  「内容无关 + 平台侧」为止，**MUST NOT 读成「已查明为 flaky」**。
- 该 failure 属**上一轮**回填提交，按 W89 既定边界「**回填提交自身不回填**」不在 W89 §七的承诺范围内；
  本轮**首次观测到并如实登记**（不追溯改写 W89 报告）。

## 八、遗留

1. **[C] 的 23 个键的接线是产品决策**（`scheduler.*` 5／`knowledge.*` 5／`cloud.*` 3／
   `runtime.{plugin_catalog,control_listener,http_enabled}`／`bindings`／`session_defaults.mode`／
   `media.output_sample_rate`／`budgets.{soft_threshold_ratio,currency}`／`cache.tts_max_bytes`）。
   本轮**只登记**，MUST NOT 读成已处理。
2. **读取形状只覆盖一种写法**：若将来用 serde 反序列化到结构体读配置，判据会报「未接线」（**假阳性**，安全方向）。
   真出现时的正确修法是**把那条读取路径也纳入派生**，MUST NOT 直接把该键搬进 [A] 台账了事。
3. **`const` 与读取点的交叠**：`session_defaults.allow_auto_without_platform_grant` 既是 `const:false`
   又被 `pipeline.rs` 读（判 [A] 优先）。在**校验过**的配置上该键恒为 `false` ⇒ `bring_up` 的
   `AutoSupervised` 分支**恒拒绝**。这是**产品语义**问题（基线 §5 明写「目前没有取得任何平台授权」），
   本轮**未裁定**，MUST NOT 读成缺陷。
4. **基线的配置样例与消费面没有对账**：样例逐键声明，消费面 10/37。若要把「声明 ≠ 消费」
   升成**基线级**陈述，那是升基线轮次的事。
5. **`budgets` 段的价值在别处**：`budget-core` 首行文档即声明「A trusted owner supplies limits」
   ⇒ [C] 不等于「预算功能缺失」，只等于「配置面未接线」。
6. **余下 3 条 `decidable_in_repo`**（`PD-22`／`PD-24` A2A、`PD-34` 需访谈）与 H03/H04 的 `pending` 状态**未动**。

## 九、方法论教训

1. **非空洞断言能抓到「对闭世界不承重」的台账——这是本轮最值钱的一条。**
   第一版的闭世界只要求「键落在三类之一」，而 [A] 由**扫描自动成立** ⇒ 清空 `WIRED` 台账判据**照旧全绿**。
   台账于是成了注释而非判据。修法：**[A] 侧也要双向**（有读取点的键 MUST 具名在册）。
   ⇒ **「双向」不是对称性的审美，它是「观测」与「签名」的分工**：观测不能替人签名，签名不能遮住观测。
2. **判据会抓到自己人的空洞排除。** 第一版把 `schemas/config/runtime.schema.json` 写进排除表求个心安，
   而它**根本不在扫描集里**（`.json` 不在语言表）⇒ 判据报「陈旧排除」。
   ⇒ **排除表里的每一条都必须是承重的**；能靠**结构性闭包**排除的，MUST NOT 用具名排除（那是第二张会过期的清单）。
3. **用前缀表达「测试面」是错的**：测试住在 `crates/*/tests/`、`apps/*/tests/`、`src/tests.rs`、`*.test.mjs`
   四个地方，前缀只挡住一个。⇒ **凡「某类路径」的规则，MUST 按路径段/结构判，MUST NOT 按前缀判**。
4. **形状判据的两个方向都要测**：数组字面量 `&["knowledge"]` 是假阳性（凭空造读者，危险方向），
   真索引漏判是假阴性。测法是**喂两个样本**，而不是只喂一个。
5. **先测后设计**：事先我按 W89 的信息估计「只有 4 个键有读者」，实测是 **10**；估计「[C] 大约 56 个」，
   实测 **23**。若照估计写台账，23 个键的名单会错，而台账错就是判据错。
6. **「成因」必须逐条可核实，不能靠归纳**：六个成因里唯一需要「读外部文档」的是 `awaiting-decision`；
   其余五条都落在**一行代码或一次零命中**上（构造器签名、硬编码常量、`grep` 零命中、另一个对象）。
7. **只登记不等于逃避**：台账的价值不在于让门控变绿，而在于**新增死键会响**、且每条在册状态
   **过期即响**。存量 23 条的存在本身是被记录的事实，不是被批准的豁免。
