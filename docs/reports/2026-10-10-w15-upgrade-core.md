# W15 实测报告：升级器基础（校验、事务、门控与成本）

- 日期：2026-10-10
- 变更：`openspec/changes/w15-upgrade-core/`（归档后为 `openspec/changes/archive/2026-10-10-w15-upgrade-core/`）
- 判据：`tests/registry.yaml` R05、R09、R10、O05，及 S08 的撤销列表缺口
- 结论：**四条判据主体落地并可执行**；R05/R09/R10/O05 可 `pending → implemented`（**不标 tested**），S08 补撤销列表证据后保持 `implemented`

## 交付物

新增 crate `crates/nai-upgrade-core`（shared 层，零新增外部依赖）：

| 文件 | 内容 |
|---|---|
| `manifest.rs` | 清单解析（JSON Schema 先、提取后）；路径规范；文件声明唯一；版本可 `semver` 解析 |
| `verify.rs` | `verify_package`：schema → 声明闭包 → 内容精确匹配 → 逐文件 SHA256 → 签名 → 撤销；入口/分块/出口复检 deadline 与 cancellation |
| `signature.rs` | `SignatureVerifier` 接缝 + `DeterministicVerifier` 演练替身（**非真实签名链**） |
| `revocation.rs` | `RevocationList` 带期限三态判定；`check_install` / `check_startup` 双入口 |
| `preflight.rs` | 平台/内核范围/依赖闭包/双份磁盘/凭据/运行时六项；三态资源，未知即拒绝 |
| `plan.rs` | `UpgradeHost` / `UpgradeJournal` 双接缝 + `UpgradeTransaction`（`prepare` / `activate` / `recover`） |
| `rehearsal.rs` | `RehearsalHost` / `RehearsalJournal` / `RehearsalSessions` / `RehearsalDrain`（内存替身） |
| `gates.rs` | `ActiveSessionGate` + `SessionGate`；`SessionDrain` + `downgrade`；`require_rollback_compatible` |
| `cost.rs` | `CostEstimate` / `Component` / `Total`：未知 ≠ 0，部分和不当作总额 |

配套：`schemas/config/upgrade-package.schema.json`（新）、`crates/nai-storage/src/upgrade.rs`（journal 持久化，
复用既有 `upgrade_journal` 表）、`xtask` 新增 `test --suite upgrade`。

## 实测数字

- `cargo test -p nai-upgrade-core`：**3 lib + 7 gates + 12 plan + 8 upgrade = 30 passed / 0 failed**
- `cargo test -p nai-storage --lib`：50 passed / 0 failed（含新 journal 模块）
- `cargo xtask check-deps`：18 个 workspace 包通过
- `cargo clippy -p nai-upgrade-core --all-targets -- -D warnings`：干净
- `cargo fmt --all --check`：干净
- `cargo xtask check --profile portable`：**EXIT=0，70 个测试目标（53 二进制 + 17 doc-test），368 passed / 0 failed**（基线 361，本轮 +7）

## 逐项验收映射

### R05 升级原子与回滚（`automation: fault`）

| 规格要求 | 落点 | 证据 |
|---|---|---|
| 九步固定顺序 | `plan.rs::prepare` / `activate` | `activation_commits_and_swaps_the_manifest_once` |
| 任一步失败无半成品 | `plan.rs::converge` | `a_failure_after_the_backup_converges_to_rolled_back`（Migrate/SelfCheck/Commit 三点） |
| 换清单原子 | `UpgradeHost::commit` 契约 | 上述测试断言 `swaps().len() == 1` |
| journal 五状态 + 失败终态 | `nai-storage/src/upgrade.rs` | `the_transaction_journals_into_the_storage_table` |
| 崩溃恢复幂等 | `plan.rs::recover` | `recovery_from_migrated_restores_and_is_idempotent`、`recovery_after_a_landed_swap_confirms_commit_without_repeating_effects`、`recovery_of_an_absent_or_committed_transaction_has_no_effects` |
| 回滚保留升级后写入 | `ExportedWrites` | `a_failure_after_the_backup_converges_to_rolled_back`（facts/audit 计数） |

**未标 tested 的原因**：`automation: fault` 要求真实故障注入，本轮为演练宿主（内存替身）注入，
非真实 Windows 安装器/断电。真实设备与安装器接线未做。

