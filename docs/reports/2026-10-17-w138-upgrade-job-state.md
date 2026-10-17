# W138：O02 的另一半——作业状态接上真实 journal

- 日期：2026-10-17
- 变更：`openspec/changes/archive/2026-10-17-w138-upgrade-job-state/`
- ADR：[0113](../adr/0113-upgrade-job-state.md)
- 基线：`docs/直播数字人 + 智能体系统技术方案 V3.11.md`（§30.1 第 4 条「升级与回滚提示」、§662 O02）

## 一、勘定（先量，再动手）

W137 结尾把 §30.1 第 4 条收窄成一句可判的话：**只缺 journal/执行状态那一半**。本轮去补那半，
现测出三件事，合起来决定了形状：

| 现测对象 | 结果 |
| --- | --- |
| `upgrade_journal` | **真实存在**：`crates/nai-storage/src/upgrade.rs` 的表、`UpgradeStep` 五态、`record_upgrade`/`upgrade_record`；`Store` 就是 `UpgradeJournal` 的生产实现 |
| `UpgradeHost` | **生产实现为零**：`impl UpgradeHost for` 全仓只有 `crates/nai-upgrade-core/src/rehearsal.rs:153` 的 `RehearsalHost`（注释自述 holds no bytes and touches no disk） |
| 控制面 `/jobs/{id}` | **零分支** ⇒ 落到 404 `CapabilityAbsent`（与「路径拼错」不可区分） |
| 契约 `/jobs/{id}` | 声明了 `job.read` + `*readResponses`，**没有** `x-implementation-status` |
| `*jobResponses` | 被 **14** 条路由复用（`grep -c` = 15，含锚点定义行），全部没有可轮询状态 |
| `Access::new` | `permissions.len() > 8` 是**单令牌条数**上限；全局名集已有 9 个（W137 的 `runtime.upgrade` 是第 9 个） |
| `CONTROL_PERMISSIONS` | 引擎签发的唯一令牌**用满 8 条**，且**不含** `runtime.upgrade` |

最后一条是本轮**意外撞上**的既有缺口：W137 把 `plan` 做成 `implemented-loopback`，但引擎令牌没有那个权限
⇒ 那条已实现的路由对**唯一签发出去的凭据**恒 403。控制面用例自造 `Access`，所以全绿。

**两处勘定的否决**（都写进了 ADR 的 Alternatives）：

- 给 `/upgrades/execute` 铸 `job_id` 并写一条 `prepared` 记录再 202：`Prepared` 之后一步都推不动，
  那条记录会永远停在 `prepared`，而读数会让「准备好了」看起来像「正在升级」。
- 新增第三个 `x-implementation-status` 值（`declared-not-implemented`）：`operationErrors` 的判据是
  `expected = declaredSet.has(key) ? IMPLEMENTED_STATUS : undefined`，即「不在已实现表里 ⇒ MUST 没有该标记」。
  加第三个值必须先放宽这条闭世界——用削弱判据换表达力，而既有表达已经够用（**没有标记**就是「已声明未实现」）。

## 二、交付

### 2.1 契约（`schemas/openapi/control.yaml`、`schemas/core/error.proto`）

- 新增 `ErrorCode NOT_FOUND = 17`。16 个码里**没有**「所命名的对象不存在」这一格，于是
  「该 id 不命名任何事务」与「作业面未接入」只能共用一个响应。`tests/contracts/descriptor.test.mjs:48`
  已有「openapi 错误枚举 == proto 枚举」的耦合断言 ⇒ 改一处漏一处当场红。
- 新增 `JobView`（`additionalProperties: false`，`step` 五值闭集且注明**含顺序**）。`backup_ref`/`error_code`
  是**可选**而非 nullable：它们在某些步骤上真的不存在，必填会逼出一个占位值。
- `/jobs/{id}` 改为 `200 + JobView` / `404 NotFound` / `503 ServiceUnavailable`，标 `implemented-loopback`。
  新增 `NotFound` 与 `ServiceUnavailable` 两个响应组件（此前全仓没有声明过 404/503，
  而 `approvals_list` 早就在返 503——这一轮把两个组件补上，不是新造语义）。
- `/upgrades/execute`、`/upgrades/{id}/rollback` 增 `503` 声明与描述（点名缺平台安装器），
  **仍不**标 `implemented-loopback`。
- `scripts/contracts/openapi-ops.mjs` 的 `implemented` 增 `get /jobs/{id}`（9 → 10 条）。
- 生成物本机再生：`cargo xtask schema` ⇒ `schema --check` **no drift**（33 能力 / 112 消息 / 10 源）。

