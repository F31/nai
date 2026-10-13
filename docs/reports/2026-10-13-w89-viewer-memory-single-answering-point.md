# W89：观众长期记忆开关的单一来源与写入面（PD-31 结清）

- 日期：2026-10-13
- 变更：`openspec/changes/w89-viewer-memory-default-off-single-home/`
  （归档后为 `openspec/changes/archive/2026-10-13-w89-viewer-memory-default-off-single-home/`）
- 实现：`crates/nai-kernel/src/evaluate/memory.rs`（+42 / −8）、`crates/nai-kernel/tests/memory.rs`（+86 / −20）
- 数据：`tests/pending-decisions.yaml`（+8 / −4）、`scripts/contracts/pending-decisions.mjs`（+2 / −2）
- 登记：`tests/registry.yaml` 的 **H03** notes 追加 W89 订正（**`status` 保持 `pending`**）
- 承接：W88 §八第 2 条登记的张力；ADR 0054 遗留 3 的第二条

## 一、判断：不是实现缺陷，是「默认关闭」的落点不闭合

W88 §八第 2 条登记的张力是「`PD-31` 的『空集』与 W57 已交付的 `allowed_viewer_fields` 非空冲突」。
本轮核实后，**结论比登记的张力更靠前**：

> 「观众长期记忆**默认关闭**」这条开关在仓库里被写了**三遍**，而**只有读取面一处有执行体**。

⇒ 所以「空集」那句**没有说错**（它指运行时被写入的字段值集合，默认关闭 ⇒ 确实为空），
`allowed_viewer_fields` 也**不是缺陷**（它是启用后才生效的允许字段白名单，D10 沿用）。
真正的问题是**同一件事有三处载体、两处是空的**——这正是 W88 在**分派层**确立的
「**同一件事 MUST NOT 两处定义**」在**能力开关层**的复现。

## 二、核实：先读后判（三条实测事实）

| 编号 | 事实 | 判据（实测） |
| --- | --- | --- |
| **F1** | **配置面是死键** | `schemas/config/runtime.schema.json` 的 `session_defaults` 是 `additionalProperties:false` 且 **`required` 明列** `viewer_memory_enabled`；全仓**非测试**代码里 `composition.config()["session_defaults"]` **只被读一次**（`apps/engine/src/pipeline.rs:874`），读的是**同层的** `allow_auto_without_platform_grant` ⇒ 把 `viewer_memory_enabled` 改成 `true` **没有任何效果**。同层字段「**一个有主、一个无主**」——这个**对照本身**就是判据 |
| **F2** | **判定面只覆盖一半** | `viewer_memory_enabled` 在 `memory.rs` 只被两处消费：`read_records`（**读取面**，→ `viewer_memory_disabled`，有集成测试）与 `viewer_record_allowed`（**无生产调用方**）。**写入面 `admit_write` 既不接受策略、也不检查它** ⇒ 默认关闭下一条 `kind = "viewer"` 的候选**会被接受**（真跑改前那一行：`Ok(())`） |
| **F3** | **措辞同形不同义** | 推荐的「空集」论据是「§5.5 的默认关闭**使字段集没有对象**」⇒ 指**运行时被写入的字段值集合**；而 `allowed_viewer_fields = ["preference","answered_topic"]` 与 `max_retention_ms`（30 天上界）是**启用后才生效的允许字段白名单与上界**，W57 报告 §七明写「**阈值不定夺**（沿用既有值，**不新造取值**）」⇒ 台账**用同一个词指了两样东西** |

**一处顺带发现（W88 的漏改）**：`tests/pending-decisions.yaml` 的**头部注释**只列五态
（不含 W88 新立的 `decided_in_repo`→`landing`）——W88 立第六态时漏改了本注释。它是**活文档**
（被门控消费），故本轮**就地订正**。

**基线侧无待裁定项**：`PD-31` 的 `introduced_in` = §48.7，`also_in` = §5.5；§5.5 原文
「观众长期记忆：**默认关闭**。开启需记录平台与应用作用域 ID、用途、**少量允许字段**、授权、保留期和删除入口」——
**「少量允许字段」本身就承认白名单非空**，故「空集」不可能指白名单。

