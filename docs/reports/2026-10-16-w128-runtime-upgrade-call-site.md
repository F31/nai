# W128 报告：装配根驱动升级事务（R10 调用点收口）

- 轮次：W128｜日期：2026-10-16｜归属：`tests/registry.yaml` **R10**（开播与回滚不丢写入）
- 关联：[ADR 0106](../adr/0106-runtime-upgrade-call-site.md)、ADR 0101／0100／0099、
  `openspec/changes/2026-10-16-w128-runtime-upgrade-call-site/`、
  审计 `docs/reports/2026-10-16-state-of-implementation-audit.md`（缺口③：仓库内可闭环）

## 一、缺口（现测）

**缺口逐字来自 registry，不来自散文。** R10 的 W123 段原话：
「**运行时仍无升级激活调用点**（`nai-runtime`/`apps/engine` 内无 `UpgradeTransaction` 使用处）
⇒ 「开播中只暂存不激活」尚未在真实运行时链路上被执行」。

**现测（本轮前）**：`grep -rn "UpgradeTransaction" crates apps xtask` 的命中只有
`nai-upgrade-core`（`plan.rs` 定义、`lib.rs` 再导出、`download.rs` 文档提及）与其
`tests/{plan,gates}.rs` ⇒ 核心有 `activate`、装配根有 `SessionGate`（W123），**没有任何一处把两者接起来**。

**前置障碍（决定了本轮的第一步）**：`Prepared` 在核心之外**无法字面构造**——字段虽全 `pub`，
但既无构造器，`PreflightReport::checks` 又是私有。故调用点的判据**只能**通过一次**真的** `prepare` 得到它，
也就必须拿到那份 demo 包；而 `nai-runtime` 的 dev-dependencies **够不到另一个 crate 的 `tests/`**。
于是先做夹具共享化：把散在 `nai-upgrade-core/tests/{plan,gates,upgrade}.rs` 的**三份逐字重复**的
demo 夹具（`demo manifest` JSON + `CONTENT` + `facts()`）提到 `src/rehearsal.rs`。

## 二、改动与判据（真实证据）

### 2.1 夹具共享化（`nai-upgrade-core`，纯增加）

`src/rehearsal.rs` 新增 `RehearsalPackage`（`new`/`with_cost`/`manifest`/`host`/`request`）、
`DEMO_CONTENT`、`demo_cost`、`demo_manifest`、`demo_facts`；`lib.rs` 扩导出。
- 无新依赖（`serde_json`/`sha2` 已在 `[dependencies]`）。
- 三处重复消除的现测：`plan.rs` / `gates.rs` / `upgrade.rs` 中 `fn facts()` 与 `fn manifest(` 命中数**均为 0**。
- **用例数逐项不变**：plan **12**、gates **7**、upgrade **8** ⇒ 这是纯重构，判据未增减；
  core 全量 **99 passed / 0 failed**（3+21+20+7+17+11+12+8）。
- `crates/nai-runtime/tests/common/mod.rs` 同时收敛了 `hold_session` 的两处重复定义。

### 2.2 运行时调用点（生产代码，只增不改）

新增 `crates/nai-runtime/src/upgrade.rs`：`UpgradeActivation` + `Composition::stage_upgrade` / `activate_staged`；
`lib.rs` 导出 `UpgradeActivation` 并补 `From<UpgradeError> for RuntimeError`（与既有 `From<StorageError>` 同形）。

| 位置 | 现测计数 | 新增 |
| --- | --- | --- |
| `crates/nai-runtime/tests/upgrade.rs`（**新目标**） | 0 → **5** | 5 条 |
| `crates/nai-runtime` 合计 | 91 → **96** | +5 |
| Rust 侧测试目标 | 78 → **79** | +1（新文件） |

5 条判据各钉住什么：

- `a_live_show_stages_the_package_but_defers_the_swap`：在播时暂存**成功**（暂存不问门），
  激活边界返回 `Deferred{active:1}`，且宿主**效果计数恰为「一次暂存、其余全 0」**、切换记录为空、
  日志仍 `prepared`、真实会话仍在 `Running`。「推迟」被翻译成「什么都没发生」。
