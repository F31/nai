# 报告：engineering-loop-platform-truths（工程校验回路的平台事实与真缺陷）

- 日期：2026-10-10
- 变更：`openspec/changes/engineering-loop-platform-truths/`
- 基线：V3.6 §1.4（Slice-0 工程入口）；`docs/engineering.md` 完成证据规则
- 结论：`cargo xtask check --profile portable` **在本机首次整体通过**（EXIT=0，63 个测试目标，
  321 passed / 0 failed）。上一轮报告记的 13 项「既有平台差异」已逐条落到三个**互相独立**的根因，
  其中一项是**真缺陷**（备份与恢复在 Windows 上从未成功过）。

## 背景：上一轮留下的两句话

上一轮收尾报告如实写了两件事：`cargo xtask check --profile portable` **本机无法整体通过**，
且 `cargo test --workspace` 有 **13 项失败**（keeper `faults.rs` 6、engine `engine.rs` 3、
storage `--lib` 4）。它同时给出了一份解释：这些是「Windows 平台差异」。本轮的工作就是**去验证这份解释**。

验证结果是：解释**有一半是错的**。三条线索里，一条是真缺陷，两条是平台事实但**缺少声明方式**，
还有一条把「没有验证」写成了「通过」。

## 根因一（真缺陷）：`FlushFileBuffers` 拒绝只读句柄

storage 的 4 项失败被上一轮记为「`fs::rename` 覆盖语义与备份/遍历断言」。实测不是。

用临时插桩把被丢掉的 `io::Error` 打出来后，失败点是 `snapshot.rs` 的
`File::open(&path)?.sync_all()?`，错误是 `Os { code: 5, PermissionDenied }`：

- Windows 的 `FlushFileBuffers` **要求可写句柄**；只读句柄返回 `PermissionDenied (os error 5)`。
- Linux 的 `fsync` 对只读 fd 合法，所以这条路径在 Linux 上一直是对的。

**后果比「4 个测试失败」严重**：`backup` 与 `restore` **各自**都在这行调用，因此在 Windows 上
**备份从来没成功过，恢复也从来没成功过**。4 个失败里 2 个是备份/恢复用例，
另外 2 个只是碰巧不经过这条路径。恢复路径的缺陷还一直被掩盖着——因为它在更早的 `sync_all` 就炸了。

**修复**：抽出单一 `fn flush_durable(path)`（`OpenOptions::new().read(true).write(true)` 再 `sync_all`）
供备份与恢复**共用**。不做 `#[cfg(windows)]` 分叉：可写句柄在 Linux 上同样正确（不写任何字节），
一份实现胜过两份。

修复后该用例**首次**在这台机器上通过，并单独重复 8 次全过（8/8）。

## 根因二（平台事实，但缺声明）：原生子进程演练没有门控

`apps/output-keeper/tests/faults.rs` 的 6 项失败，实测错误是 `Os { code: 231 }`（`ERROR_PIPE_BUSY`）。
逐层排除后得到一个**宿主级**事实：

- 该宿主对**任何** stdin 为管道的子进程都拒绝 `spawn`——新管道或继承管道都一样，
  用一个独立程序 spawn `cmd /c echo` 即可复现，且沙箱开关与否都复现。
- **但**：`CreatePipe` 直建可用，piped stdout / stderr 可用。拒绝只针对 piped stdin。
- 同一受限能力也卡住别处：`node` 的 `spawnSync` 返回 `EBUSY`，直接导致
  `cargo xtask schema --check` 与 `test --suite contracts` 在本机不可执行。

而同包内的 `tests/frame_drill.rs` 早就用**文件级** `#![cfg(target_os = "linux")]` 声明过
「原生子进程演练仅限 Linux，是平台事实而非死代码」。`faults.rs` 却没跟上。

**修复**：`faults.rs` 采用**同一套**门控；`apps/engine/tests/engine.rs` 的 `/proc` + `kill`
监督演练同样门控到 Linux。同时**保留** `host_stdin_pipe_refusal` 具名诊断——门控声明**范围**，
诊断负责在范围内仍失败时给出可读原因（而不是甩一个 `ERROR_PIPE_BUSY` 让人以为是 keeper 的错）。

工程细节：该诊断函数原先签名是 `-> !`，作为 `unwrap_or_else` 的 `fn(E) -> T` 参数会编译失败
（`E0271`：`!` 不会自动强制成具体返回类型）。改为返回 `Child` 并以 `panic!` 收尾。

