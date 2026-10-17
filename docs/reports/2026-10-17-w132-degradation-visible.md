# W132：H17 的 runtime 接线——把「层失败」送到界面

- 日期：2026-10-17
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`（§12.2、§47.4、§48.5；INV-32）
- ADR：`docs/adr/0110-degradation-visible.md`
- OpenSpec：`openspec/changes/archive/2026-10-17-w132-degradation-visible/`

## 一、缺口（现测，MUST NOT 由「同族」推断）

| 现测项 | 结果 | 证据 |
|---|---|---|
| `evaluate_degradation` 的生产调用点 | **零** | `grep -rn evaluate_degradation` 只命中 `crates/nai-kernel/tests/harness.rs` |
| `assert_no_free_form` 的生产调用点 | **零** | 同上，只被测试引用 |
| `DegradeVerdict::visible` 的取值 | **恒 `true`** | `evaluate_degradation` 无条件写 `visible: true` |
| `CapabilityView` 能否承载降级 | **不能** | proto 三条字段 `session_id`/`revision`/`capabilities`；`CapabilityDescriptor` 描述**插件能力** |
| `decision_log` 的 kind CHECK | 13 个名字，**无 `degradation`** | `migrations/sqlite/0007-decision-log-assembly-kind.sql` |
| 本机能否改 proto | **不能** | `cargo xtask schema --check` 需 protoc 3.21.12（工具链前提） |

⇒ 缺口**不是**「算不出落到哪一级」（W66 已建、**24 条**覆盖，现测 `HEAD` 的 `grep -c '^\[test\]'`），而是**那条结论没有 transport**。
`visible` 是构造常量 ⇒ 「无静默降级」钉在判定面**恒绿**，是装饰；会坏的是判定到界面之间那一段。

四环定位：①判定面（W66 已建）→ ②**执行体（本轮新建）** → ③调用方（呈现层，未闭合）
→ ④真实产生者（真实界面与真实模型，未接线）。

## 二、改动与判据

### 2.1 承载物

- **`DecisionKind::Degradation`**（`crates/nai-storage/src/decision.rs`）：判为 **critical**。
  telemetry 写失败会落进**遥测缓冲**、调用方只看到 `persisted == false`；一条能悄悄消失的降级
  正是 INV-32 要禁的静默降级。W104 已踩过一次「只改枚举不改 CHECK ⇒ 插入被 schema 拒绝并**静默**落入缓冲」的缝，
  本轮两处一起改。
- **迁移 `0011-decision-log-degradation-kind.sql`**：SQLite 改不了 CHECK ⇒ 逐列重建表、
  逐字复制行（含四个哈希列）、重建两个只追加触发器。链不变——`verify_decision_chain` 的前像从不提表名。
- **版本号成套六处 10 → 11**：`migration::VERSION` / `SQL` 数组 + `include_str!` /
  `backup.schema.json` const / Rust 契约样本断言（`tests/knowledge_publication.rs`）/
  `tests/contracts/schema.test.mjs` 备份样本（含「上一版 MUST 被拒」9→10）/ TS 生成物
  `packages/contracts-ts/src/generated/config/backup.ts`。

### 2.2 呈现层承载物（`crates/nai-kernel/src/evaluate/harness.rs`，新增）

`DegradationNotice` + `render_degradation_notices`：

- 一条判定 ⇒ **恰好一条**通知；**不去重、不合并**（同一层失败两次是两个降级事件，合并会吞掉一次）。
- 每个字段**照抄**判定输出（层标签 / 错误码 / 落点级 / 可行动说明 / 是否拒绝调用 / 丢了什么 / 已有会话是否受影响）。
  渲染器不重算 code、level 或说明。
- 六层的可行动说明**互异**——一个对所有层通用的「已降级」是「可见但什么也没说」。
- 空判定集 ⇒ 报 `0`。

### 2.3 接线（`crates/nai-runtime/src/degradation.rs`，新增）

- `Composition::record_degradation(failures, available, retained)`：
  入参是**事实**（哪些层失败了 / 还剩什么可用 / 上下文还留着什么），**不是判定结果**。
  「落到哪一级」由 `evaluate_degradation` 在**生产路径上**算出。
  接收判定结果 ⇒ runtime 侧不再有任何东西保证这个映射（那是「注入判定」）。
- 判定**先于写入**：`assert_no_free_form` 在本入口执行；违规 ⇒ 具名拒绝、**零落盘**。
  拒绝侧携带「本来会显示什么」便于检查，但明确不落盘。
- 无失败 ⇒ 不落盘（零降级没有可显示的东西）；判定**照跑**——这是「未检查」与「检查过为空」的区别。
- `Composition::read_degradation_view(limit)`：**只**从链上解码，不接受调用方的答案。
  不可解析 ⇒ 具名失败（MUST NOT 静默跳过）；空链 ⇒ 报 `0`；域 = `degradation` 那一种。
- `DegradationView.refuses_call` 由 notices 派生：§47.4 的「装配层拒绝该次调用」是界面 MUST 说的一部分。

### 2.4 刻意不做（三条）

1. **不扩展 `CapabilityView` 契约**。`CapabilityDescriptor` 描述**插件能力**；把 harness 的层失败
   映射进去要编造 `capability_id`/`plugin_id`，且 `PluginState` 里没有对应取值——那是在说
   「一个插件失败了」，而事实是「某一层失败了」。不实陈述比缺口更贵。另需 protoc 3.21.12（本机无）。
2. **不新增 INV-32 的违规类**。两类定义已由 W66 给出，本轮只把它们放到生产路径上执行。
3. **不替呈现层决定排版**。渲染不去重是「不吞输入」的可判事实；界面怎么排是它自己的事。

### 2.5 判据绿（fmt-clean 之后）

fmt 先跑 `--check` ⇒ **发现 3 处红**（新用例的 `assert_eq!` 需折行、`use` 排序）⇒ 就地修掉**再**取基线
（W129 的坑：基线 MUST 是 fmt-clean 的最终字节）。

| 面 | 计数 |
|---|---|
| `cargo test -p nai-kernel --test harness` | **29/29**（**24 + 5** 新） |
| `cargo test -p nai-runtime --test degradation` | **9/9**（新文件） |
| `cargo test -p nai-storage --lib`（`RUST_TEST_THREADS=4`） | **82/82**（81 + 1 迁移） |
| `node --test tests/contracts/schema.test.mjs` | **30/30** |
| `node --test tests/registry.test.mjs` | **56/56** |

⚠️ `nai-storage --lib` 在**默认并行度**下 1 条红：
`corrupted_database_artifact_and_traversal_never_activate_destination` 报
`DeadlineExceeded / operation stopped`；`RUST_TEST_THREADS=4` 下 **82/82 全绿**。W84 同族（期限敏感断言
在 CPU 争抢下误报），根因未修、本轮亦未修，处置是限并发 + 四条替代判据，**不改代码**。

## 三、注入验证（每条判据都要证明它会响）

基线 = fmt-clean 的最终字节；4 个被注入文件全部 `sha256sum -c` **逐字节还原**。

| # | 注入 | 红的用例 | 条数 |
|---|---|---|---|
| K1 | `actionable` 换成通用串「已降级」 | `a_notice_carries_the_judged_fields_verbatim`、`every_layer_renders_a_distinct_actionable_note` | 2 |
| K2 | 渲染 `.take(1)` | `one_verdict_renders_to_exactly_one_notice`、`rendering_does_not_collapse_repeated_failures`、`every_layer_renders_a_distinct_actionable_note` | 3 |
| K3 | `drops` 恒 `None` | `a_notice_carries_the_judged_fields_verbatim` | 1 |
| K4 | 写「首选级」而非「回退后的级」 | **只有** `a_failure_whose_preferred_level_is_unavailable_shows_the_level_it_walked_down_to` | 1 |
| R1 | 不可解析改 `unwrap_or_default()` 跳过 | `an_undecodable_degradation_row_is_a_named_failure_not_a_skip` | 1 |
| R2 | 不跑 `assert_no_free_form` | `..._lost_its_policy_constraint_is_refused_and_appends_nothing`、`..._lost_its_persona_is_refused_by_its_own_name` | 2 |
| R3 | 去掉 kind 过滤 | `only_degradation_rows_enter_the_view` | 1 |
| R4 | 载荷键写成 `error_code` | `every_failure_layer_reaches_the_chain_and_reads_back`、`the_read_back_view_equals_what_was_judged_field_by_field`、`a_failure_whose_preferred_level...`、`an_assembly_failure_still_refuses...` | 4 |
| R5 | `refuses_call` 恒 `false` | `an_assembly_failure_still_refuses_the_call_after_a_round_trip` | 1 |

**9 条注入、8 条真红。**

### 3.1 K4：ADR 0109 的第二个方向，再实证一次

既有 8 条 runtime 用例**全部**用 `DegradeContext::all_available()`。在该输入下「层首选级」与
「回退后落点级」**构造性相等**（首选级一定可用，回退不发生）⇒ 写 `preferred_level()` 的坏实现让
**kernel 29/29 全绿**、**runtime 既有 8 条全绿**，只有本轮**新补的**回退用例红。

这不是「被更早的闸门短路」（ADR 0109 的第一个方向），而是**没有任何既有输入能走到那一层**。
处置是**补一条能走到那一层的输入**
（`template_available: false, approved_cache_available: false, human_available: true`），
MUST NOT 改判据去迁就。⇒ 先问「有没有输入能走到那一层」，再问「它是不是构造性事实」。

### 3.2 R2 首跑假绿：第三种成因

首版注入写成 `if !violations.is_empty() && !cfg!(test)`，结果 **9/9 全绿**。成因：`cfg!(test)` 在
**生产 lib**（`degradation.rs` 不是测试 crate）里恒为 `false` ⇒ 注入**没有改到任何行为**。
改成 `&& verdicts.is_empty()` 后红 2 条。

⇒ 「注入不变红」除「被更短的闸门短路」「判据根本没有覆盖那一层」外，还有**第三种：注入自身坏了**。
**注入后全绿 MUST 先怀疑注入**，MUST NOT 直接记作「判据无判别力」。

### 3.3 由**构造**保证、因此不需注入的事实（如实标明，MUST NOT 记作「已注入验证」）

- `DegradeVerdict::visible` 恒 `true`：判定面没有「不可见」这一格。这不是缺陷，但它意味着
  「无静默降级」**不能**由判定面的断言证明——本轮因此把判据放到 transport 上。
- `render_degradation_notices` 的长度不变式（输出等长于输入）在 K2 里被注入验证；
  「字段照抄」在 K1/K3 里被验证。

## 四、registry 判定（`H17` **仍 `pending`**）

四环：①判定面（W66 已建、**24 条**）②执行体（本轮新建、9 条）③调用方（呈现层渲染，**未接线**）
④真实产生者（真实界面与真实模型，**未接线**）。

⇒ 本轮交付的是「降级项清单**存在且可被界面读到**」（有渲染形状、有落盘、有读回面），
**不是**「界面已经显示了它」。**验收仍 `pending`**，与 H15/H01/H02 接线轮惯例一致：接线到 runtime ≠ 验收达标。

具名登记（本轮发现但不修）：

1. **真实界面仍未接线**（需契约扩展 + 壳体 IPC）。
2. **关键种类的清单是手写的**：`crates/nai-storage/src/tests/decision_log.rs` 的
   `critical_failures_surface_and_never_enter_the_buffer` 逐字列出四个 `DecisionKind`；本轮新增第五个
   critical 种类而它**没有变红**——它不声明关键种类的闭世界，故「新增一个 critical 种类」在那里无信号。
3. **`required_str` 在 `assembly.rs` 与 `degradation.rs` 各有一份**：合并要改 `assembly.rs` 的 10 处
   调用点，属范围外。
4. **期限敏感断言在 CPU 争抢下误报**（W84 同族，根因未修）。
5. **`degradation` 判 critical 的代价**：写入失败变硬错误而非缓冲，频繁工具失败会把它暴露在热路径上。
   这是刻意付的代价。

## 五、门控（`cargo xtask check --profile full`）

**绿，`GATE_EXIT=0`，744s（12m24s）**，本机 `RUST_TEST_THREADS=4`（与 W131 起 CI 的取值一致）。

| 项 | 本轮 | W131 | 说明 |
|---|---|---|---|
| Rust | **1073 passed / 0 failed** | 1058 | **+15 == 5（kernel）+ 9（runtime）+ 1（迁移）**，逐项吻合 |
| `test result` 行数 | 99 | 98 | +1 = 新增的 `degradation` 测试目标 |
| `^     Running ` 目标 | **82** | 81 | 同上；另 1 行是 cargo 自回显的 xtask 包装行 |
| `^   Doc-tests ` | **18** | 18 | 不变 |
| Node | **13 套件 324/324，0 failed** | 324/0 | 不变 |
| 前端门控 | **6 步全通过**（`[frontend] 全部步骤通过`） | — | contracts-ts 与 desktop 各一次 `typecheck`/`build`，加 `test` 与 `test:ui` |
| 错误类（`LNK1104`／`^error[`／`out of memory`／`FAILED`／`panicked`） | **0** | 0 | — |
| 非 `success` 且非 `skipped` 的步骤 | **无** | — | — |

⚠️ 一处计数订正：本轮 kernel 的 `tests/harness.rs` 是 **24 → 29（+5）**，不是此前记的「25 + 4」。
发现它的正是「增量对不上」——门控总量 +15，而按「+4」算是 +14 ⇒ 回 `HEAD` 现测
`grep -c '^#\[test\]'` = **24** ⇒ 本轮新增**五**条（`one_verdict_renders_to_exactly_one_notice`、
`a_notice_carries_the_judged_fields_verbatim`、`every_layer_renders_a_distinct_actionable_note`、
`rendering_does_not_collapse_repeated_failures`、`an_empty_verdict_set_renders_zero_notices`）。
⇒ **门控总数是比任何一行注释都硬的核对点**：先记「新增几条」再拿总数对账，MUST NOT 只信注释。

## 六、提交与推送

一次提交：**`3bc7aa2d4492436ef914206a5b3faee06a35c033`**（`feat(w132): H17 的 runtime 接线——把「层失败」送到界面`），
**22 文件，+1481 / −12**：

| 组 | 文件 |
|---|---|
| 承载物 | `migrations/sqlite/0011-decision-log-degradation-kind.sql`、`schemas/config/backup.schema.json`、`packages/contracts-ts/src/generated/config/backup.ts`、`tests/contracts/schema.test.mjs`（升版六处成套，`db_schema_version` 随迁移 +1） |
| 呈现层 | `crates/nai-kernel/src/evaluate/harness.rs`（新增，5 条渲染判据） |
| 接线 | `crates/nai-runtime/src/degradation.rs`（新增） |
| 登记 | `tests/registry.yaml`（H17 记 `pending`，判定见 §四） |
| 规格 | `openspec/changes/2026-10-17-w132-degradation-visible/{proposal,design,tasks}.md` + `specs/agent-harness/spec.md`，并入 `openspec/specs/agent-harness/spec.md` |
| 文档 | 本报告、`docs/adr/0110-degradation-visible.md`、`docs/development-plan.md`、`docs/knowledge.md` |

推送至 `origin/main`（本仓库走 SSH 443）。**推送这一事实不由命令输出承担、而由 CI 承担**：
run `39004921781` 的 `head_sha` 就是 `3bc7aa2…`——没推上去就不会有这个 run（§七）。

## 七、CI 观测（触发确认 1 次 + 完成态 ≥2 次）

按三件齐观测（唯一查询参数 + `Cache-Control: no-cache` + 回读 `head_sha` == 推送 SHA）：

- run **`39004921781`**（`Engineering foundation`），`status: completed` / `conclusion: success` /
  `run_attempt: **1**` / `head_sha` 回读 == `3bc7aa2d4492436ef914206a5b3faee06a35c033`；
- 触发到完成：`05:04:30Z → 05:23:02Z`（**18m32s**）；
- **逐平台看完整步骤序列**（不只看「非 success」，否则会被 skipped 淹没）：

| 平台 | 结论 | 成功步 | 跳过步 | bad |
|---|---|---|---|---|
| `foundation (ubuntu-24.04)` | **success** | 26 / 27 | `Publish the gate failure summary as annotations`（仅失败时执行） | **0** |
| `foundation (windows-latest)` | **success** | 24 / 27 | 同上 + `Install Linux C++ protobuf runtime`、`Full three-language contract suite (Linux C++ toolchain)`（Linux 腿专属） | **0** |

耗时：ubuntu `05:04:33 → 05:12:12`（**7m39s**），windows `05:04:33 → 05:23:02`（**18m29s**）。

⚠️ **口径提示（不拿数字硬算）**：本 run 两腿的 `jobs[].steps` 各 **27**，而当前（W136）同 workflow 各 **20**。
这两组都是 **API 口径**——它把 `Set up job` / `Post Run actions/checkout` 等**包装步**也算作 step
（workflow 文件今天只有 13 条实步：3 个 `uses:` + 10 个 `- name:`）⇒ API 步数与文件步数**不是同一口径**。
W134 删掉的 **7 条 `test --suite …` 重复步**（报告口径是 `foundation.yml` 22 → 15）恰好也是 7，
但两处同为 7 **MUST NOT 相减或互证**：一边含包装步、一边是文件里的步。

**能同口径比的只有耗时**（同一 API、同一 workflow、同一矩阵，两次都 `attempt 1`）：

| 平台 | W132（run `39004921781`） | W136（run `39032204063`） | 差 |
|---|---|---|---|
| `ubuntu-24.04` | **7m39s** | **5m58s** | −1m41s |
| `windows-latest` | **18m29s** | **11m43s** | **−6m46s** |

差额方向与「删掉 7 条重复步」一致，但**不主张精确收益**：两次的依赖缓存命中不同，W134 §1.3 已记过
同一条纪律（472 秒不与 517 秒相减）。