- `the_deferred_package_is_swapped_at_the_next_silent_boundary_without_restaging`：会停后同一 `Prepared`
  再送进激活边界 ⇒ `Ran(Committed)`、`commit == 1`、`stage == 1`、清单换成该包的 `to_manifest`。
- `an_unobservable_session_fails_the_swap_but_not_the_staging`：`hold_session` **握手**造出已就位的会话锁 ⇒
  暂存仍成功、激活返回 `Conflict`，宿主无切换、包仍 `prepared`。**分离性在失败面上同样可观测**。
- `a_failed_swap_reports_a_rollback_rather_than_a_wait`：离播时注入迁移失败 + 宿主 `with_post_upgrade_writes(3, 7)`
  ⇒ `Ran(RolledBack{code: Conflict, exported: (3, 7)})`、`rollback == 1`、`commit == 0`、清单未替换。
- `the_call_site_inherits_the_gates_definition_of_on_air`：`Draining` 仍推迟（⇒ 调用点**不是**只比 `Running`），
  `Ready` 放行（⇒ 也**不是**只比 `Stopped`）——两态各堵一种「在调用点重写判定」的写法。

真实会话由 `Created→Preparing→Ready→Running→Draining→Stopped` **逐步**走出（kernel 的合法迁移表；
`Ready → Stopped` 不合法，故停播必须经 `Draining`）。

## 三、注入验证（每条判据都要证明它会响）

编辑工具注入 + 真跑 + `cp` 备份 + `sha256` 逐字节还原。基线：

```
crates/nai-runtime/src/upgrade.rs  b77a8c91fef0e6bb388454dd3ee807ea7f70b5802afde24f53bffe8a3c0d17a6
```

四组注入，各自变红且**可归因**（红的是哪几条互不相同）：

| # | 注入 | 变红用例 | 条数 |
| --- | --- | --- | --- |
| I1 | `stage_upgrade` 开头加 `SessionGate::new(self).require_idle(op)?`（破坏暂存/激活分离） | 在播暂存、静默复用、busy 锁、Draining/Ready | **4** |
| I2 | `Deferred` 分支照样 `activate`（推迟被当成放行） | 在播暂存、静默复用、Draining/Ready | **3** |
| I3 | 门错误被吞成 `Deferred{active:0}`（未知伪装成已知的等待） | busy 锁那一条 | **1** |
| I4 | 回滚结局的 `exported` 被清零（运行时侧重写核心的 `Outcome`） | 失败激活报具名回滚那一条 | **1** |

- I1 保持绿的是「离播失败激活」——门在那种情形本来就放行，故问门不改变结论 ⇒ **判据不是无差别开火**。
- I2 与 I3 的红色集合**不相交**（I2 不动错误路径，I3 不动推迟路径）⇒ 两条守卫各有独有覆盖。
- I4 单独红「导出计数」= **回滚导出升级后写入**这一条在**运行时侧**也真被断言，而不是被核心的 `Outcome`
  顺带带过。
- 每次注入后 `cp` 还原并 `sha256sum -c` 核对（末次核对输出 `OK`，摘要与基线一致）。

**如实披露（一处断言不具判别力）**：`the_deferred_package_is_swapped_at_the_next_silent_boundary_without_restaging`
里的 `stage == 1`（包被复用而非二次暂存）**没有任何注入能让它单独变红**——激活边界**拿不到包的字节**，
所以它**按构造**无法暂存。该断言是「锁在一条由构造保证的性质上」的文档性守卫，不是注入验证过的判据；
它所在的用例非空洞（`Ran(Committed)` 与 `commit == 1` 都被 I1/I2 红过），已在测试的文档注释里写明。

## 四、registry 判定（**维持 `implemented`**）

三条各自独立的理由（逐项现测，非「同族」推断）：

1. **同类缺口的另一半仍在**：`downgrade` / `require_rollback_compatible` 在核心之外**恰 0 个调用点**
   （现测，只在 `crates/nai-upgrade-core/tests/gates.rs` 被调用）⇒「降档先排空后停用、排空不彻底即停」
   同样没在真实链路上跑过。本轮闭合的是**升级侧**的调用点。
2. **第④环真值缺口未解**：真实安装器（staging / 快照 / rename 切换）与真实分发端点仍是外部注入点；
   本轮所有世界效果都经 `RehearsalHost` 替身。
