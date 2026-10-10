# W27 实测报告：Pro 多会话隔离与容量准入（A11）

- 日期：2026-10-10
- 变更：`openspec/changes/w27-multi-session-isolation/`
- 决策：[ADR 0029](../adr/0029-multi-session-isolation-boundary.md)；指南：[docs/multi-session.md](../multi-session.md)
- 判据：`tests/registry.yaml` A11

## 结论

A11 的三句话——**每会话状态私有**、**共享只限三类只读资源**、**分族准入且拒绝不伤已开播会话**——在 `crates/nai-kernel/src/multiplex.rs` 内落成结构性判据，**零新增依赖**（kernel 仍只依赖 contracts），18 包与依赖方向不变，全部在 portable 下可测。

registry A11 由 `pending` 升为 `implemented`，**不标 `tested`**：`automation: integration` 的真实多路会话（同机并发写实会话、真实音频时序）未做，准入用的是宿主给定的上限表。

## 目标行为

| 判据 | 实现 | 关键设计 |
| --- | --- | --- |
| 每会话独立 epoch | `Multiplexer::advance_epoch` | 只推进被点的会话；共享计数器会让一个会话的重启作废另一个的工作 |
| 无串场 | `access(requester, owner, lane)` | 跨会话**没有**访问路径：lane 只以属主为键，requester ≠ owner 即拒 |
| 共享只限三类 | `SharedKind` 封闭枚举 | 活的 lane 状态**没有**可共享表示，是 API 形状而非检查 |
| 共享只读 | `SharedPool::write` 永远拒绝 | 没有写路径才是「只读快照」的实现 |
| 受控连接池 | `SharedPool` 持有者上限 | 上限为零在构造时即拒，不留到首次 acquire |
| 分族准入 | `AdmissionFamily` 四族 | 某族余量**不能**换另一族余量 |
| 拒绝不半提交 | `admit` 两阶段 | 全部族通过才写用量；拒绝时被拒会话为 0 且已开播会话数字不变 |
| 隔离审计 | `audit_isolation`（纯函数） | 检出无持有者的租约与重复 session id |

## 实测数字

- `cargo test -p nai-kernel --test multiplex`：**24 passed / 0 failed**
  - 命名空间与 lane 9：独立 epoch、跨会话被拒、未绑定 lane 缺席、未知会话缺席、每会话每 lane 只绑一次、六 lane 可寻址、重复 open 冲突、注册表有界、close 退休命名空间。
  - 共享 5：白名单恰三类、持有者上限、重复 acquire 幂等、无写路径、释放槽位、零上限被拒。
  - 准入 6：分族独立判定、拒绝不伤已开播会话、部分越界零提交、同族多请求求和后再判、未声明族不放行、零量被拒、release 只影响目标会话。
  - 审计 2：健康注册表干净、重复 session id 被报出。
- `cargo xtask test --suite kernel`：见下「门控」。
- `cargo xtask check --profile portable`：见下「门控」。

## 逐项验收映射（A11）

| 条款 | 覆盖 | 位置 |
| --- | --- | --- |
| 任务/音频/审批/预算/知识无串场 | lane 以属主为键，跨会话拒绝 | `access` |
| 每会话独立 epoch | 逐会话推进 | `advance_epoch` |
| 共享只限不可变模型/只读知识快照/受控连接池 | 封闭三枚举 + 无写路径 + 有界池 | `SharedKind`、`SharedPool` |
| 四族分别准入 | 分族判定，不合并总量 | `Admission::admit` |
| 资源不足拒绝新增会话 | `RESOURCE_EXHAUSTED` | `Admission::admit` |
| 不让已开播会话 OOM | 两阶段：拒绝时不写任何用量 | `Admission::admit` |

## 门控

- `cargo xtask check --profile portable`（`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=4`）：**EXIT=0**，74 个测试目标（57 `Running` + 17 `Doc-tests`；较 W26 的 73 多 1，即本轮新增的 `tests/multiplex.rs`），0 失败。
- `cargo test -p nai-kernel --test multiplex`：**24 passed / 0 failed**。
- `cargo xtask check-deps`：18 包，依赖方向不变（`nai-kernel` 的 layer 为 `kernel`，新增模块未引入任何新依赖，仍只依赖 contracts）。
- `cargo xtask check --registry`：通过（A11 证据与状态一致）。
- `openspec validate w27-multi-session-isolation --strict --no-interactive`：`Change 'w27-multi-session-isolation' is valid`。
- clippy `--all-targets -D warnings` 与 `fmt --all --check` 在 portable 内一并通过。

> 注：本轮门控先后遇到三次宿主的 LNK1104（Windows 链接器无法打开 `debug/deps/` 下**新生成**的测试 exe，逐次落在 `nai-engine` / `nai-control-api` / `nai-budget-core` 等**不同**包上，与代码无关）。按既有处置：清理被占用产物 → 单独链接（`cargo test -p <pkg> --no-run`）→ 必要时重试；预链收敛后完整门控一次通过。该现象属宿主事实，不写进 portable 检查。

## 未覆盖边界

- **真实多路会话未做**：`automation: integration` 的同机并发写实会话与真实音频时序需设备；本轮以宿主给定的上限表覆盖全部准入分支。
- **不探测硬件**：`Admission` 不知道机器真实余量，上限由调用方提供。
- **跨节点会话路由未做**：多副本与 ownership 属轻云里程碑（§27.4）。
- **多会话 UI 未做**。
