## Context

本轮只动「工程校验回路」本身，不动任何产品行为。约束：**不许用 mock 换取绿色**、**不许靠放宽断言换取绿色**、**不许把主机限制写成代码常量**（用户本轮明确要求「保持架构的合理性，不要硬编码和绑定太多依赖」）。

### 实测环境事实（本机，可复现）

| 现象 | 观测 | 性质 |
|---|---|---|
| 子进程 stdin 为管道时 spawn 失败 | `Os { code: 231 }`（`ERROR_PIPE_BUSY`），对**任何**子进程成立；独立程序 spawn `cmd /c echo` 复现；`CreatePipe` 直建可用、piped stdout/stderr 可用 | **宿主限制**（非仓库代码） |
| 全量并行链接时内存耗尽 | `LNK1102: 内存不足`、`memory allocation of 2097152 bytes failed`、`os error 1455`（提交限制） | **宿主资源**（受限并发下全绿） |
| 文件系统吞吐很慢（`fsync` 本身不慢，约 2 ms/次） | 隔离实测：打开 Store 并完成迁移/权限装配固定约 **5 s**；`backup`+`restore` 再加约 **7 s** | **宿主吞吐**：单次存储操作有 30 s 产品上限（`Operation::new` 拒绝更长超时），workspace 全局并行时会被挤压逼近 |
| `Command::spawn` 之外：node 无法 spawn 子进程、PATH 无 pnpm | `EBUSY` / 命令不存在 | 本机工具链限制 |

## Decisions

### 1. harness 拆成 lib + 薄 bin，而不是让 profile 不测自己

`cargo test --workspace` 会命名 harness 的 bin 目标并**重新链接正在执行的映像**；Windows 拒绝覆盖运行中的可执行文件（`LNK1104`）。Linux 允许，所以这是 Windows 宿主特有的入口缺陷。

- **选择**：把检查逻辑搬进 `xtask/src/lib.rs`（`pub fn execute`），`main.rs` 只留 CLI 外壳。profile 的测试步改为 `cargo test --workspace --exclude <harness>` + `cargo test -p <harness> --lib`。
- **包名取 `env!("CARGO_PKG_NAME")`**：写成 `"nai-xtask"` 字面量的话，包一旦改名，`--exclude` 就静默排除一个不存在的包（cargo 会容忍）——排除失效而无人察觉。取包名常量则不可能失配。
- **为什么不用「profile 不测 harness」**：那会把 harness 自身的测试（`deps`/`registry`/`schema` 三个模块的 10 个用例）从校验回路里删掉。
- **为什么不失去 bin 的检查**：clippy 仍是 `--workspace --all-targets`，bin 目标照旧被类型检查（`-D warnings`）。只有**链接**被回避，检查没有。

### 2. `flush_durable`：一个真缺陷，不是平台差异

备份与恢复都在「写完 → 立刻证明字节落盘」处调用 `File::open(path)?.sync_all()`。Windows 的 `FlushFileBuffers` **要求可写句柄**，只读句柄得到 `PermissionDenied (os error 5)`；Linux 上 `fsync` 对只读 fd 是合法的。于是**备份与恢复在这台开发机上从未成功过**——上一轮报告把它记为「`fs::rename` 覆盖语义」，那是把统一后的错误文案当成了结论。

- **选择**：抽出 `fn flush_durable(path: &Path)`（`OpenOptions::new().read(true).write(true).open(path)?.sync_all()`），备份与恢复**共用同一处**。两处都需要同一条平台知识，复制它就会分叉。
- **为什么不是 `#[cfg(windows)]` 分支**：用可写句柄打开再 flush 在 Linux 上同样正确（不写任何字节），所以不需要平台分叉；一份实现胜过两份。
- **证据**：修复前 storage 4 项失败（`filesystem access denied`）；修复后 `nai-storage --lib` 48 passed，其中 `consistent_backup_restores_published_knowledge_but_not_output_authority` 与 `online_backup_restores_wal_content_files_and_revokes_old_authority` 都是**首次**在这台机器上通过。

### 3. 错误词表：按类别分派，但不搬运 OS 文本

`From<std::io::Error>` 原先把一切压成 `"filesystem operation failed"`；`From<rusqlite::Error>` 把一切压成 `"sqlite operation failed"`。这正是上面那个真缺陷被误判成「平台差异」的原因：报告读不出「被拒访问」和「路径缺失」的区别。

- **选择**：`io::Error` 按 `kind()`、`rusqlite::Error` 按 SQLite `ErrorCode` 映射到稳定 reason（如 `filesystem access denied` / `sqlite busy` / `sqlite database corrupt`）。
- **为什么不含 OS 文本**：OS 消息是**本地化平台文本**（这台机器返回中文），写进稳定词表会让日志不可跨机比对。
- **为什么 SQLite 码是可移植的**：`ErrorCode` 是 SQLite 自己的结果码，不是宿主 API；跨平台语义一致。
- **不改变语义**：都仍是 `ErrorCode::Internal`；只让 reason 可区分。没有任何测试断言旧文案（已核实），因此不构成规格变更。

