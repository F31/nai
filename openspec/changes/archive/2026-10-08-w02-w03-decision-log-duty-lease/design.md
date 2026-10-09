# Design

## 决策日志（decision_log）

### 表结构（migrations/sqlite/0005-decision-log.sql）

- 字段：`seq INTEGER PRIMARY KEY`、`workspace_id`（FK workspaces）、`session_id`（可空）、`kind`、`criticality TEXT CHECK(IN('critical','telemetry'))`、`payload_json TEXT CHECK(json_valid)`、`payload_hash`、`prev_hash`、`entry_hash`、`created_at_ms`、`trace_id`。
- 只追加由触发器强制：`BEFORE UPDATE` / `BEFORE DELETE` 一律 RAISE(ABORT)。恢复流程（store.rs recover）不触碰 decision_log。
- 每个 workspace 一条独立链；创世条目的 prev_hash 为 64 个 '0'。`entry_hash = sha256_hex(prev_hash \n kind \n criticality \n session_id(或空) \n payload_hash \n created_at_ms \n trace_id)`；`payload_hash = sha256_hex(payload_json 规范化串)`（经 serde_json Value 序列化，禁止调用方手拼）。
- kind 枚举（Rust 层强制）：event、observation、decision、guard、cue、approval、synthesis、playback_ack、cancellation、expiry、usage、duty_lease。playback_ack 仅存储层接受，本轮无生产端。

### 分级写入（ADR 0015 勘误落实）

- critical 类别（approval、cue、cancellation、duty_lease）：同事务直接插入，失败原样返回错误，不进缓冲、不丢弃。
- telemetry 类别（event、observation、decision、guard、synthesis、playback_ack、expiry、usage）：插入失败时进入 Store 内的有界内存缓冲（容量 1024）；缓冲满丢弃最旧条目、累加 dropped 计数并 tracing::warn 告警；每次成功插入前同事务冲刷缓冲。缓冲统计可查询，供诊断与测试。
- 日志写入在 P0 路径之外；本 crate 不提供异步 worker，缓冲语义在单写 owner 内实现。

### 校验

`verify_decision_chain(workspace)` 按 seq 顺序重算 prev_hash/entry_hash，首个不一致处报错并给出 seq（`StorageError.position`）。篡改测试通过直接 SQL 插入伪造条目（绕过 API）验证检测能力；UPDATE/DELETE 由触发器拒绝。

## 值守租约（nai-kernel::duty）

### 语义（V3.6 §21.4、INV-08、ADR 0015）

- `DutyLease { operator, granted_at, expires_at }`，以单调时钟 Instant 表达，不入库（恢复不继承自动权限，§21.4 重启恢复语义已覆盖）。
- 授予/续期只能经 `confirm_duty(operator, ttl, now)` 显式操作；心跳接口不延长 expires_at（提供 `note_heartbeat` 仅刷新“最近心跳”观测值，测试证明 expires_at 不变）。
- 到期判定 `duty_valid(now)`。`auto_reply_permitted(now)` = lifecycle==Running 且 mode==AutoSupervised 且值守有效。
- `enforce_duty(now)`：到期即停新自动回复；`now >= expires_at + 600ms` 时撤销输出授权（走 epoch 递增的撤权路径，媒体状态 Muted）。返回 Valid/HaltedPendingRevoke/Revoked 供调用方区分。
- `change_mode` 目标为 AutoSupervised 时必须 `duty_valid(now)`，否则拒绝（Unauthorized）。
- 重连不恢复：租约到期后除 `confirm_duty` 外没有任何路径恢复自动权限；测试覆盖“平台重连事件后仍无自动权限”。

### 边界

值守租约授予/到期/撤销应写决策日志（kind=duty_lease，critical），但 kernel 不依赖 storage；本轮两者独立交付，接线待 composition root（runtime crate）建立，报告如实注明。

## 测试

- storage：追加+链校验通过；伪造 prev_hash 条目被校验发现；UPDATE/DELETE 被触发器拒绝；telemetry 失败（如无 FK 的 workspace）进缓冲、满容量丢最旧并计数；critical 失败直接报错不进缓冲；两个 workspace 链相互独立；缓冲冲刷后条目顺序与链连续。
- kernel：授予/到期边界；心跳不续期；到期即停自动；+600ms 撤权（epoch 递增、media Muted）；仅显式确认可续期；无值守切 AutoSupervised 被拒；重连不恢复。

## 风险

- 哈希链只校验逻辑完整性，不防有写库权限者整体重写——与 V3.6 威胁模型 T-13 一致（防篡改依赖只追加+离线备份），不夸大声明。
- 缓冲冲刷与插入同事务，崩溃时缓冲内 telemetry 丢失属允许语义（INV-15）。