## 三、裁定（[ADR 0067](../adr/0067-viewer-memory-single-answering-point.md)）

1. **`viewer_memory_gate(policy)` 是这条开关的唯一判定处**；读取面、写入面与请求准入面 MUST 共用它，
   MUST NOT 各自复写 `if !policy.viewer_memory_enabled`。
2. **写入面 MUST 也判它**——本轮补的是**执行体**，不是新增策略。判定是**能力级**的（不依赖条目内容）。
3. **拒因复用 `viewer_memory_disabled`，MUST NOT 新造名字**（同一件事 MUST NOT 有两个名字）
   ⇒ `MemoryRejection::ALL` 仍 **23**，既有的「构造出的集合**恰为** `ALL`」总账断言**不放宽**。
4. **判据位置**：`admit_write` 里放在**必填项之后、来源判定之前**，与读取面
   「**结构必填 → 能力开关 → 授权**」同序；**顺序 MUST 有断言**，否则只是注释。
5. **开关 MUST NOT 塞进 `MemoryCandidate`**（那是 W57 §四.1 已排除的「把门槛交给被审对象自己填」）
   ⇒ `admit_write` 增加 `policy: &MemoryPolicy` 入参。
6. **legacy `viewer_record_allowed` 保留 `EvaluateError` 契约，只做错误类型映射**——
   **映射不是第二处定义**，`if !enabled` 这个判定在仓库里只剩一处。
7. **F1 只登记不改**，MUST NOT 用「不动」伪装成「已处理」。
8. **台账的「空集」就地订正为两个具名对象**（活文档就地订正，1:1 行替换）：
   运行时字段值集合为空集／允许字段白名单沿用 D10 且 **R1 内不生效**／R3 的**取值**仍未定。

## 四、实现

**`crates/nai-kernel/src/evaluate/memory.rs`**
- 新增 `viewer_memory_gate(policy) -> Result<(), MemoryRejection>`（唯一判定处）；
- `read_records` 的 `if !policy.viewer_memory_enabled { … }` 改为 `viewer_memory_gate(policy)?`；
- `viewer_record_allowed` 改引 gate 并把拒绝**映射**成 `EvaluateError::Unauthorized`（契约不变）；
- `admit_write(candidate, policy)`：必填项之后插入
  `if record.kind == VIEWER_KIND { viewer_memory_gate(policy)?; }`；
- 模块头注释与 `admit_write` 的判据顺序注释同步（三件事各有各的名字：缺必填项／能力没开／来源不可信）。

**`crates/nai-kernel/tests/memory.rs`（21 → 23）**
- 既有 20 处 `admit_write` 调用点改为局部 helper `admit()`（默认策略）；
- 新增 `a_closed_viewer_memory_is_closed_on_the_write_face_too`（写面被拒 + 与读取面**同一拒因** +
  启用后通过 + **非观众类别不受影响**）；
- 新增 `the_capability_gate_is_answered_before_the_source_gate`（顺序断言 + 必填项仍先于能力）。

**数据与文档**：台账 `PD-31` 迁 `decided_in_repo` + `landing`
（`["crates/nai-kernel/src/evaluate/memory.rs"]`），`DISPOSITION_COUNTS` `decidable_in_repo` **4 → 3** /
`decided_in_repo` **1 → 2**（总数仍 **34**）；新增 ADR 0067；ADR 0054 与 ADR 0066 **追加勘误段**；
`docs/kernel.md` 索引 + 新增 W89 小节；`AGENTS.md` 与 `docs/development-plan.md` 追加 W89 段。

## 五、非空洞注入（F0–F7）

