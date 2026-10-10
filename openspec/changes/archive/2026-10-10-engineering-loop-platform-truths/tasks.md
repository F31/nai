# Tasks

## 1. 规格与校验
- [x] 1.1 proposal/design/tasks/specs 完成并通过 `openspec validate engineering-loop-platform-truths --strict --no-interactive`。

## 2. harness 自锁缺陷
- [x] 2.1 检查逻辑从 `xtask/src/main.rs` 搬入 `xtask/src/lib.rs`（`pub fn execute`、`pub const HARNESS`），`main.rs` 收成为只剩参数转发与退出码的 CLI 外壳。
- [x] 2.2 profile 的 workspace 测试步改为 `cargo test --workspace --exclude <env!("CARGO_PKG_NAME")>` 后再 `cargo test -p <同名> --lib`，并写明为何排除且为何不失去 bin 的检查（clippy 仍 `--all-targets`）。
- [x] 2.3 实测 `cargo xtask check --profile portable` 不再出现 `LNK1104`：`cargo run -p nai-xtask -- check --profile portable` 以 **EXIT=0** 结束，末行输出 `portable: engineering foundation checks passed`。

## 3. nai-storage 跨平台持久化与错误类别
- [x] 3.1 抽出 `fn flush_durable(path)`（可写句柄 + `sync_all`），备份与恢复**共用同一处**，并注明 Windows `FlushFileBuffers` 需要可写句柄。
- [x] 3.2 `From<std::io::Error>` 按 `ErrorKind` 分派稳定 reason；`From<rusqlite::Error>` 按 SQLite `ErrorCode` 分派；两者都不搬 OS 本地化文本，且都仍是 `ErrorCode::Internal`。
- [x] 3.3 清理此前排查用的临时探针（io 错误回溯打印），确认无 `RUST_BACKTRACE`/调试残留。
- [x] 3.4 `nai-storage --lib` 全绿（48 passed；修复前 4 项失败），且 `consistent_backup_restores_published_knowledge_but_not_output_authority` 单独重复 8 次全过。

## 4. 平台受限演练的声明方式
- [x] 4.1 `apps/output-keeper/tests/faults.rs` 加文件级 `#![cfg(target_os = "linux")]`，与同包 `frame_drill.rs`、`usage.rs` 一致；模块文档说明门控理由与「范围 vs 诊断」的分工。
- [x] 4.2 保留 `host_stdin_pipe_refusal` 具名诊断，并把签名从 `-> !` 改为 `-> Child`（`fn(E) -> !` 无法作为 `unwrap_or_else` 的 `fn(E) -> T` 使用，编译期实测 `E0271`）。
- [x] 4.3 `apps/engine/tests/engine.rs` 的 `/proc` + `kill` 监督演练门控到 Linux，并在模块文档记录宿主管道限制。
- [x] 4.4 `0600` 断言拆为 `#[cfg(unix)]` 实测与 `#[cfg(not(unix))]` **特征化断言**（断言缺口存在，而非静默通过）。
- [x] 4.5 实测 `cargo clippy --workspace --all-targets -D warnings` 通过；`cargo test -p nai-engine`、`-p nai-output-keeper` 在 Windows 上不再有失败项。

## 5. 时钟竞态
- [x] 5.1 `document_expiry_is_rechecked_for_approval_and_preplay` 改为「宽窗口导入 + 在 `knowledge_snapshot_documents` 推进过期边界」，移除 `sleep(2100ms)`；并核对三条复核路径（候选/审批/预播）都读同一张表。
- [x] 5.2 单独运行该用例通过（1.80s），不再依赖 wall clock。

## 6. 交付
- [x] 6.1 报告 `docs/reports/2026-10-10-engineering-loop-platform-truths.md`：三个根因、平台事实与真缺陷的区分依据、宿主限制（管道 stdin / 提交限制）与复现命令。
- [x] 6.2 更新 `docs/engineering.md`（profile 语义 + 受内存限制宿主的 `CARGO_BUILD_JOBS` 提示 + 平台演练门控说明）、`docs/storage.md`（持久化确认与错误类别）、`docs/development-plan.md`、`AGENTS.md`，并新增 ADR 0024 记录本轮边界。
- [x] 6.3 全量匹配检查：fmt/clippy/check-deps/registry 通过；`check --profile portable` 在本机需同时约束构建与测试并发（`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=4`）以免撞上宿主的提交限制与吞吐上限，详见报告；`schema --check` 与 contracts suite 因本机 node 无法 spawn 子进程未执行，如实记录。
- [x] 6.4 归档变更并 `validate --archived`。
