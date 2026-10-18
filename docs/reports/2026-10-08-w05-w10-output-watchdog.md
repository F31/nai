# W05/W10 前置：输出进程看门狗演练实测报告

- 日期：2026-10-08；变更：`w05-w10-output-watchdog`（OpenSpec SDD，主规格 `output-watchdog-process`；ADR 0016）。
- 对应 V3.6：W05 output-keeper、W10 集成与故障注入；控制/媒体进程边界（§3.3/§8.3/§12.1）。
- 验收：R03（故障隔离）缺口补一条可移植证据；R01 保持设备约束待测。
- 命名：不新增 D 编号，按 V3.6 工作项推进。

## 目标与范围

建立**显式、仅演练**的独立输出看门狗进程，验证真实子进程场景下的独立租约检查与撤权，
而不是把进程内 MediaGate 单测当作 engine/keeper 故障隔离。

明确不做（保持未开始，不冒充完成）：
- 不输出 PCM/帧，不生成播放 ACK，不授予 Ready；真实设备/窗口/音轨/声卡始终 Unavailable。
- engine/bridge/worker 正式进程、Windows 命名管道身份/ACL、真实设备「600ms 内实际停声」尚未实现/认证。
- 共享内存数据面（W06）与本机窗口/2D 渲染未在本轮实现。

## 实现

### Protobuf 工程子契约（provisional）

`OutputWatchdogCommand`（schema_version/session_id/seq/budget + oneof arm|renew|inspect）与
`OutputWatchdogStatus`（epoch_floor、lease_valid 诊断、input_closed、output_available=false、
backend_error=CAPABILITY_NOT_READY、last_control_error、dropped/skipped）。先改 media.proto 并生成
Rust/TS/C++（109 消息）；12 类消息互通 fixture 覆盖了大整数与 unavailable 状态。契约标记 provisional，
不冻结为完整 output.deliver 接口（ADR 0015）。

### nai-output-keeper（新 media 层 apps 包）

- `Watchdog` 复用 `MediaGate`；新增 `MediaGate::with_epoch_floor`（无授权的 floor 构造，supervisor 提供
  幸存/恢复 floor，不接受该 floor 及以下的旧授权）。仅接受 Rehearsal 租约，拒绝其他模式。
- `lib.rs::run`：可信启动器继承的私有 stdio；reader/writer 独立线程；主循环 10ms tick。输入容量 16、
  帧 ≤64KiB、在途帧 250ms 期限、seq 单调、budget ≤250ms、lease 剩余 ≤600ms。EOF/非法帧/溢出/阻塞读取
  到期会逻辑关闭控制并撤权（EmergencyHandle + poll），迟到控制不得恢复。
- 输出只发布「最新一份」pending 诊断状态（try_lock + skipped 计数），writer 阻塞不阻塞看门狗 owner。
- `main.rs` 仅 `--watchdog-drill <session> <trusted-floor>` 可启动，其余参数返回退出码 2。
- `MediaGate::with_epoch_floor` 释放 epoch floor 构造：新旧资料见下文测试与 [ADR 0016](../adr/0016-output-watchdog-drill-boundary.md)。

### tests/support/lease_source.rs（tests 专用，stdlib 编译）

独立编程 `stdio` opaque frame 源，演示停续租、平缓 partial 卡读。「stall」在写满输入后在返回前 sleep；
「partial」写入 4 字节不完整帧头后 sleep，验证 partial-control 到期不被同步 read 阻塞绕过。测试用它
通过 `#[cfg(test)]` 目的根真实的子进程强杀/停止续租/阻塞读取场景。发布包不含该 fixture。

## 实际运行的命令与结果

| 命令 | 结果 |
|---|---|
| `cargo test -p nai-output-keeper --offline` | 11 项通过（5 单元 + 6 integration） |
| `cargo xtask test --suite contracts` | 12 类消息三语言互通通过；schema 无 drift |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 |
| `cargo xtask check-deps` / `check --registry` | 13 个 workspace 包、registry.yaml 通过 |
| `cargo xtask schema --check` | 109 消息生成无 drift |
| `cargo xtask check --profile portable` | portable 依赖/clippy/测试通过（250 项 Rust） |
| OpenSpec strict validate（实施前） | valid；归档后见下 |

### 子进程故障演练观测

| 场景 | 观测 |
|---|---|
| 强杀控制源 | logical revoke 在源 SIGKILL 后约 10ms 观测到，keeper 存活 |
| 源存活但停止续租 | lease 在无续租下约 577ms 后到期（看门狗自己的时钟），keeper 未关闭 |
| partial 控制帧卡读 | 在 250ms 在途预算到期后输入关闭，验证 10ms 轮询不被同步 read 阻塞绕过 |
| 强杀 keeper 再用旧 floor 启动 | 新 keeper 拒绝 floor 及以下旧租约；需更高 floor 单独准入 |
| 控制输入风暴 / 超大帧 / 缺失演练模式参数 | 有界并给出明确 CAPABILITY/退出码，不授予输出 |

新增测试中 `lease_source` 持续睡眠在返回后完成强杀，但不伪造 Windows/真实媒体进度。

## 验收影响

- R03 由「无任何独立进程证据」补为：真实子进程强杀/停续租/阻塞读取仅让软件租约失效并撤权，
  R03 仍保持 implemented 而非 tested（真实 engine/bridge/worker 与设备故障矩阵缺口保留）。
- W05/W10 行更新：本演练过程完成；剩余窗口/音轨/共享内存/正式独立 P0 通道在本轮外。
- R03 由「无进程级证据」补为可移植证据，仍保持 implemented（真实设备/正式进程矩阵未测）。
- 测试运行中 portable 曾超时，改为 `cargo test --workspace --locked` 独立复跑 250 项通过；
  最终列举的 portable 检查是在修复 clippy 之后通过的。
- OpenSpec strict validate / archive / spec:validate / validate --archived 通过：20 个主规格、18 项归档（0 失败）。

## 未测环境与外部阻塞

- 正式 engine/keeper/worker/bridge 进程：尚未实现，强杀矩阵不可测；本轮仅证明 tests 自有源进程与
  演练 keeper 的边界事实。
- 真实设备「600ms 内实际停声」、Windows 命名管道/ACL、1窗口/音轨 P95、共享内存数据面与渲染：未实现，
  认证未执行（保持 Blocked/未开始）。
- 决策日志/值守事件运行时接线：composition root 未建立（W17 范围）。

## 归档与后续

本轮已归档为 `openspec/changes/archive/2026-10-09-w05-w10-output-watchdog/`，6/6 任务完成，
主规格 `output-watchdog-process` 生效。后续 W05 继续：真实 compositor/设备管理（窗口/音轨/帧数据面）
以及把 watch（per session）转成 keeper 正式主循环；W10 补真实 engine/worker 故障矩阵与 P95。
Windows/SDK/设备里程碑继续保留阻塞声明。