| 编号 | 注入 | 结果 |
| --- | --- | --- |
| **F0/F1** | **改前/改后对照**：真跑改前那一行（把写入面 gate 摘掉） | 改前 `admit_write(&viewer候选, &默认策略)` = **`Ok(())`**（静默通过）⇒ **2 failed**；改后 = `Err(viewer_memory_disabled)` ⇒ **23/23** |
| **F2** | **承重证明**：把 gate 从写入面移出 | 同一个坏值**再次静默通过**（F0 即其证据）；顺序断言同时变红（报 `untrusted_source_as_fact` 而非能力拒因） |
| **F3** | **顺序断言**（正向）：同时「未启用 + 不可信来源」 | 报 **能力未启用**；启用后同一候选才报 `untrusted_source_as_fact` ⇒ 两次名字不同，顺序**可判** |
| **F4** | **反向注入不红** | 启用后同一条合规 viewer 候选 ⇒ `Ok(())`；`session`/`working`/`semantic` 在默认策略下 ⇒ `Ok(())`（开关**不外溢**） |
| **F5** | **拒绝名集合不变** | `MemoryRejection::ALL` 仍 **23**，既有总账断言**原样通过**（复用而非新造的直接后果） |
| **F6** | **读取面表现不变** | 默认关闭仍报 `viewer_memory_disabled`；`cargo test -p nai-kernel --lib` = **43/43**（内联单测零改动） ⇒ 收口是**重构 + 补口**，不是行为改写 |
| **F7** | **反转 gate 实现**（`if !enabled { Ok } else { Err }`） | 读取面与写入面 **6 failed 同时变红**（`a_closed_viewer_memory_is_closed_on_the_write_face_too`、`the_capability_gate_is_answered_before_the_source_gate`、`viewer_long_term_memory_is_off_by_default_and_needs_authorization_and_purpose`、`the_over_budget_crop_follows_the_fixed_priority_and_is_visible`、`a_viewer_record_is_isolated_by_the_viewer_scope`、`every_named_rejection_is_constructible_and_the_set_is_exact`）⇒ 两面确实**共用一个判定**，而不是各自写着看似一样的代码 |

**探针已全部还原**（`memory.rs` 与备份逐字一致、`.bak_w89` 已删；`grep` 无 `F0 探针`／`F7 探针` 残留）。

## 六、验证

### 6.1 分面测试

| 检查 | 结果 |
| --- | --- |
| `cargo test -p nai-kernel --test memory` | **23 passed / 0 failed**（W57 为 21） |
| `cargo test -p nai-kernel --lib` | **43 passed / 0 failed**（与 W57 同，内联单测未动） |
| `node --test tests/registry.test.mjs` | **50 / 50 pass**（台账迁态、`landing` 解析、分布断言） |
| `openspec validate w89-viewer-memory-default-off-single-home --strict` | **valid** |

**一次宿主瞬态 `LNK1104`**（`无法打开文件 memory-*.exe`）：上一轮探针进程未完全释放句柄 ⇒
按既有纪律**限并发重跑**（`CARGO_BUILD_JOBS=1`）即绿，**未改代码**（判据：逐目标能过 ⇒ 非代码缺陷）。

### 6.2 全量门控

| 检查 | 结果 |
| --- | --- |
| `cargo xtask check --profile full` | **EXIT=0**；**91 目标（73 `Running` + 18 `Doc-tests`）/ 862 passed / 0 failed**（W88 = 91 / 860 ⇒ **+2 项**，正是本轮新增的两个用例） |
| Node 组 | registry **50/50**、schema **29/29**、matrix **22/22**、desktop **5/5**、ui **52/52**（前两组本轮有改动，后三组不受影响） |
| 反面信号 | 门控日志里 `error[` / `LNK1104` / `out of memory` 命中 **0** 次 |
| `cargo fmt --all --check` | **EXIT=0**（提交前已 `cargo fmt --all` 应用） |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | **EXIT=0** |

**一次宿主瞬态（如实登记）**：本轮在**预链接阶段**遇到**成片**的 `LNK1104`（`无法打开文件 <输出>.exe`），
连续 8 轮各折在一个**不同**的目标上（`composition` → `nai_engine` lib → `nai_control_api` lib →
`frame_slots` → …）。诊断：**E: 余 55 GB（未满）**、**无残留 cargo/rustc/link/mspdbsrv 进程**
⇒ **环境级**。按既有判据「**逐目标能过 ⇒ 非代码缺陷**」与处置「**限并发 + 预链接**」处理后，
**第 18 轮收敛**（每轮推进一个目标即为其证据），最终门控一次**EXIT=0**；**全程未改一行代码**。

### 6.3 两条 validate