3. **ADR 0100 的 `tested` 门槛**（`A03` 判例：结构守卫通过但剩余缺口是外部真值 ⇒ 不具备可核验证据）。

**另如实登记**：`apps/engine/src` 至今对 `upgrade` **0 命中**（引擎侧升级入口未建）。本轮**不**新建该入口——
真实安装器不存在，引擎侧入口只能是演练路径，而「演练不得表示生产就绪」是本仓硬规矩。

## 五、门控（`cargo xtask check --profile full`）

**首跑即绿**（`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`，5m42s，`GATE_EXIT=0`），现测计数：

- 错误类扫描（`LNK1104` / `error[` / `out of memory` / `^error` / `FAILED`）：**0**
- `test result` 行 **97** 条 = **79** 个真实测试目标（`Running unittests|tests\`）+ **18** 个 `Doc-tests`
  （日志里 `Running ...nai-xtask.exe check --profile full` 是 **gate runner 自身回显，不计**）
- Rust 合计 **1028 passed / 0 failed**（逐目标 `0 failed`，无一条非零）
- Node 侧 **13** 个套件、**323 passed / 0 failed**（逐套件 `# fail 0`）
- 前端 6 步全过（`[frontend] 全部步骤通过`）
- **真实 rustc 告警 0**（`grep -E "^warning: " | grep -vE "error deleting lock file|generated [0-9]+ warning"` 计数为 0；
  其余「warning」行是增量编译 lock 文件的宿主噪音，W127 已具名）

**对照 W127**（**1023 passed / 0 failed**、**78** 个目标）：Rust **+5**，与本轮 nai-runtime **+5** 逐项吻合；
目标数 **78 → 79**，因本轮**新增了一个测试文件**（`crates/nai-runtime/tests/upgrade.rs`）。

⚠️ 本日志**不含** `GATE_OK` / `PREBUILD_OK` token（W127 已记：MUST NOT 把「没有该 token」判成失败）；
结论以 `GATE_EXIT=0` 与上表逐项现测为准。

## 六、提交与推送

**一次**提交，**20** 个文件（`git show --stat` 现测）：

```
38cea79 feat(w128): 装配根驱动升级事务——暂存不问门、激活先问门（R10 调用点收口）
20 files changed, 1067 insertions(+), 297 deletions(-)
```

`git push` 输出 `3e4f146..38cea79  main -> main`（SSH 443）。

**逐项核对过暂存区**（W123 的教训：显式暂存路径的盲区恰是「工具链自己改写的文件」）：本轮**没有**新增
依赖（`nai-upgrade-core` 已是 `nai-runtime` 的常规依赖）⇒ `Cargo.lock` **无改动**，`git status` 现测确认；
`target-check/`（safe-delete 阈值删不掉的缓存）与 `.commitmsg_w128.txt` 未被暂存。

## 七、CI 观测

三件齐（唯一查询参数 + `Cache-Control: no-cache` + 回读 `head_sha` == 推送 SHA），
全程 **3** 次请求、**未轮询**：

| # | 目的 | 结果 |
| --- | --- | --- |
| 1 | 触发确认 | `HTTP=200`、`total_count=1`、run **#199**（id `38913947053`）`queued`；`head_sha` 回读 == `38cea79` ✓ |
| 2 | 完成态（等 ~13.0 min 后） | 仍 `in_progress` ⇒ 触发「平台时长不同步」分支（平台差距不固定，单次不够） |
| 3 | 完成态（再等 ~5.1 min 后） | `completed / **success**`；`head_sha` 回读 == `38cea79415804a97bcfb420e2c737415b1cf6345` ✓ |

run #199 的 matrix 含 `foundation (ubuntu-24.04)` 与 `foundation (windows-latest)` 两个 job；
**run 级 `success`** ⇒ 两平台全部 job 通过（任一 job 失败时 run 级不会为 `success`）。
run 自身 `created_at → updated_at` = **14.75 min**，而 query #2 落在其完成前 **86 s** —— 正是「单轮完成态
**最少两次**查询」这条口径要挡的情形。**本机绿 ≠ CI 绿**：CI 有独立于本机 profile 的步骤
（如 `cargo xtask schema --check`）；本轮未触及 `migration::VERSION`，无升版成套项。