**为什么不做运行期探测后提前 return**：libtest 没有 skipped 结局，提前 return 会显示 **`ok`**——
把「没跑」报告成「通过」，与 `docs/engineering.md`「不使用 mock、空成功任务为真实认证标 Passed」直接冲突。

## 根因三（没验证，却被写成通过）

`engine.rs` 的 `incomplete_or_unsafe_invocation_is_rejected_without_listening` 断言了
「世界可读的令牌文件必须被拒绝（`0600`）」。该规则是 **Unix mode 检查**；Windows 分支下引擎
因**别的**原因被拒绝，原断言于是**碰巧通过**——它记录的不是验证结果。

**修复**：`#[cfg(unix)]` 保留实测；`#[cfg(not(unix))]` 换成**特征化断言**——直接断言
「令牌仍可被读取」，把「Windows 尚无 ACL 检查」这个缺口钉在测试里。将来谁给 Windows 加上 ACL 检查，
这条会立刻失败并提醒换成拒绝断言，而不是悄悄多出一个泄漏。

## 附：一处自相矛盾的时钟竞态

`document_expiry_is_rechecked_for_approval_and_preplay` 用「2 秒窗口 + 固定 `sleep(2100ms)`」，
而**同一个文件**里的 `expired_facts_block_existing_and_new_approvals` 已写明
「Simulate clock passage at the persisted boundary, without flaky sleeps」。本轮实测：整机负载高时，
建立会话状态本身就要超过 2 秒，文档在 `record_candidate` 前就过期，测试失败而规则没错——
即这条用例本身是 flaky 的。

改为：导入时给 1 小时宽窗口让会话建立**确定性地**完成，随后把
`knowledge_snapshot_documents.valid_until_ms` 推到过去来模拟时间流逝。之所以仍测同一件事：
候选复核（`record_candidate`）、审批复核（`approval_valid` → `candidate_documents_valid`）
与预播复核（`retrieve_local`）三条路径**都只**从该表读过期边界。

## 附：让「平台差异」这种误判不再成立

根因一之所以能被误判成「`fs::rename` 语义差异」，直接原因是错误文案被压平了：
`From<std::io::Error>` 把一切变成 `"filesystem operation failed"`，
`From<rusqlite::Error>` 把一切变成 `"sqlite operation failed"`。报告里能读到的只有一个笼统原因。

**修复**：两个转换都改为按**类别**分派——`std::io::Error` 依 `ErrorKind`，
`rusqlite::Error` 依 SQLite `ErrorCode`（`sqlite busy` / `sqlite database corrupt` /
`sqlite database unopenable` / `sqlite storage full` …）。仍然统一为 `ErrorCode::Internal`，
**不**携带 OS 本地化文本（这台机器返回中文错误，写进稳定词表就没法跨机比对）；
SQLite 的结果码是 SQLite 自己的，语义跨平台一致。

这不是装饰：本轮在**主机提交限制**下复现过一次 `Store::restore` 失败，正是这条映射让它可判读
（详见下方「宿主资源事实」）。

## 根因零：harness 把自己验证掉了

上一轮已记录：profile 先跑 `cargo clippy --workspace --all-targets`，再跑 `cargo test --workspace`，
后者会重新链接**正在执行**的 `nai_xtask.exe`；Windows 锁定运行中的映像，报 `LNK1104`。
Linux 允许覆盖运行中的可执行文件，所以这是 Windows 宿主特有的**入口缺陷**——校验回路在自己身上失败。

**修复**：检查逻辑搬进 `xtask/src/lib.rs`，`main.rs` 收成 CLI 外壳。测试步改为
`cargo test --workspace --exclude <包名>` + `cargo test -p <包名> --lib`。

- 包名取 `env!("CARGO_PKG_NAME")`，不写字面量：写成 `"nai-xtask"` 的话，包一旦改名，
  `--exclude` 会静默排除一个不存在的包（cargo 容忍），排除失效而无人察觉。
- **没有减少检查**：clippy 仍 `--all-targets`，bin 目标照旧被 `-D warnings` 类型检查；
  回避的只是**链接**。

## 验证（本轮实际执行）

