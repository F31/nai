# W02/W03 决策日志哈希链与值守租约（Slice-0 前置）

- 日期：2026-10-08
- 变更：`openspec/changes/w02-w03-decision-log-duty-lease/`（OpenSpec SDD，主规格 `decision-log-integrity`、`duty-lease`）
- 对应 V3.6：W03（存储与决策日志）、W02（内核会话与租约）；不变量 INV-19、INV-08、INV-15
- 验收影响：R07（pending → implemented）、S01（补值守证据）
- 命名说明：本轮不使用 D 编号（D16–D19 为 V3.5 遗留占位），按 V3.6 §31 工作项标识

## 目标与范围

实现两个 Slice-0 前置机制：

1. 只追加哈希链决策日志（V3.6 §5.8、R07 前置）：表、链、分级写入与链校验。
2. 自动模式值守租约（V3.6 §21.4、INV-08）：操作员显式确认门控，到期停自动并在 600ms 内撤权。

明确不做（保持未开始，不冒充完成）：桩 Provider 回放一致性、决策日志与运行时的接线（composition root
尚不存在）、播放 ACK 生产端（无 output-keeper）、故障域强杀进程测试（无 keeper/worker 进程可杀，留下一轮）。

## 实现

### 决策日志（nai-storage）

- 迁移 `migrations/sqlite/0005-decision-log.sql`：`decision_log` 表（STRICT，含 check 约束）、
  `(workspace_id,seq)` 索引、`BEFORE UPDATE`/`BEFORE DELETE` 触发器拒绝一切改动。schema 版本 4 → 5。
- `crates/nai-storage/src/decision.rs`：
  - `DecisionKind`（12 类）与 `criticality()`：approval/cue/cancellation/duty_lease 为 critical，
    其余为 telemetry（落实 ADR 0015 勘误「关键提交先持久化、遥测可丢」）。
  - `append_decision`：critical 直接持久化，失败原样返回；telemetry 失败进入容量 1024 的有界缓冲，
    满则丢弃最旧并累加 dropped 计数；每次写入前尝试按到达顺序冲刷，冲刷时按持久化尾部重新链接，
    被丢弃遥测等同从未记录，存活链仍可校验。返回 `DecisionAck{persisted, entry_hash, dropped_oldest}`。
  - `verify_decision_chain`：逐条重算 payload_hash/prev_hash/entry_hash，报告首个不一致位置。
  - `decision_buffer_stats`：缓冲长度与丢弃计数，供诊断与测试。
- 哈希前像：`prev_hash\nkind\ncriticality\nsession\npayload_hash\ncreated_at_ms\ntrace_id`；创世 prev_hash 为 64 个 0。
- 契约同步：`schemas/config/backup.schema.json` 的 `db_schema_version` const 4 → 5，`cargo xtask schema`
  重新生成 TS 类型（`packages/contracts-ts/src/generated/config/backup.ts`），`tests/contracts/schema.test.mjs`
  与备份清单断言同步；`cargo xtask schema --check` 无 drift。

### 值守租约（nai-kernel）

- `crates/nai-kernel/src/duty.rs`：`DutyLease`、`DutyEnforcement{Valid,HaltedPendingRevoke,Revoked}`、
  `DUTY_REVOKE_DELAY=600ms`（ADR 0015 初值）。
- `Session` 集成：`confirm_duty`（仅此路径授予/续期）、`note_heartbeat`（刻意不延长）、`duty_valid`、
  `auto_reply_permitted`（Running + AutoSupervised + 有效租约）、`enforce_duty`（到期停自动，600ms 后
  走 epoch 递增的撤权路径并置媒体 Muted，幂等）；`change_mode` 切 AutoSupervised 需有效租约，
  否则拒绝且模式不变（签名增加 `now`，调用点仅 session 内部 command）。
- 恢复语义：租约仅在内存，`recover`/`recover_after` 不继承自动权限；除显式确认外无恢复路径。

## 实际运行的命令与结果

| 命令 | 结果 |
|---|---|
| `cargo test -p nai-storage --offline` | 40 项通过（新增 6 项决策日志测试） |
| `cargo test -p nai-kernel --offline` | 42 项单元 + 4 项值守租约 + 平台/调度/守门等集成测试通过 |
| `cargo test --workspace --offline` | 232 项通过，0 失败（D14 时 222 项，本轮 +10） |
| `cargo xtask schema` / `schema --check` | 生成成功；重新生成无 drift |
| `cargo xtask check --registry` | 通过（44+ 项验收、映射不悬空） |
| `cargo xtask check-deps` | 通过（12 个 workspace 包依赖边界） |
| `cargo xtask test --suite contracts` | 通过（三语言编译、schema/OpenAPI 校验、双向互通 10 类消息） |
| `cargo xtask test --suite storage` / `--suite kernel` | 通过 |
| `cargo fmt --all --check` | 通过（本轮修正模块声明顺序） |
| `cargo xtask check --profile portable` | 通过：fmt/clippy(-D warnings)/依赖边界/全量测试；设备/SDK/GPU/安装认证未执行 |
| `pnpm -r typecheck` | 通过（contracts-ts 与 desktop） |
| `pnpm exec openspec validate w02-w03-decision-log-duty-lease --strict --no-interactive` | valid |
| `pnpm exec openspec archive w02-w03-decision-log-duty-lease --yes` + `validate --archived` | 已归档为 `2026-10-08-w02-w03-decision-log-duty-lease`；16 项归档、19 个主规格 0 失败 |

新增测试清单：

- nai-storage（`src/tests/decision_log.rs`，6 项）：追加+链校验、伪造条目检出与拒改删、critical 失败不入缓冲、
  telemetry 缓冲/溢出丢最旧/工作区出现后按序冲刷且链校验通过、workspace 链相互独立、超大载荷拒绝。
- nai-kernel（`tests/duty.rs`，4 项）：授予与到期边界、心跳不续期、切自动需租约、
  600ms 撤权与幂等撤权、失效后仅显式确认可恢复。

## 未测环境与外部阻塞

- 真实播放 ACK、真实设备「600ms 内实际停声」、output-keeper 侧租约校验：无 output-keeper 进程与真实
  音频设备（Windows 设备里程碑），保持未实现/未实测。
- 决策日志与 runtime 接线、桩 Provider 回放：composition root 未建立，W17 范围。
- 强杀 engine/keeper/worker/bridge 的进程级故障演练：R03 缺口保留记录，下一轮候选。
- 值守租约授予/到期写决策日志：kernel 不依赖 storage，未接线，未在报告中宣称已记录。
- R07 保持 implemented 而非 tested：哈希链与校验已实测，但 V3.6 R07 通过标准还含「用桩 Provider 回放
  与原决策一致」，该部分未做。
- 哈希链只保证逻辑完整性，不防拥有写库权限者整体重写；不夸大防篡改声明。

## 后续依赖

- output-keeper（W05）与 render-worker（W06）落地后接播放 ACK 与真实租约静音。
- composition root（runtime）建立后接决策日志写入与值守事件记录。
- 下一轮候选：故障域补测（进程级强杀基线）、审批端点与待审批上限（O02）、影子/回放（W17）。