### 4. 平台受限的演练：门控，而不是「运行期跳过」

仓库既有约定是文件级 `#![cfg(target_os = "linux")]`（`apps/output-keeper/tests/frame_drill.rs`、`apps/render-worker/tests/usage.rs`），其理由已被写进注释：「native subprocess drills only；media drill is Linux-only，是平台事实而非死代码」。

- **选择**：`faults.rs` 与 `frame_drill.rs` 采用**同一种**门控；`apps/engine/tests/engine.rs` 的 `/proc` + `kill` 演练同样门控到 Linux。
- **为什么不用运行期探测 + 提前 return**：libtest 没有「skipped」结局，提前 return 会显示 **`ok`**——把「没跑」报告成「通过」，正是本仓库明令禁止的（`docs/engineering.md`：「不使用 mock、空成功任务为真实认证标 Passed」）。
- **为什么不用 `#[ignore]`**：它是静态属性，无法表达「本宿主不支持管道 stdin」这个**运行期**事实；一旦加上就会在所有机器上永久跳过。
- **保留 `host_stdin_pipe_refusal`**：门控假定 Linux 支持管道 stdin，而本轮的实测恰恰是「宿主可能不支持」。两者不矛盾：门控声明**范围**，具名诊断负责在范围内仍失败时给出可读原因（并指向报告），而不是甩一个 `ERROR_PIPE_BUSY`。

### 5. `0600` 断言：拆成实测 + 特征化，而不是静默通过

`incomplete_or_unsafe_invocation_is_rejected_without_listening` 原先把「世界可读的令牌文件必须被拒绝」当成跨平台断言。该规则是 **Unix mode 检查**，Windows 分支下引擎因**别的**原因被拒绝——原断言在 Windows 上是「碰巧通过」，不是验证。

- **选择**：`#[cfg(unix)]` 保留原实测；`#[cfg(not(unix))]` 换成**特征化断言**——直接断言「令牌仍可被读取」，把「Windows 尚无 ACL 检查」这个缺口钉在测试里。
- **为什么**：缺口可以暂时存在，但不能**无声**变化。将来谁给 Windows 加上 ACL 检查，这个断言会立刻失败并提醒把它换成拒绝断言，而不是悄悄多出一个泄漏。

### 6. 时钟竞态：在持久化边界推进，而不是延长 sleep

`document_expiry_is_rechecked_for_approval_and_preplay` 用「2 秒窗口 + 固定 `sleep(2100ms)`」。本轮实测：在整机负载高时，**建立会话状态本身**就超过 2 秒，文档在 `record_candidate` 前就过期了——测试失败，而规则没错。同文件的 `expired_facts_block_existing_and_new_approvals` 已写明「Simulate clock passage at the persisted boundary, without flaky sleeps」。

- **选择**：导入时给一个宽窗口（1 小时）让会话建立**确定性地**完成；随后把 `knowledge_snapshot_documents.valid_until_ms` 推到过去，模拟时间流逝。
- **为什么这仍然测的是同一件事**：候选复核（`record_candidate`）、审批复核（`approval_valid` → `candidate_documents_valid`）与预播复核（`retrieve_local`）三条路径**都**只从 `knowledge_snapshot_documents` 读过期边界。推进该表就是推进那三条路径共同的时间来源。
- **代价（如实）**：不再覆盖「真实时钟自行走过边界」这一路径；该路径由同一文件内的 facts 用例以同一手法覆盖，仓库约定如此。

## Rejected alternatives

- **给 profile 加 `-j N` 上限**：能把本机提交限制糊过去，但会把**一台机器的资源假设**烘进 portable 检查。改为在 `docs/engineering.md` 记录：受内存限制的宿主可用 `CARGO_BUILD_JOBS` 约束并发（cargo 原生支持，无需新开关）。
- **把 13 项失败改成 `#[ignore]` 或删除**：等同于把「没验证」写成「不需要验证」，与 `docs/engineering.md` 的完成证据规则冲突。
- **为 keeper 演练写一个纯 Rust 替身进程**：能绕开宿主管道限制，但那就把「两个真实进程共享一条管道」这个**演练对象**换成了自证；且引入测试专用进程编排代码，属过度绑定。
- **把 SQLite 错误码也塞进 `position`**：`position` 已有明确语义（完整性校验的首个分歧序号），复用会污染它。

## Verification commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p nai-storage --lib --locked --offline
cargo test -p nai-output-keeper --locked --offline
cargo test -p nai-engine --locked --offline
cargo xtask check-deps
cargo xtask check --registry
cargo xtask check --profile portable            # 受内存限制的宿主可加 CARGO_BUILD_JOBS
```