| 检查 | 结果 |
|---|---|
| `cargo fmt --all --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 |
| `cargo xtask check-deps` | 17 包依赖边界通过 |
| `cargo xtask check --registry` | 通过 |
| `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4`） | **EXIT=0**（63 个测试目标，**321 passed / 0 failed**），末行 `portable: engineering foundation checks passed` |
| `cargo xtask check --profile portable`（仅 `CARGO_BUILD_JOBS=2`） | EXIT=0（同上）；热缓存下偶发一次宿主吞吐抖动，见下方「宿主资源事实」 |
| `cargo test -p nai-storage --lib --locked --offline` | **48 passed / 0 failed**（修复前 4 项失败）；三次独立运行一致 |
| 备份/恢复用例单独重复 | **8/8 通过** |
| `cargo test -p nai-output-keeper --locked --offline` | 6 passed / 0 failed（`faults.rs` 在 Windows 上按平台范围不编译） |
| `cargo test -p nai-engine --locked --offline` | 23 passed / 0 failed（`engine.rs` 2 项实测通过，3 项 Linux-only；`pipeline.rs` 13 项） |
| `cargo xtask schema --check` / `test --suite contracts` | **未执行**：本机 `node` 无法 spawn 子进程（`spawnSync ... EBUSY`），非本轮改动所致 |
| `pnpm typecheck` / 前端 smoke | **未执行**：本机 PATH 无 pnpm |

对比基线：`cargo test --workspace` 由 **272 passed / 13 failed（3 个目标失败）** 变为
**321 passed / 0 failed（0 个目标失败）**。通过数上升是因为此前失败的目标整体不产生结果行，
其未执行的用例现在才真正跑完。

## 宿主资源事实（如实记录，未写进代码）

修完三个根因后，profile 仍会在本机偶发失败。逐条追下去是**宿主吞吐/内存**事实，不是代码缺陷，
但每一条都用实测钉住了性质：

1. **内存/提交限制**。不加并发约束时，全量并行链接 30 余个带 debuginfo 的测试二进制会触到系统提交限制：
   `LNK1102: 内存不足`、`Allocation failed`、`os error 1455`（页面文件太小）。
   加 `CARGO_BUILD_JOBS=2` 后整体通过：**EXIT=0，63 个测试目标，321 passed / 0 failed**。
2. **宿主吞吐**。同一配置在热缓存下仍出现过一次 storage 用例抖动（`DeadlineExceeded`，
   另有 `Store::restore` 一次 `sqlite operation failed`）。隔离计时给出解释：
   - 打开一个 Store 并完成迁移/权限装配：**固定约 5 秒**（`full_table_catalog_...` 5.02s、
     `expired_facts_...` 4.97s，均无备份）。
   - 再加 `backup`+`restore`：**约再加 7 秒**（`online_backup_restores_wal_...` 12.76s、
     `corrupted_database_...` 11.71s）。
   - 而 `fsync` 本身**不慢**（独立测量：约 2 ms/次）——所以慢的不是同步原语，而是整体工作负载。
   - 单次存储操作有 **30 秒硬上限**（`Operation::new` 拒绝更长超时，属**产品不变量**，不可放宽）。
   16 秒的用例空闲时已用掉一半以上预算；workspace 全局并行时被挤压越过上限，于是报
   `DeadlineExceeded "operation stopped"`——**存储层如实停止，规则没错**。
   加 `RUST_TEST_THREADS=4`（libtest 原生读取）后存储套件**连续两次全绿**（187s / 201s）。
3. 因此本机可用的完整命令是
   `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4 cargo xtask check --profile portable`，
   在此配置下**EXIT=0 且无任何失败**。

**未**把任何一个并发上限写进 profile：那会把一台机器的资源假设烘进 portable 检查。
两个都是 cargo/libtest 的原生开关，已记入 `docs/engineering.md`。

## 诚实边界

- 本轮只动**工程校验回路**，未改任何产品行为；两个 delta 规格只约束「回路如何说真话」。
- `faults.rs` 的 6 项演练在 Windows 上**不编译**是**范围声明**，不是「已通过」；
  它们的能力由 Linux 上的执行覆盖，本机未验证。
- Windows 的令牌 ACL 检查**仍未实现**，本轮只把缺口钉成特征化断言。
- `schema --check`、contracts suite、前端 typecheck/smoke 在本机**未执行**，原因在表里逐条写明。
- 本机通过 ≠ 设备 / GPU / 官方 SDK / 干净机安装验收；profile 末行原文即声明了这一点。

## 验收状态

`tests/registry.yaml`：本轮不改变任何 V3.6 验收项状态。A04（依赖方向强制）等工程项本就 `tested`，
其证据由本轮报告补强；不因「测试变绿」升级任何产品项。

## 归档

`openspec/changes/archive/2026-10-10-engineering-loop-platform-truths/`；
主规格 `engineering-foundation`（+2）、`local-persistence`（+2）增量生效。ADR 0024。
