# 运行时装配根与子进程监督（W05/W06 后续）

`nai-runtime`（layer `runtime`，ADR 0019）是本轮新增的装配根库：它持有唯一存储 owner、会话与值守租约、
媒体门、审批后端与决策日志，并驱动受监督的媒体子进程。它不是引擎可执行程序——`apps/engine`、
loopback HTTP 服务启动与 broker 装配接线仍待后续里程碑。

## 装配（Composition::prepare）

`Composition::prepare(paths, workspace, actor, profile, bootstrap)` 的固定顺序：

1. `Store::open` 取得唯一存储 owner（第二 owner 打开被拒），包为 `Arc<Mutex<Store>>`。
2. 仅 `bootstrap=true` 时执行可信 owner 管理：建工作区并授予 `config.read/write`、`approval.read/write`、
   `session.manage`。该路径**只对可信 owner 开放**，模型/插件永远不可达；工作区必须不存在（一次性）。
3. `load_config(ConfigKind::Runtime, profile)` 装载经 JSON Schema 校验的运行时配置；缺 `config.read`
   权限或配置非法在装配期显式拒绝。
4. 交叉检查 `media.lease_renew_ms < lease_expire_ms` 与 `runtime.max_active_sessions ∈ [1,1024]`。
5. 构造 `Session`（session_id 即 workspace id）、`MediaGate::with_epoch_floor`、
   `Registry::new(boot_id, Limits)` 与 `ApprovalBackend`（共享 store）。

⚠️ **`config_json` 的形状被强制，不等于它的每个键都被运行时看见。** `schemas/config/` 下**每份**
schema 的 `required` 键的**消费状态**由 `scripts/contracts/config-consumption.mjs` 逐键在册、在门控内
执行：每个受判 `required` 键 MUST 落在「有读取点（**非测试、非生成**的源码，且在该 schema 的**归因
文件集**内——`include_str!` 它的文件 ∪ 具名外部读者）／值被 schema 用 `const` 钉死（值不可能不同 ⇒
无需读者）／在册登记（附可核实的成因）」三类**之一**；三类 MUST 两两不相交、并集恰等于受判键集合。
⇒ **加一个 `required` 键而没有任何东西读它，门控会红**；读取形状覆盖**直接索引／中间变量／辅助函数／
结构体解码**四种（判据与两张台账的成因见 [ADR 0068](adr/0068-config-surface-key-consumption.md)、
[ADR 0069](adr/0069-config-key-attribution.md)）。
⚠️ **判定单元是 `(schema, 键路径)`，不是裸键名**：`schema_version` 在 5 份 schema 的顶层同形，
`["path"]`／`["session_id"]` 等通用名会在**别的对象**上命中（实测 `crates/nai-plugin-sdk`／
`crates/nai-control-api`）⇒ 「同名」MUST NOT 被当作「同一个键」，归属外的命中 MUST NOT 授权。

每次存储/决策访问新建 5s `Operation`（上限 30s，无长租表达），P0 静音经 `EmergencyHandle`
不等数据库锁。

## 控制与值守

- `control(Mute|Takeover)`：先 `gate.emergency().mute()`，再 `Session::revoke`（epoch 递增），
  最后写 critical `cancellation` 决策日志。返回的 epoch 只表示状态变更，**不代表设备已停**。
- 值守包装：operator-only `confirm_duty` 授予、`enforce_duty`/`tick_duty` 到期/撤权，
  均写 critical `duty_lease` 日志；到期执行先静音再撤权再落日志。
- `record` 按 `DecisionKind::criticality()` 分级：critical 失败直接返回错误，telemetry 允许缓冲。
- `rehearsal_lease` 只产出 Rehearsal 模式有界租约（TTL ≤600ms）；生产播放租约需审批与事实，不在此生成。

## harness 观测落链（W103，验收 H15）

`Composition::record_observation(record, policy)` 是 §47.1 五类观测**进入只追加哈希链**的唯一入口：
它先调 `nai_kernel::evaluate::harness::evaluate_observation` 判定（判定规则本身由 W66 落地，本轮**一行未改**），
再按判定结果分两路：

- **可落日志** ⇒ 追加一条 `DecisionKind::Observation` 条目（telemetry 级），载荷**恰为**判定输出的
  脱敏材料三项：层标签、`verdict.redacted`、`verdict.redacted_viewer`。
- **不可落日志** ⇒ 返回 `ObservationRefusal{layer, rejections, redacted}`，**一个字节都不落盘**；
  返回类型是枚举 ⇒ 「已落日志」与「已拒绝」**类型上**可分。

⚠️ **runtime 是传输者，不是加工者**：载荷 MUST NOT 取 `record.fields`，也 MUST NOT 添加任何判定没审过的
字段——故**不带** `record.subject`（判定只扫 `fields`，`subject` 永不被审）。把未审的部分补进结果不是
「补全」，而是把判定**扩不到的地方**变成默认可信。见 [ADR 0081](adr/0081-observation-log-wiring.md)。

⚠️ **判定与追加合并为一个入口**：不暴露「先 `evaluate` 再 `append`」两个出口，否则「不许追加未判定的记录」
会退化成每个调用方的纪律，而不是结构保证。

