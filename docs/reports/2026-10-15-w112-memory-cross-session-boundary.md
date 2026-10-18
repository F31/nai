# W112 报告：§42.5 跨会话边界在 memory 面的判定面闭合

- 日期：2026-10-15
- 关联：W107（写入面接线）/ W109（读取面接线）/ W110（控制面接线）/ W111（清除侧）
- ADR：0090
- 验收线：r1；contract `memory.access@1`
- **判定面闭合，零生产代码改动**（除非测试暴露缺口——本轮无缺口，只纠正了测试自身的写法）

## 一、问题（现测，不是推断）

§42.5 跨会话边界（V3.11 §42.5）在 memory 面**从未建判据**：只靠设计意图断言「授权、审批、
租约、设备身份与自动权限不跨场继承」，没有任何可测守卫把它变成事实。W107/W109/W110/W111
四次都是「闭合判定面」，本次把 §42.5 在 memory 面的判定面也闭合。

现测取证（写测试前先核实，不是推断）：

| 事实 | 取证位置 |
| --- | --- |
| `scope.session_id` 来自控制面绑定，请求同名字段被忽略 | `crates/nai-control-api/src/lib.rs:708-712` |
| 跨场读取靠 `scope_matches` 的 `session_id` / `viewer_scoped_id` 匹配 | `crates/nai-kernel/src/evaluate/memory.rs:619-628` |
| memory 路由鉴权走 `self.access`（`Access` 绑定 `session_id` + `device`） | `crates/nai-control-api/src/lib.rs` `memory_read` / `memory_forget` |
| 记录按 `expires_at_ms` 保留，无「会话结束即清」 | `crates/nai-storage` 的 `memory_records` 无 session-end 触发器 |
| §42.5 在 memory 面无显式判据 | ADR 0087/0088/0089、kernel.md:123、runtime.md:305 均记「未覆盖」 |

## 二、裁定

### 决策 1：判定面闭合，不引入新执行体

§42.5 在 memory 面的「不跨场」由**既有**两个机制保证：(a) runtime `scope_matches` 的
session / viewer_scoped_id 隔离；(b) ControlApi `self.access` 的场绑定鉴权。本轮只把这两个
机制变成**显式、可测**的判据（补测试），不新增生产代码。

### 决策 2：作用域是事实，不是请求参数

沿用 W109/W111 的纪律：`scope.session_id` 来自控制面绑定，请求伪造同名字段被忽略。补测试钉死。

### 决策 3：场次记忆跨会话保留是存储层事实

`memory_records` 按 `expires_at_ms` 保留，无 session-end 清除。补判据钉死「会话结束不清空
场次记忆」（回归保护，防止将来误加清除逻辑）。

### 决策 4：观众 `authorization_ref` 当场绑定如实披露为遗留

`read_records`（line 652）只查 `authorization_ref` 非空，不验证它属于当前 session。验证需
授权系统配合或改冻结契约（`memory.access@1` frozen），超界——本轮**不解决**，如实披露
（与 W111 同纪律）。

### 决策 5：跨设备 token 拒由同一条 `self.access` 绑定覆盖，不单列测试

任务 3.2（别的 device 的 token）与 3.1（别的 session 的 token）走**同一条** `self.access`
校验：`Access` 同时绑定 `session_id` + `device`，任一不符 ⇒ 401。3.1 的测试用「另一个控制面
签发的 token」直接触发这条绑定失败，已覆盖 device 维——**没有独立的执行分支**，故不单列测试
（否则是测同一行 `if !access.valid` 两次，假阳性）。

## 三、判据落点（先测后设计，钉死既有行为）

| 判据 | 落点 | 结果 |
| --- | --- | --- |
| 2.1 跨 session 查非观众记录 ⇒ `OutOfScope`（具名） | `crates/nai-runtime/tests/memory.rs`（`a_record_outside_the_scope_is_refused_by_name_and_not_filtered_away`，**归属既有测试**，W112 注释显式闭合） | 既有的，本轮显式归属 |
| 2.2 跨 `viewer_scoped_id` 查观众记录 ⇒ `OutOfScope`（具名） | `crates/nai-runtime/tests/memory.rs` `a_viewer_record_outside_its_viewer_scope_is_refused_by_name`（**新增**） | 绿 |
| 2.3 场次记忆跨会话保留：旧场物理保留、对新场 `OutOfScope`、不删行 | `crates/nai-runtime/tests/memory.rs` `session_scoped_memory_survives_session_end_and_stays_out_of_scope_for_others`（**新增**） | 绿 |
| 3.1 跨场 token 访问本场控制面 memory 路由 ⇒ 401 | `crates/nai-control-api/src/tests.rs` `memory_route_rejects_a_token_issued_by_another_control_surface`（**新增**） | 绿 |
| 3.3 作用域来自控制面绑定，请求伪造被忽略 | `crates/nai-control-api/src/tests.rs` `memory_scope_comes_from_the_control_surface_and_the_request_cannot_pick_it`（**归属既有测试**，W112 注释显式闭合） | 既有的，本轮显式归属 |

## 四、测试先跑红 → 纠正（如实 disclosure）