### 2.2 控制面（`crates/nai-control-api`）

- `JobBackend`（持 `Arc<Mutex<Store>>`，照 `ApprovalBackend` 同形；`upgrade_journal` 是实例级、无 workspace，
  故不需要 workspace/actor）+ `JobView` + `From<UpgradeRecord>`。
- `ControlApi::with_jobs` 链式装配；`jobs_get` 三态；`upgrade_refusal` 具名拒绝。
- 权限表：`get /jobs/{id} => job.read`；execute/rollback 的 `runtime.upgrade`。
- `Access` 的条数上限从 `PERMISSION_VOCABULARY` 派生，并**拒绝表外权限名**——拼错的授权在构造时就失败，
  而不是在调用时变成一个说不出原因的 403。
- `from_contract` 增 `NotFound => 404`、`CapabilityNotReady => 503`，并**保留**生产方给的 `category`
  （原先一律改写成 `control`，等于让作业面的存储故障挂在控制面名下）。
- `crates/nai-storage`：`UpgradeStep::ALL`（五态单一列举），供「契约值域 == 枚举」判据取用。

### 2.3 装配（`apps/engine`）

`CONTROL_PERMISSIONS` 补 `runtime.upgrade` 与 `job.read`（8 → 10 条）；`.with_jobs(JobBackend::new(store.clone()))`
接上装配根**同一把** store 锁——复制品会让刚写进去的那一步看不见。

### 2.4 桌面（`apps/desktop`，零 DOM）

- `control.ts`：`JOB_STEPS`/`JobStep`/`JobView`/`parseJobView`（未知步骤**具名拒绝**，`terminal` **不重算**）+ `ControlClient.fetchJob`。
- `upgrade.ts`：`JOB_STEP_LABELS`（键集 MUST 等于 `JOB_STEPS`）、`jobStatusNote`（未知步骤有「不做猜测」的出口）、
  `ROLLBACK_UNAVAILABLE_NOTE`。
- `console.ts`：`upgrade_rollback_prompt` 的 `missing` 从「无 plan/journal 来源」改为真实缺口
  （无渲染面 + 无写入方），仍 `unimplemented`。

## 三、判据

### 3.1 控制面（`crates/nai-control-api/src/tests.rs`，39 → 49，**+10**）

| 判据 | 钉住什么 |
| --- | --- |
| `a_job_surface_that_is_not_wired_is_not_ready_not_absent` | 无后端 ⇒ 503（不是 404），`category=job`，含 `journal` 锚点 |
| `a_job_the_journal_never_recorded_is_not_found` | 404 `NOT_FOUND`，且响应里**没有** `step` |
| `a_job_is_reported_as_the_journal_recorded_it_and_satisfies_the_contract` | 200 且满足自己声明的 `JobView`（两层 `additionalProperties:false`） |
| `every_step_the_journal_can_hold_is_reported_verbatim` | 五态逐字 + `terminal` 逐态对照 `step.terminal()` |
| `the_contract_step_vocabulary_equals_the_journal_enum` | 契约值域 == `UpgradeStep::ALL`，**含顺序**，且每值可 `parse` |
| `the_backup_reference_appears_only_after_it_was_taken` | 没备份 ⇒ **缺键**，不是空串 |
| `the_jobs_route_needs_the_job_permission` | 403 且无 `step` |
| `execute_and_rollback_refuse_by_name_and_mint_no_job_id` | 两条都 503，无 `job_id`，含 `UpgradeHost` 与 `installer` 锚点 |
| `an_id_that_cannot_name_a_transaction_is_a_bad_argument` | 超长 id ⇒ 400 `INVALID_ARGUMENT`（不是 404） |
| `a_locked_journal_is_a_conflict_not_a_missing_job` | 持锁 ⇒ 409 `CONFLICT`（读不到 ≠ 没有） |

### 3.2 桌面（`tests/ui/upgrade-job.test.mjs`，新增 **9**）

信封严格（八个键、六个必填）、未知步骤具名拒绝、可选项只认缺省、`JOB_STEP_LABELS` 与 `JOB_STEPS` 两集相等、
终态与未竟分开说、未知步骤出口、`ROLLBACK_UNAVAILABLE_NOTE` 三点内容锚点、客户端只发 GET 且不带体、空 id 不发请求。

### 3.3 注入验证（**7** 组，逐组真红，逐字节还原）

