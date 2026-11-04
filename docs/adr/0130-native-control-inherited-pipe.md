# ADR 0130：原生控制走私有继承管道

- 状态：接受
- 日期：2026-10-19
- 依据：V3.11 §10.1、§11、§29、§1259；ADR 0046 D5、ADR 0129

## 背景

ADR 0046 已裁决「受保护 IPC 是生产路径、loopback HTTP 是测试适配」，ADR 0129 交付了可运行的原生壳，
但壳与 engine 之间没有控制通道，操作者无法在原生控制台使用已实现的会话、候选与审批流程。

## 决策

1. **传输 = 派生进程的继承 stdin/stdout**。壳用固定候选路径启动 engine，带 `--control-stdio`；
   engine 在该模式下**不监听任何 TCP 端口**。管道随进程句柄存在，没有可被第三方连接的名字。
2. **消息 = 生成的 Protobuf + 4 字节大端长度**。`LocalControlFrame` 带版本、序号、kind、请求 id、
   方法/路径、JSON 体、状态、TTL；帧上限 1 MiB，结构检查在发送前与接收后都执行。
3. **业务判定只有一处**：engine 把请求重建成 `http::Request<Full<Bytes>>` 交给**同一个**
   `ControlApi::handle`。权限、会话代次、审批幂等与 ACK 语义没有第二份实现。
4. **凭据只走引导帧**：壳用系统随机源生成 32 字节令牌，仅置于 bootstrap 帧；engine 用同一令牌建
   `Access`，壳后续在管道内侧注入 `Authorization`。令牌不进入命令行、环境、磁盘、日志或 WebView。
5. **期限与租约**：客户端请求 4 秒期限、队列有界（16）；后台每秒心跳。engine 5 秒收不到完整帧即
   走既有 P0 路径（静音 → 撤权 → critical 日志）并退出。超时/部分帧/序号/身份不符一律失败关闭，
   **不重试**：已派发请求的结果不确定，重发会把「两次副作用」伪装成一次。
6. **窗口与引擎解耦**：关窗隐藏，托盘显示/退出；显式退出有界停止并回收本壳启动的 engine。
   心跳与进程所有权都在 Rust 宿主，不依赖 WebView 定时器。
7. **能力面最小**：新增唯一 Tauri 命令 `control_exchange`，权限 `allow-control-exchange` 的
   `commands.allow` 恰为一项；WebView 只能提交有界帧，不能命名路径、进程或凭据。

## 成本与边界

- 新增 shared 层 `nai-control-ipc`（依赖 contracts、prost、getrandom）与 Tauri `tray-icon`；
  Linux 构建需已有的 AppIndicator；无新增常驻服务，内核依赖方向不变。
- 继承能力证明「谁能写这两个句柄」；它**不是**命名管道 ACL 或对端 PID 认证，同用户调试器/恶意宿主
  不在本边界内。该限制沿用 ADR 0018 的如实登记。
- 原生壳的 WebView 若被同源脚本攻破，仍可调用唯一的控制命令；CSP 禁止内联脚本仅缩小入口，
  engine 的权限与审批仍逐次生效。不得把这条通道称为完整的 OS 隔离认证。
- Linux 进程/导航证据不替代 Windows 安装、托盘、设备或签名验收；A12/O01 不因本轮升级。
- 根 SBOM 仍只覆盖根 workspace，不含原生壳独立依赖闭包；发布前须补。
