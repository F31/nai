## Why
上一轮 `engine-text-to-playback-ack` 的收尾报告如实记下两件事：`cargo xtask check --profile portable` **在本机无法整体通过**，以及 `cargo test --workspace` 有 **13 项既有失败**。两项都不是产品能力缺失，而是**工程校验回路本身**的问题：

1. **包装器重链自己**：profile 先跑 `cargo clippy --workspace --all-targets`，再跑 `cargo test --workspace`。后者会重新链接 `nai_xtask.exe`，而该文件**正在执行**；Windows 锁定运行中的映像，链接器报 `LNK1104`。于是这个入口在任何 Windows 宿主上都不可能通过——校验回路在验证别人的同时把自己验证掉了。
2. **13 项失败被笼统记为「平台差异」**：报告原文把 storage 那 4 项写成「`fs::rename` 覆盖语义与备份/遍历断言」，但**这不是实测结论**。逐条追下去得到三个**互相独立**的真实根因，其中一个还是**真缺陷**（见 `design.md`）：备份/恢复路径在 Windows 上**从未成功过**，只因错误文案被统一压成 `filesystem operation failed`，看起来像「平台差异」，实际是 `FlushFileBuffers` 拒绝只读句柄。
3. **平台受限的演练缺少声明方式**：同包内的 `tests/frame_drill.rs` 用文件级 `#![cfg(target_os = "linux")]` 声明「原生子进程演练仅限 Linux」，而 `tests/faults.rs` 没有；`apps/engine` 的监督演练与 `0600` 断言也没有。结果是：本机跑不通的东西既没被**门控**，也没被**命名**，只能靠一份报告口头解释。

本轮的意图不是「让测试变绿」，而是让**校验回路说真话**：能跑的必须真跑、真通过；跑不了的必须由平台事实显式门控，并在报告里可复现地说明为什么。

## What Changes
- **harness 拆成 lib + 薄 bin**（`xtask/src/lib.rs` 新增，`main.rs` 收成 CLI 外壳）：profile 的 workspace 测试改为 `cargo test --workspace --exclude <harness>` + `cargo test -p <harness> --lib`。被测包名取 `env!("CARGO_PKG_NAME")`，不写字面量——改名不会留下一个排除不存在包的空排除。clippy 仍 `--all-targets`，所以 bin 目标**没有**失去类型检查。
- **修复备份/恢复的持久化缺陷**（`crates/nai-storage/src/snapshot.rs`）：`sync_all()` 在 Windows 上需要**可写**句柄，只读句柄被 `FlushFileBuffers` 拒绝。抽出单一 `flush_durable(path)` 供备份与恢复共用，并在两处都调用。
- **错误词表按类别分派，但仍保持可移植**（`crates/nai-storage/src/lib.rs`）：`std::io::Error` 按 kind、`rusqlite::Error` 按 SQLite 结果码映射为稳定 reason。**不**携带 OS 本地化文本；目的是让报告能区分「路径缺失」「被拒访问」「库忙」「库损坏」，而不必开 debug 构建。
- **原生子进程演练按既有约定门控**：`apps/output-keeper/tests/faults.rs` 加文件级 `#![cfg(target_os = "linux")]`（与 `frame_drill.rs` 一致），保留 `host_stdin_pipe_refusal` 具名诊断以覆盖「宿主连 Linux 也拒绝管道」的残余情形；`apps/engine/tests/engine.rs` 的 `/proc` + `kill` 监督演练门控到 Linux；`0600` 断言拆为 `#[cfg(unix)]` 实测 + `#[cfg(not(unix))]` **特征化断言**（把「Windows 尚无 ACL 检查」这个缺口钉住，而不是静默跳过）。
- **消除一处时钟竞态**：`document_expiry_is_rechecked_for_approval_and_preplay` 原先用「2 秒窗口 + 固定 `sleep(2100ms)`」，在同文件 `expired_facts_block_existing_and_new_approvals` 已声明「不用 flaky sleep」的约定下是自相矛盾的。改为在**持久化边界**推进过期时间。
- 报告 `docs/reports/2026-10-10-engineering-loop-platform-truths.md`：记录三个根因、主机限制（管道 stdin、提交限制）、以及「哪些是平台事实、哪些是真缺陷」的区分依据。

## Capabilities
### New Capabilities
无。

### Modified Capabilities
- `engineering-foundation`: profile 的 workspace 测试 MUST NOT 重新链接正在执行的 harness 映像；平台受限的演练 MUST 由平台事实门控或在失败时给出具名诊断，MUST NOT 静默跳过或记为通过。
- `local-persistence`: 备份与恢复的持久化确认 MUST 在两个平台上都实际可达；错误 MUST 以可移植的**类别**区分，MUST NOT 只给一个涵盖全部情形的笼统 reason。

## Impact
- 代码：`xtask/src/{lib,main}.rs`（拆包）、`crates/nai-storage/src/{lib,snapshot}.rs`（flush + 错误分类）、`crates/nai-storage/src/tests/knowledge_publication.rs`（去时钟竞态）、`apps/output-keeper/tests/faults.rs`（平台门控）、`apps/engine/tests/engine.rs`（平台门控 + 特征化断言）。
- 规格：两项能力增量。
- 依赖：**零新增**。不引入 mock 框架、不加测试专用 crate、不新增 feature。
- 明确不做：不改 profile 的并发度（主机提交限制属环境事实，以文档记录而非写进 portable 检查）；不实现 Windows ACL 检查（只把缺口钉住）；不动 13 项失败之外的任何产品行为。
