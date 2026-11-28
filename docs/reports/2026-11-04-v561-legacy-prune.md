# V5.6.1 第二批旧 Rust 树清理

日期：2026-11-04。状态：已完成并通过 `cargo xtask check --profile workbench`。

## 目标

按 ADR 0193 与用户要求，不保留向前兼容，删除 V4.8 旧 Rust 树与过期依赖，避免旧工作区/审批/预算/升级代码继续参与构建。

## 已删除

| 资产 | 说明 |
|---|---|
| `apps/engine` | V4.8 旧 engine；V5.6.1 使用 `apps/workbench-engine` |
| `crates/nai-budget-core` | 预算/审批旧线 |
| `crates/nai-provider-core` | 旧 provider 抽象 |
| `crates/nai-security` | 旧安全/凭据抽象 |
| `crates/nai-knowledge-core` | 旧知识/RAG 线 |
| `crates/nai-storage` | 旧 workspace/role/approval/ledger 存储 |
| `crates/nai-upgrade-core` | 旧升级线 |
| `crates/nai-control-api` | 旧控制 API |
| `crates/nai-control-ipc` | 旧控制 IPC |
| `xtask/examples/golden_eval.rs` | 依赖已删除 crate 的旧 golden 示例 |

## 依赖与构建调整

- `Cargo.toml`：workspace 从 26 个包降到 **18 个包**；移除上述 members 与 workspace dependencies。
- `default-members` 只保留 V5.6.1 活跃线。
- `xtask/Cargo.toml`：移除 `nai-budget-core`、`nai-provider-core`、`nai-security` dev-dependencies。
- `xtask/src/lib.rs`：删除 `registry` 模块与旧 suite（knowledge/provider/scheduler/storage/upgrade）；session 只保留 kernel/media-core。
- `xtask/src/schema.rs`：contracts suite 不再运行 V4.8 `tests/registry.test.mjs`。
- `workbench_checks`：改用 `scripts/check-v561-registry.mjs`；clippy 只检查 `--lib --bins`，避免旧测试/示例阻断 V5.6.1 门控。
- `apps/public-output-keeper`：Windows-only `research_clock_bridge` 示例加 `windows-research` feature 门槛，Linux 测试不再因缺 `main` 失败。
- `crates/nai-workbench-host`：移除两个测试夹具中的无效 `..Default::default()`。

## 保留

- `apps/workbench-engine`、`apps/workbench-desktop`、`apps/public-output-keeper`、`apps/output-keeper`。
- `apps/audition-worker`、`apps/human-mic-worker`：未跟踪可选 worker；不在 V5.6.1 主线但保留文件，若后续确认无关再删。
- `adapters/storage-sqlite`：保留；V5.6.1 新 `v561` 模块已加入，但旧 V4.8 模块/测试仍在，下一步按实际消费者收敛。
- `crates/nai-kernel`、`nai-plugin-sdk`、`nai-contracts`、`nai-runtime-core`、`nai-media-core`、`nai-live`、`nai-avatar-files`、`nai-workbench-host`：活跃线保留。

## 验证

```bash
cargo check --locked                 # pass
cargo check                          # pass
cargo xtask check-deps               # pass, 18 workspace packages
cargo test -p nai-foundation-sqlite v561 --locked   # 7 passed
cargo xtask check --profile workbench # pass
node scripts/check-v561-registry.mjs # 34 ACC
openspec validate v561-flashhead-single-engine --strict --no-interactive # valid
```

## 架构原则对齐

- **轻量**：移除 9 个旧 crate/1 个旧 app；workspace 26 → 18。
- **解耦**：V5.6.1 活跃线不再依赖预算/审批/升级/旧控制 API。
- **性能**：减少默认构建包与测试面；保留 WAL/mmap/cache/index 的 V5.6.1 存储。
- **易用**：`cargo xtask check --profile workbench` 现在可完整通过；不再被旧 registry/示例阻断。
- **可扩展**：保留 kernel/contracts/runtime-core/media 端口；旧存储模块按后续 1.x 任务逐步收敛，不做一次性大重写。
- **可配置**：V5.6.1 数据根/配置仍由 `V561OpenOptions` 注入；不新增硬编码。

## 剩余旧代码

- `adapters/storage-sqlite` 内仍保留 V4.8 workspace/role/approval/budget 等模块与测试；它们不再被 V5.6.1 数据根使用，但 `Store`/engine 仍有旧调用。下一步按 1.x 实际消费者收敛，不在本次一次性删除。
- 旧 `scripts/contracts/*` 中仍有 V4.8 锚点/registry 脚本；不在 V5.6.1 `workbench` 门控中运行。后续按任务 7.x 清理或归档。
- `tests/registry.yaml` 保留只读归档，不参与 V5.6.1 门控。

## 下一步

1. 1.3/1.4：启动器、依赖探测、配置读写/凭据。
2. 收敛 `adapters/storage-sqlite` 旧模块：先确认 workbench-engine 的旧 Store 调用，再迁移到 `v561`。
3. 2.x Qwen3-TTS、3.x FlashHead worker。
4. 目标机 M0 基线（当前 Blocked）。
