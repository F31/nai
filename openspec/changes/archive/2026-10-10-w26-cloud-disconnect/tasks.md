## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w26-cloud-disconnect`（proposal/design/tasks + `local-autonomy` 增量），`validate --strict` 通过。
- [x] 1.2 确认复用基座：`nai-upgrade-core`（shared）的 `ErrorCode` 与 W25 `Violation{token,subject}` 稳定标记约定可用；本 capability 不需要新增依赖——依赖图与容量声明都是纯值，云链路由宿主注入。

## 2. 依赖方向与可用性（A10 / E07）

- [x] 2.1 `cloud.rs`：`NodeClass::{Local, Cloud}` 与 `LinkGraph`（有向边 `from depends_on to`）；`add_local`/`add_cloud`/`depends_on` 校验节点存在、拒绝自环、节点数上限 4096。
- [x] 2.2 `cloud_reach(node)`：沿依赖求**传递闭包**，返回可达云节点集合；节点未声明返回 `None`（与「真是云依赖」区分）——两跳依赖与直连同属可达。
- [x] 2.3 `audit_local_autonomy(graph, critical)` 做成对**公开图值**的纯函数，三类违规稳定标记分开报：`local critical node depends on cloud`（可达云节点，subject 指出两端）、`local critical node is not local`、`local critical node is undefined`；`AutonomyAudit::is_local()` 给出结论。
- [x] 2.4 `availability(link, class)`：`(_, Local) => Available`（本地可用性与链路状态无关，这是 A10 的核心恒等式）；`(Unreachable, Cloud) => Unavailable{"cloud link unreachable"}`。

## 3. 云能力门控（A10 / §27.3）

- [x] 3.1 `CapabilityMode::{Disabled, Active, NeedsPrepare}` 与 `CapabilityGate`（注册归属、显式 `activate`）；`NeedsPrepare` 与 `Active` 是两个状态而非布尔加注释。
- [x] 3.2 `disconnect()`：循环**只遍历** `NodeClass::Cloud` 条目，把 `Active` 摘到 `Disabled`；本地能力连遍历都不进，故「本地不受影响」是作用域事实；返回 `LinkTransition{deactivated, needs_prepare}`。
- [x] 3.3 `reconnect()`：把 `Disabled` 的云能力挪到 `NeedsPrepare`，**不**到 `Active`（§27.3 不自动恢复旧的 Auto 状态）；只有显式 `activate()` 才能运行。

## 4. 低云容量（A10 / E08）

- [x] 4.1 `Measurement{instances, load_pct}`（申报实例与负载）与 `Sizing::{Measured, Unmeasured}`；`Capacity::declare(.., Sizing)` 对 `Unmeasured` 返回 `CAPABILITY_NOT_READY("capacity is unmeasured")`，**没有零默认值**。
- [x] 4.2 测量条件校验：`instances == 0` 或 `load_pct ∉ 1..=100` 一律 `INVALID_ARGUMENT`；合法容量保留 `measurement` 作为其构造的一部分。
- [x] 4.3 `CapacityBudget` 与 `assess(&capacity, &budget)`：逐项比对连接/内存/延迟，**报出全部**越界族（`connections over budget` / `memory over budget` / `latency over budget`），不隐藏后项。
- [x] 4.4 `ThrottlePolicy::{RateLimit, Absorb}` 与 `audit_low_cloud`：越界且策略为 `Absorb` 时报 `budget breach absorbed instead of throttled`（静默吞掉会把量过的天花板换回没量过的）。
- [x] 4.5 测试 `tests/cloud.rs`（21 项）：依赖图 6、可用性 3、门控 5、容量 7，覆盖上列全部判据与三类违规标记。

## 5. 规格、文档与提交

- [x] 5.1 新增 `docs/cloud.md`（依赖方向图与三类标记、可用性恒等式、门控与重连入 Prepare、容量的可构造性与全量越界、限流审计、未做边界）。
- [x] 5.2 新增 ADR 0028，记录放入 `nai-upgrade-core` 而非新 crate 的理由，并记录否决方案（独立 crate / 放 kernel / 只查直连 / 写成文档 / 运行时探测作为唯一实现 / `Option` 加零默认 / 只报首个越界 / 重连即 Active / 重连还原快照 / 断连全停）。
- [x] 5.3 报告 `docs/reports/2026-10-10-w26-cloud-disconnect.md`：实测数字、逐项验收映射（A10）、未覆盖边界与环境边界。
- [x] 5.4 `tests/registry.yaml`：A10 补证据并按实现 `pending → implemented`（**不标 `tested`**——真实断网/弱网与真实云 SDK 未做）；`docs/development-plan.md` 记录本轮。`check --registry` 通过。
- [x] 5.5 归档 `w26-cloud-disconnect`（归档名 `2026-10-10-w26-cloud-disconnect`，新 capability `local-autonomy` 生效），跑 `validate --all --strict`（含 `--archived`）。
- [x] 5.6 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：依赖方向与可用性（§2）、门控（§3）、容量（§4）三部分共用同一个 `cloud.rs` 与同一个 capability，分开提交会让首次提交的测试目标不完整；文档、ADR、registry、归档与门控（§5）为纯声明变更，代码状态与门控时一致，沿用同一次门控留档。