| # | 注入 | 变红的判据 | 只红这一条？ |
| --- | --- | --- | --- |
| I1 | 未知 id 回 200 空对象 | `a_job_the_journal_never_recorded_is_not_found` | 是（48 绿 / 1 红） |
| I2 | 无后端答 404 | `a_job_surface_that_is_not_wired_is_not_ready_not_absent` | 是 |
| I3 | execute 铸 `job_id` 并 202 | `execute_and_rollback_refuse_by_name_and_mint_no_job_id` | 是 |
| I4 | 拒绝消息缩成 `unavailable`（非空、无锚点） | 同上 | 是（**证明「只判非空」不够**） |
| I5 | `terminal` 改成 `step == "committed"` | `every_step_the_journal_can_hold_is_reported_verbatim` | 是（只测 `committed` 的实现会全绿） |
| I6 | 删掉 `job.read` 权限行 | 整组作业判据 + `the_jobs_route_needs_the_job_permission` | 否（权限是入口闸门，多条同红——如实记录） |
| I7 | 只**调换**契约枚举里两个值的顺序（集合不变） | `the_contract_step_vocabulary_equals_the_journal_enum` | 是（证明「顺序也钉住」是判别性的） |

基线是 fmt-clean 的最终字节；7 组还原后 `sha256sum -c` 五个文件逐字节 `OK`，`schema --check` 仍 no drift。

## 四、门控

`cargo xtask check --profile full` **`GATE_EXIT=0`**（`CARGO_INCREMENTAL=0` + `CARGO_BUILD_JOBS=1` + 机外 `protoc 21.12`）：

- Rust **101** 个目标 / **1122 通过 / 0 失败**（W137 为 1112，**+10** = 本轮新增的 10 条控制面判据）。
- Node：13 个非前端套件 **291** + `apps/desktop → test` **5** + `test:ui` **71**（W137 为 62，**+9**）= **367 通过 / 0 失败**。
- 日志 **4126** 行内 `LNK1104` 与 `^(error|failures:)` 均**零命中**。

门控前有两次红灯，都不是代码缺陷，处置都按既有判据：

1. **clippy 真红一次**：`if_same_then_else` —— 我给 `/upgrades/plan` 与 execute/rollback 各写了一条
   `runtime.upgrade` 分支，clippy 指出两块相同。**这一条是代码问题**，改法是把三个路径合并进一条条件，
   MUST NOT 用 `#[allow]` 盖掉。
2. **`LNK1104` 宿主瞬态，共 8 次、目标 6 个互不相同**（`render`、`nai_budget_core`、`protocol`、
   `candidate_hash`、`control_http`、`nai_contracts`）。判据：报错指向的 exe **在盘上不存在**
   （同目录其它哈希的 exe 都在）、磁盘 39G 可用、无残留链接进程、逐目标单跑全绿 ⇒ 非代码缺陷。
   处置 = **有界重试**：先 `cargo build --workspace --all-targets` 串行预构建（第 1 次即过），
   再 `cargo test --no-run` 有界重试补链接（第 5 次过），最后门控一次即绿。
   **未改代码、未放宽判据、未改测试期限常量。**
   ⚠️ 「预构建过一次」不等于「后面不再红」：`cargo test` 会重新链接自己的哈希产物，所以预构建与门控
   要分别有界重试——这条与 W136/W137 的记录一致。

## 五、具名登记（发现但不做）

1. **12 条同族路由**：`knowledge/import`、`knowledge/snapshots/{id}/publish`、`knowledge/evaluate`、
   `knowledge/documents/{id} delete`、`providers/{id} put`、`providers post`、`avatar post`、`scene put`、
   `scene post`、`scene delete`、`devices post`、`backups post`、`restore/preflight` —— 除 upgrades 两条外
   复用 `*jobResponses` 的 12 条，同样铸 `job_id` 而无状态来源。各自缺的是**自己的状态表**；
   本轮没有为它们设计状态形状，故不修、也不新造一条会当场变红的判据。
2. **提交后的回滚语义未实现**：journal 的 `permits` 拒绝一切终态迁移 ⇒「已提交的升级再回退」必然是
   **一笔新事务**（`from` = 旧记录的 `to`），不是对旧记录的状态迁移。记在 ADR「未做」里，
   免得后来者以为 journal 漏了一个状态。
3. **`UpgradeStep::ALL` 与枚举可能不一致（残余风险）**：判据能钉住「契约 == `ALL`」与「每值可 `parse`」，
   但钉不住「有人加了一态却忘了改 `ALL`」——那时三者一起保持旧值，全绿。构造性盲区，只能靠注释 + ADR 具名，
   **MUST NOT** 记作「已注入验证」。
4. **两个后端的 `try_lock` 共用同一把 store 锁**：并发时会把对方判成 409。属既有形状的继承，
   改它要重设计锁粒度，不在本轮。