### R09 升级预检与下载失败

| 规格要求 | 落点 | 证据 |
|---|---|---|
| 依赖闭包 / 平台 / 运行时矩阵 | `preflight.rs` | `preflight_refuses_on_each_axis` |
| 双份磁盘空间 | `install_bytes + migrated_bytes` | 同上 |
| 未知即拒绝且与不足可区分 | `ResourceState` 三态 | `an_insufficient_or_unknown_preflight_leaves_no_trace`（`RESOURCE_EXHAUSTED` vs `CAPABILITY_NOT_READY`） |
| 预检失败不触碰当前版本清单 | 只产出报告 | 同上（`current_manifest` 不变、零效果计数） |

**未覆盖**：真实下载器（断流/校验失败/重试）未实现；本轮只覆盖「下载内容到达后」的校验与预检。

### R10 开播与回滚

| 规格要求 | 落点 | 证据 |
|---|---|---|
| 开播中只暂存不激活 | `SessionGate` 与 `activate` 分离 | `a_live_session_defers_activation_but_not_staging` |
| 降档先排空后停用、数据保留 | `gates.rs::downgrade` | `a_downgrade_drains_then_retires_and_keeps_records` |
| 排空不彻底即停止 | `downgrade` 的 `remaining > 0` 分支 | `a_downgrade_stops_when_sessions_outlive_the_drain` |
| 不兼容阻止静默回滚 | `require_rollback_compatible` | `an_incompatible_rollback_target_is_refused` |

**未覆盖**：真实会话注册表（`ActiveSessionGate` 的真实实现）属装配根，未接线。

### O05 升级成本显示

| 规格要求 | 落点 | 证据 |
|---|---|---|
| 逐项列示四类成本 | `CostEstimate::components` | `cost_marks_unknown_components_instead_of_zeroing_them` |
| 未知项标注未知、不冒充 0 | `Component::Unknown(reason)` | 同上 |
| 已知项求和 | `Total::Complete` / `Partial{floor}` | `a_fully_known_cost_totals_completely` |

### S08 插件与供应链（撤销列表缺口）

| 规格要求 | 落点 | 证据 |
|---|---|---|
| 安装与启动都查撤销 | `check_install` / `check_startup` | `signature_and_revocation_gate_staging` 及 revocation 单测 |
| 过期缓存不作「无撤销」 | 三态判定 | revocation 单测（过期即拒） |

**S08 保持 `implemented`**：SBOM 与真实签名链仍未建。

## 关键设计决定（详见 ADR 0026）

1. **shared 层业务内核**：三条候选（塞 storage / 放 runtime / 新增 shared crate）中前两条因职责错位与
   portable 不可测被否决，采用第三条。
2. **journal 写前记录**：崩溃只留「至多领先一步」的记录，恢复永远向后收敛。
3. **三态资源，未知即拒绝**：`Unknown` 与 `Insufficient` 同等拒绝，错误码区分。
4. **撤销列表带期限**：否决「做成集合」——过期即拒绝，否则撤销在最需要时静默失效。
5. **门控与激活拆两次调用**：会话事实归调用方，升级器不持有第二份真相。
6. **成本未知 ≠ 0**：部分和不当作总额。

## 未覆盖边界（诚实清单）

- **无真实签名链**：`DeterministicVerifier` 为 `sha2` 演练替身，四处（`lib.rs` 文档 / ADR 0026 / 本报告 /
  registry notes）写明「非真实签名链」。
- **无真实安装器 / 下载器**：`RehearsalHost` 不持有字节、不落盘。
- **无 Windows 文件锁 update-helper**。
- **`semver` 只做交集判断**，多包依赖冲突未覆盖。
- **未接入装配根**：`nai-upgrade-core` 尚无 app 依赖。
- **未改 `nai-storage` 的只向前迁移语义**；未升级 A08（档位升级不重装内核，R2/W25）。

## 环境边界

本机为 Windows MSVC 宿主（Rust 1.96.0）。`CARGO_BUILD_JOBS` / `RUST_TEST_THREADS` / `CARGO_INCREMENTAL=0`
是 cargo/libtest 原生开关，用于规避宿主提交与链接争用限制，**不写进** portable 检查。
本轮不需要网络（所有 mock 与替身均在进程内）。