**边界（如实）**：落链路径已可测（八项测试 + 四组非空洞注入），但**五类观测的真实产生者**
（装配裁剪、意图判定、记忆读写、工具调用、缓存命中）**未接线**，属 H01/H02/H03/H04/B07
⇒ H15 仍 `pending`。降级侧（`evaluate_degradation` → `CapabilityView` 与界面）亦未接线，属 H17 的另一轮。

## 装配落链与只读重建（W104，验收 H02）

`Composition::record_assembly(record, fragments)` 是 §41.5 上下文装配**进入只追加哈希链**的唯一入口：
它先调 `nai_kernel::evaluate::assembly::rebuild_assembly` 判定（判定规则本身由 W69 落地，本轮**一行未改**），
再按判定结果分两路：

- **可重建** ⇒ 追加一条 `DecisionKind::Assembly` 条目（telemetry 级），载荷**恰为**判定放行的那一份材料：
  按**记录进入顺序**排列的片段（`id`、来源标识 `source_ref`、**字面正文**）、模型版本、提示词版本。
- **不可重建** ⇒ 返回 `AssemblyRefusal{rejections, unlogged_ids, contentless_ids, credential_ids}`，
  **一个字节都不落盘**；返回类型是枚举 ⇒「已落日志」与「已拒绝」**类型上**可分。

`Composition::rebuild_assembly_from_log(record, limit)` 是**只读**的另一半：读回 `assembly` 条目、
按**日志自身顺序**解出被引用片段，再交给**同一个** `rebuild_assembly`，结果映射为拥有所有权的
`AssemblyRebuild`。判定是**调用**、不是复写第二份；读回 SHALL NOT 追加、修改或重排链。

⚠️ **正文 MUST 落字面**：载荷里的 `content` MUST NOT 为空、MUST NOT 是「指向当时内存的指针」——
H02 的判据正是「日志**自己**能不能重排出同一段序列」。把正文留在内存、只存引用，就是它要禁的那一种。

⚠️ **顺序 MUST 落成记录顺序、重建 MUST 取日志顺序**：`rebuild_assembly` 的比较基准是「日志自身顺序 vs
记录顺序」。写侧若按 id 排序落盘，读侧的比较就**恒真**，`RebuildOrderDiffers` 成空话。

⚠️ **`assembly` 是存储 schema 的**第二个**闭世界**：`decision_log` 的 `kind` 有 `CHECK(kind IN(…))`，
与 Rust 的 `DecisionKind` 枚举并存（迁移 `0007` 逐列重建该表以容纳新种类）。只改枚举而不改 CHECK ⇒ 插入被
schema 拒绝、**静默落入遥测缓冲**（`ack.persisted == false`，不报错）。见 [ADR 0082](adr/0082-assembly-log-wiring.md)。

**边界（如实）**：落链路径与只读重建已可测（十二项测试 + 五组非空洞注入），但**装配器的真实产生者**
（把九段片段喂给模型、§41.2 分段预算初值标定、§47.2 六类回归属 **H16**）**未接线** ⇒ H02 仍 `pending`。
H17（`evaluate_degradation` → `CapabilityView` 与界面）亦未接线。

## 子进程监督（Supervisor）

- 二进制路径由调用方显式传入（`Binaries::validate` 缺失即 `CapabilityNotReady`）；库内不用
  `env!("CARGO_BIN_EXE_…")`，跨包不可见。
- `spawn_keeper`：`--watchdog-drill`，可选 `--frame-drill`（池 FD 经 stderr 继承，与 keeper 约定一致）；
  `arm`/`renew` seq 递增、`remaining_budget_ms ≤250`，仅 Rehearsal 租约。
- `spawn_worker`：`--render-drill`，PCM 原始字节与 protobuf ticket 经文件传入，帧数据不走 JSON/Base64；
  可选 `--ready <path>` 预热标记——worker 先 `Avatar::admit` 写标记后有界等待 reserve ticket
  （冷启动 ~290ms 超过槽位 250ms 非延长预算，故先预热再 reserve）。
- `observe()` 故障处置顺序固定：**静音 → revoke（epoch 递增）→ critical 日志 → standby**；
  日志失败上报但绝不回退静音/撤权，standby 无条件设置。
- `shutdown` kill + 有界回收；非 Linux 显式不可用。

## render-worker（apps/render-worker，layer media）

显式 `--render-drill`；池 FD 经 stderr 继承并读写重开（`/proc/self/fd/2`）；`nai-avatar-2d`
真实像素经 `validate_producer_handle`/`write_frame` 写入池；stdout 仅输出有界
`FrameSlotEvent{write_started|committed|error}`；复检 epoch/取消/期限，失败 error 事件 + 退出码 2。
不产生 PlaybackAck、不声明 Ready、不接设备。

```bash
cargo build -p nai-output-keeper --offline   # supervisor 测试需 keeper 二进制先存在
cargo test -p nai-runtime -p nai-render-worker --offline
```

实测见 [报告](reports/2026-10-09-w05-w06-composition-root.md)：强杀 keeper 后撤权/standby/日志落库且
哈希链校验通过；render-worker 真实像素被 owner commit 接受、consumer 读到 owned 副本；
陈旧 epoch ticket 与篡改 SHA256 均被拒并退休整池。
