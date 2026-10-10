## 1. 立项与骨架

- [x] 1.1 OpenSpec 变更 `w27-multi-session-isolation`（proposal/design/tasks + `multi-session-isolation` 增量），`validate --strict` 通过。
- [x] 1.2 落点确认：放进 `crates/nai-kernel`（kernel 只允许依赖 contracts，`xtask/src/deps.rs`），不放进 shared 层、不新开 crate——18 包与依赖方向不变；自带 `MultiplexError{code, reason}`（稳定 token，与 `UpgradeError` 同形）。

## 2. 每会话命名空间与 lane 访问（A11）

- [x] 2.1 `Lane`（封闭六值 `Task | Audio | Approval | Budget | Knowledge | Output`）与 `LANES` 常量；`SessionNamespace { session_id, epoch, lanes }` 为公开值，使审计可做成纯函数。
- [x] 2.2 `Multiplexer`：`open`（重复即 `CONFLICT`；上限 `MAX_SESSIONS = 64`）、`close`、`bind`（同会话重复绑定即 `CONFLICT`；同 lane 在不同会话不冲突）、`namespace`/`sessions` 快照。
- [x] 2.3 `advance_epoch(session)` 只推进被点的会话（checked_add，耗尽即 `RESOURCE_EXHAUSTED`）——独立 epoch 不是共享计数器。
- [x] 2.4 `access(requester, owner, lane)`：requester ≠ owner 即 `CONFLICT("lane belongs to another session")`；会话未知或 lane 未绑定即 `CAPABILITY_ABSENT`。跨会话**没有**访问路径，「无串场」是签名的性质。

## 3. 共享白名单（A11）

- [x] 3.1 `SharedKind` 封闭三值（`ImmutableModel | KnowledgeSnapshot | ConnectionPool`）与 `SHAREABLE` 常量；活的 lane 状态**没有**可共享表示——「共享只限三类」是 API 形状。
- [x] 3.2 `SharedPool::new(max_holders)`：上限为零在构造时即拒（避免第一次 acquire 才发现池不可用）；`acquire` 对同一 `(kind,key)` 达到持有者上限即 `RESOURCE_EXHAUSTED("shared holder limit")`，同会话重复 acquire 幂等；`release` 释放一个槽位，未持有时 `CAPABILITY_ABSENT`。
- [x] 3.3 `SharedPool::write` 永远以 `PERMISSION_DENIED("shared resource is read-only")` 拒绝——「只读快照」可测而非承诺。

## 4. 分族容量准入（A11）

- [x] 4.1 `AdmissionFamily` 四族（`GpuThroughput | Vram | AudioTiming | ApiQuota`）与 `Request`；`Admission::new(limits)` 上限表，未声明的族按 0（保守读法）。
- [x] 4.2 `admit(session, requests)` 两阶段：先按族汇总本次请求（零量即 `INVALID_ARGUMENT`），逐族对照**全机总量**检查，**全部通过才写入**用量；任一族不足即 `RESOURCE_EXHAUSTED("family capacity exceeded")` 且不写任何用量——拒绝不半提交，已开播会话的数字原封不动。
- [x] 4.3 `used_total`/`used_by` 查询与 `release(session)`（只释放该会话份额）。
- [x] 4.4 测试 `tests/multiplex.rs`（24 项）：独立 epoch、跨会话访问被拒、未绑定 lane 缺席、每会话每 lane 只绑一次、六 lane 可寻址、重复 open 冲突、注册表有界、close 退休命名空间、白名单恰三类、持有者上限、重复 acquire 幂等、共享无写路径、release 释放槽位、零上限被拒、分族独立判定、拒绝不伤已开播会话、部分越界零提交、同族多请求求和后再判、未声明族不放行、零量请求被拒、release 只影响目标会话、健康注册表审计干净、重复 session id 被审计报出。

## 5. 规格、文档与提交

- [x] 5.1 新增 `docs/multi-session.md`（命名空间与独立 epoch、以属主为键的 lane 访问、三类只读共享与受控池、分族两阶段准入、隔离审计标记、与 W25 驻留账本的维度区分、未做边界）。
- [x] 5.2 新增 ADR 0029，记录放进 `nai-kernel` 而非 shared/新 crate 的理由（分层约束），并记录否决方案（全局 lane 表 + 检查 / 类型级隔离 / `allow_share` 布尔 / 允许写共享 / 池无上限 / 四族合成总量 / 逐族 check-then-commit / 未声明族按不限制）。
- [x] 5.3 报告 `docs/reports/2026-10-10-w27-multi-session-isolation.md`：实测数字、逐项验收映射（A11）、未覆盖边界与环境边界。
- [x] 5.4 `tests/registry.yaml`：A11 补证据并按实现 `pending → implemented`（**不标 `tested`**——真实多路 GPU/音频时序未做，准入用宿主给定的上限表）；`docs/development-plan.md` 与 `AGENTS.md` 记录本轮。`check --registry` 通过。
- [x] 5.5 归档 `w27-multi-session-isolation`（归档名 `2026-10-10-w27-multi-session-isolation`，新 capability `multi-session-isolation` 生效），跑 `validate --all --strict`（含 `--archived`）。
- [x] 5.6 全量 `cargo xtask check --profile portable` 通过并留档；提交 + 推送。

## 里程碑切分

本轮为单个可独立验证的切片：命名空间与 lane（§2）、共享白名单（§3）、分族准入（§4）共用同一个 `multiplex.rs` 与同一个 capability，分开提交会让首次提交的测试目标不完整；文档、ADR、registry、归档与门控（§5）为纯声明变更，沿用同一次门控留档。
