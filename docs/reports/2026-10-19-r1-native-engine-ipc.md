# R1 原生控制通道：私有继承管道与引擎生命周期

日期：2026-10-19。变更已归档至 `openspec/changes/archive/2026-10-19-r1-native-engine-ipc/`。依据：
[ADR 0130](../adr/0130-native-control-inherited-pipe.md)、
[ADR 0129](../adr/0129-native-shell-build-isolation.md)、[ADR 0046](../adr/0046-desktop-shell-form-and-tauri-boundary.md)。

## 现在能做什么

- 原生壳从固定候选路径派生 engine，经**私有继承 stdin/stdout** 交换有界 Protobuf 帧；
  engine 在该模式不监听 TCP 端口。窗口内可读取真实状态、使用已接线的手工候选/审批/预检界面。
- 引导凭据由系统随机源生成，只出现在 bootstrap 帧；壳在管道内侧注入 `Authorization`。
  WebView 不持有地址、令牌或进程句柄。
- 请求 4 秒期限、队列 16、每秒心跳；engine 5 秒未收到完整帧即 P0 静音→撤权→critical 日志并退出。
  部分帧、超限、错序、身份不符与超时失败关闭，**不重试**。
- 关窗隐藏到托盘；托盘显式退出有界停止并回收本壳启动的 engine；后台存活不依赖 WebView。

## 实现

- 契约：`schemas/domain/control.proto` 新增 `LocalControlKind` 与 `LocalControlFrame`，已生成
  Rust/TS/C++（`cargo xtask schema --check` 无漂移，113 条 protobuf 消息）。
- 新 shared 层 `crates/nai-control-ipc`：4 字节大端长度、1 MiB 上限、结构/身份校验、
  有界串行 actor、心跳、期限与进程所有权；`getrandom` 复用既有版本。
- engine：`--control-stdio`（与 token/port/provision/preflight/event 互斥）；`local_control` 把帧
  重建为 `http::Request<Full<Bytes>>` 交给**同一个** `ControlApi::handle`；失联经既有停机序列退出。
- 壳：`control_exchange` 唯一命令；`allow-control-exchange` 的 `commands.allow` 恰为一项；
  托盘显示/退出；`src-tauri` 仍是独立 workspace。
- 桌面：`ControlClient` 接受 HTTP 或原生 transport；`native.ts` 只编码/校验帧，无 HTTP 回退与重试。

## 实测

- `cargo test -p nai-control-ipc --offline`：**3 项**（帧边界/部分前缀、kind 越权与版本、4 秒期限且不重发）。
- `cargo test -p nai-engine --offline`：lib **8**；`engine` **19**；`native_control` **5**
  （真实 engine 状态与 mute 进同一日志、6 秒空窗靠心跳存活、部分前缀停机并 P0 日志、静默对端 5 秒租约、
  超限帧具名拒绝）；`pipeline` **13**。
- `cargo test -p nai-control-api --offline`：**50**。
- 壳独立 workspace：`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked`
  lib **2** + `native_engine` **2**（真实 engine 的状态读取、参数不完整/不可启动具名失败）。
- 壳 `clippy --all-targets -- -D warnings` 干净；`--smoke` 在 Xvfb **EXIT=0**。
- `node --experimental-strip-types --test tests/ui/native-ipc.test.mjs`：**6/6**；
  `tests/ui/desktop-shell.test.mjs` **16/16**（权限由 `permissions/*.toml` 派生、单命令核对）。
- `cargo xtask check-deps`：**20** 包，方向不变；`cargo xtask schema --check` 无漂移。

- `cargo xtask check --profile full` **EXIT=0**：**105 个测试目标**（86 `Running` + 19 `Doc-tests`）、
  **1209 passed / 0 failed**；Node **445 项 / 0 失败**。与上一轮（102 / 1201 / 438）的差为
  `nai-control-ipc` 3 项与 `native_control` 5 项、新增测试目标 3 个，以及 UI 新增 6 项与壳权限用例调整。
  归档后复跑同一命令仍 **EXIT=0**，计数逐项相同；`openspec validate --all --strict` **43/43**、
  `--archived` **157/157** 通过。壳的独立测试、clippy 与 `--smoke`（EXIT=0）在归档后复核仍通过。

## 未实现与未验收

- 继承能力证明「谁能写这两个句柄」，**不是**命名管道 ACL、对端 PID 或 AppContainer/Job Object 认证；
  同用户调试器/恶意宿主不在边界内（ADR 0018/0130 的如实登记）。
- 原生壳尚无启动后配置向导、凭据录入、sidecar 的启动/重启策略细化；engine 只在显式参数完整时启动。
- Windows 的 WebView2、托盘、安装器、签名与设备验收未执行；Xvfb 无会话总线，托盘交互**未**人工确认。
- 无设备输出、无硬件 PlaybackAck；A12/O01 保持原状态，WebView 内的控制命令仍受同一权限与审批约束。
- 根 SBOM 不覆盖原生壳的独立依赖闭包，发布前须补。
- `git diff --check` 对**新生成的 C++** 报尾随空格：那是 `protoc` 对全部 20 份既有生成文件的一贯输出，
  不是手工编辑。手写文件的 `git diff --check`（排除 `native/contracts/generated`）干净；生成物不得手改。