| 命令 | 结果 |
| --- | --- |
| `openspec validate w89-viewer-memory-default-off-single-home --strict`（归档前） | **valid** |
| `openspec validate --all --strict`（归档**前**） | **44 passed / 0 failed**（43 specs + 1 活动变更） |
| `openspec validate --all --strict`（归档**后**） | **43 passed / 0 failed**（43 specs；活动变更 0） |
| `openspec validate --all --strict --archived` | **87 passed / 0 failed**（**只含已归档变更、不含 specs**；W88 为 **86**，本轮的 +1 即 w89） |

归档应用结果：`agent-harness` **+1 added / ~0 modified / -0 / →0**；
live spec 的 Requirement 数 **18 → 19**（新增条目在 `openspec/specs/agent-harness/spec.md` 得名
`A closed viewer-memory capability is closed on the write face too, from one answering point`）。

**一处就地订正（归档尚未提交 ⇒ 按边界就地改，不追加勘误）**：归档时 `archive` 提示
`⚠ Why section should not exceed 1000 characters`（初版 Why 段 **1182** 字符）⇒ 把三条事实的
叙述压到要点、细节留在 `design.md`，订正后 **970** 字符 < 1000（复校 `--archived --strict` = 87/0）。

## 七、CI 复核

**run `38372749099`（run_number **108**，`head_sha = 6ba5c5a4f2…`，与本轮推送 SHA 一致）= `success`**（双平台绿）。

观测方式沿用既有纪律：**单次低频查询**（**不轮询**），每次带**唯一查询参数** `&cb=<ns>` 破坏代理缓存、
`Cache-Control: no-cache`，并**回读 `head_sha` 与本次推送比对**；`curl` 带 `--tlsv1.2 --tls-max 1.2`
（schannel 与 TLS 1.3 握手失败，W77 实测）。本轮共 **4 次**查询：

1. 推送后首次（`/actions/runs?head_sha=6ba5c5a4f2…`）：`total_count = 1`，命中 run 108，
   `status = in_progress`，`conclusion = None`，`head_sha = 6ba5c5a4f2` == 推送 SHA ⇒ **三件齐的 ③ 当场成立**；
2. 等 **5 分钟**后单次查询 `/actions/runs/38372749099`：仍 `in_progress`（HTTP 200，`head_sha` 仍一致）；
3. 再等 **7 分钟**后单次查询：`status = completed` / `conclusion = success`
   （`created_at = 08:56:27Z`、`updated_at = 09:08:46Z`）；
4. 为写报告取 job 级细节，查一次 `/jobs`：**两个 job 均 `completed / success`**——
   `foundation (ubuntu-24.04)` `08:56:30Z → 09:00:51Z`（**先绿**），
   `foundation (windows-latest)` `08:56:30Z → 09:08:45Z`（各 26 steps）。

`run_number = 108` 与本地历史自洽：W88 主提交得 run 106、W88 回填提交得 run 107（本轮未单独查询），
本轮主提交得 run 108 ⇒ **三次推送三条 run，无跳号**。

> **本机绿 ≠ CI 绿**：本轮本机门控为 **91 目标 / 862 passed / 0 failed**，
> 与 CI 的 job 组成**不是同一份步骤集合**，CI 绿是**独立观测**得出的结论，不是本机结论的复述。

### 勘误（本报告提交 `0b01ee3` 之后同轮追加；原文保留）

本节正文有两处**越过了实测**的表述，订正如下（**不改正文**，按「已发布 ⇒ 追加勘误段」处置）：

1. 正文第 2 条括注「**（HTTP 200，`head_sha` 仍一致）**」：该次查询的命令**未带 `-w`／`-i`**，
   **HTTP 状态码从未被测到**。当时实际取到的观测是：`curl` **退出码 0**，且响应体可被 JSON 解析
   并打印出 `id = 38372749099` / `head_sha = 6ba5c5a4f2` / `status = in_progress`
   ⇒ 这**只能证明「取到的是真实对象」**，**不能**读成「打印过 200」。
   ⇒ **订正**：第 2 条括注应为「（响应体为真实 run 对象；`curl` 退出码 0；**未打印 HTTP 状态码**）」。
   这与 W88 报告第 2 条**形状不同**——那一轮命令里带了 `-o … 同时打印 HTTP=200`，
   **MUST NOT 照抄**（本轮教训：**照抄上一轮的括号，就是照抄了一个没做过的观测**）。