2.2 的判据**先跑红**：首版用 `write(...)` 助手（内部 `MemoryPolicy::default()`，观众记忆默认
关闭）写入观众记录，写入面以 `ViewerMemoryDisabled` 拒绝、记录**不入库存**，于是越作用域拒绝
**无可守对象**（`records` 空、`refused_for(OutOfScope)` 为 0）。纠正：写入观众记录须用
`open_policy()`（与既有观众测试 `a_viewer_record_without_an_authorization_ref...` 同款），并断言
`MemoryWriteOutcome::Written` 才继续。这恰是本项目反复强调的形状——**「跨场拒绝」必须有对象可守**，
取数层若按作用域预筛，越作用域判据会整条消失（假阴性）。

门控前自检还发现：上一轮写测试时，把既有 `the_read_back_does_not_cross_scopes`（runtime）与
`the_third_party_retention_fact_is_forwarded_verbatim`（control-api）的 `#[test]` / `#[tokio::test]`
属性**误插**到新测试之前，导致这两条既有测试**被静默取消标记**。已修复（属性归位）并**实测确认
两者重新作为测试运行通过**——这正是「判定面闭合」最该防的回归：补判据时把既有守卫弄丢了。

## 五、计数（现测，非推断）

`cargo test` 实测（门控 §七 给权威总数）：

- `crates/nai-runtime/tests/memory.rs`：W112 **新增 2**（2.2 / 2.3）；`the_read_back_does_not_cross_scopes`
  为既有测试（本轮修复属性后恢复运行，非新增）。
- `crates/nai-control-api/src/tests.rs`：W112 **新增 1**（3.1）；3.3 为既有测试归属。

## 六、如实披露（判定面闭合 ≠ 验收达标）

- 观众 `authorization_ref` 只查非空、不验证属于当前 session（决策 4）——**遗留**，未解决。
- registry `H03` / `H04` 的 `status` 仍 `pending`：本轮闭合的是「判定面 → 执行体 → 调用方」之上
  的**跨场隔离守卫**，不是验收达标。控制面控制面接线、§42.4 清除副本一侧、Pro 跨会话**运营能力**
  （§14.1）均未建。
- 生产代码**零改动**：本次只补可测判据 + 修复自身误插的属性。

## 七、门控（全量 `cargo xtask check --profile full`）

- 首次 r1 在 `crates/nai-runtime/tests/composition.rs` 两用例（`invalid_runtime_config_is_rejected_at_provision`、`prepare_without_profile_is_rejected`）报 `StorageError { code: DeadlineExceeded, reason: "operation stopped" }` —— **宿主 IO 瞬态**，与 W112 无关（`composition.rs` 不在本轮回改集）。
- 复跑 r2：失败集合**漂移**（一例恢复、一例仍红），印证 W107「失败集合漂移、不绑定用例」+「相关文件本轮 git status 为空」+「同族 DeadlineExceeded」。
- 按 W110 已验证缓解手段，复跑 r3 加 `RUST_TEST_THREADS=1`（消除测试期存储 IO 争用）⇒ `GATE_EXIT=0`。

实测计数（r3）：

- `cargo test --workspace --locked --exclude nai-xtask` **953 passed / 0 failed**（W111 为 950 ⇒ **+3** = 2 runtime + 1 control-api，逐项吻合）；其中 `composition.rs` 12/0（r1/r2 的瞬态在 r3 消失）。
- `cargo fmt --all --check` 通过；`cargo clippy --workspace --all-targets --locked -- -D warnings` 通过；`cargo test --locked --package nai-xtask --lib` 通过；`node scripts/frontend-gate.mjs`（contracts-ts typecheck/build）通过。
- `openspec validate --archived --strict` **110/0**（`--changes` 含本变更）。
- ⚠️ `cargo xtask schema --check`（protoc 3.21.12）是 CI 第 9 步，**不在本地 `--profile full`**；W112 零 schema / 契约改动 ⇒ 该步对本轮**不适用**（非「本地已验证」，如实记为 N/A，由 CI 复核；CI 双平台 success 已间接覆盖）。

## 八、提交 / 推送

- 提交 `d472818`（17 文件，+544/−5），`git -c core.editor=true commit -F .commitmsg_w112.txt`（`COMMIT_EXIT=0`，无 128 竞态）。
- 显式路径暂存（排除 `target-check` 与 `.commitmsg_*.txt`），未用 `git add -A`。
- push `5f912e6..d472818`（`origin/main`，SSH 443：`GIT_SSH_COMMAND="ssh ... -p 443"`），`PUSH_EXIT=0`。
- 完整 SHA `d472818c61c55fedb8d49b83ac0867aa69b92f0f`。

## 九、CI 观测（回填提交前对主提交的一次观测）

- run **`38684479058`**：`status=completed` / `conclusion=success`；回读 `head_sha` == `d472818c61c55fedb8d49b83ac0867aa69b92f0f`（完整 40 位一致）✓。
- 双平台：`foundation (ubuntu-24.04)` **success** + `foundation (windows-latest)` **success**。
- 查询纪律：单次 + `Cache-Control: no-cache` + 缓存破坏参数 `cb`；本轮 GitHub API 调用 < 10，未触限流。
- 回填提交（docs-only）本身 MUST 另做 1 次观测（先例 run 109 回填提交曾 failure ⇒ 不观测就是赌）。

见 [ADR 0090](../adr/0090-memory-cross-session-boundary.md)、
[kernel.md](../kernel.md)、[runtime.md](../runtime.md)、
`openspec/changes/archive/2026-10-15-w112-memory-cross-session-boundary/`。