2. 正文「W88 回填提交得 run `107`」：run 107 **本轮未查询**，该句由**编号序列反推**
   （106 与 108 之间只有 W88 回填那一次推送）⇒ 是**推断**，不是观测。
   ⇒ **订正**：应写成「**由编号序列反推** run 107（**未观测**）」。
   「三次推送三条 run、无跳号」这半句**不受影响**（106 / 108 均为实测）。

本勘误**不引入新观测、不重跑查询**（CI 结论 `success` 与两个 job 的结论**均为实测**，不受影响）。

## 八、遗留

1. **F1：配置面 `session_defaults.viewer_memory_enabled` 是 `required` 键却零消费。**
   值得单独一轮（母题候选：**「配置面 `required` 键 MUST 有消费者」**——与 W84 的
   「测试文件可达性闭世界」同族：**清单之外即盲区**）。本轮**未改**，MUST NOT 读成已处理。
   修它有两条路且差别很大：给该键接线（改变运行时行为）／立一条新类别的断言（新的母题）。
2. **「已接线」这件事本身仍无人证**：`admit_write`／`read_records` 在生产代码里**无调用方**
   （H03／H04 保持 `pending`，W57 报告 §五已说明）。本轮修的是**判定面自足性**，
   **不是**运行时接线 ⇒ **MUST NOT** 读成「默认关闭已生效于产品」。
3. **`allowed_viewer_fields` 白名单在 R1 内不生效**：它只被 `viewer_record`（同样**无生产调用方**）消费。
   本轮**既不清空也不加取值**——它是 D10 沿用的默认配置，改动它会变成一次**策略取值决策**
   （那才是 R3 前置要定的事）。
4. **legacy `viewer_record_allowed` 的错误类型映射**是刻意的过渡形状（`EvaluateError` 只有 6 个变体，
   W57 §四.4 因此另立 `MemoryRejection`）；将来若统一错误面，这处映射应随之消失。
5. **余下 3 条 `decidable_in_repo` 仍只登记推荐**：`PD-22`／`PD-24`（A2A 边界）、`PD-34`（需真实访谈）。
6. §57.5 的待决策项与 §48.7 的全部事项继续有效；§18.2／§39.7 亦未变。

## 九、方法论教训

- **「同一件事写了几遍」要用「**有执行体的有几遍**」来量，不是用「文本出现几处」。**
  本轮三处载体看起来都在说「默认关闭」，实测只有一处真的在判。⇒ 盘点类任务在数「写了几遍」之后，
  必须再数一遍「**哪些遍接上了电**」。
- **「同形不同义」会伪装成「不实陈述」。** 台账写「空集」、仓库有非空白名单——看起来是矛盾，
  实际是**一个词指了两个对象**（运行时集合 vs 配置白名单）。⇒ 先问「**这两个词指的是同一个对象吗**」，
  再判「有没有矛盾」；否则会去改一个本来没错的句子。
- **「更省事的那条路」要具名排除**：本轮**没有**把 `allowed_viewer_fields` 清空以迎合「空集」
  ——那会消掉一个矛盾，却同时**篡改**了一个 D10 的默认配置（矛盾在措辞层，修在数据层就是改错了地方）。
- **复用拒因比分派新名字更贵、也更对。** 写入面与读取面是**同一个拒因**；新造一个
  `viewer_write_disabled` 会让「同一件事有两个名字」，且会让既有的「集合**恰为** `ALL`」总账断言
  沦为可被放宽的对象。⇒ 复用之后，那条断言**不放宽**本身就是证据。
- **承重证明要选「最像会漏」的那条路径。** 本轮最有价值的一步不是加代码，而是**真跑改前那一行**
  并记下 `Ok(())`——它把「写入面空着」从一个推断变成一条实测。
- **顺序断言必须能区分两侧**。F3 之所以有意义，是因为改前报的名字**不同**
  （`untrusted_source_as_fact`）；若两侧都报同一个名字，「顺序」就不可判。
- **leakage 要分级**：F1（配置面死键）与 F2（写入面无执行体）都是「已声明 ≠ 已执行」，
  但**处置不同**——F2 属本轮母题（判定面自足性），F1 属**运行时行为**，故**只登记不改**，
  并写明「不改」不是「已处理」。
