# ADR 0019：装配根边界——软件监督闭环，无设备/无 HTTP 服务/无 PlaybackAck

- 日期：2026-10-09
- 状态：Accepted
- 基线：V3.6 §16（R1 口径）、§26.3；承接 ADR 0017/0018（帧槽位与 broker 演练边界）
- 变更：`openspec/changes/w05-w06-composition-root/`

## 决策

新增 `crates/nai-runtime`（layer `runtime`）与 `apps/render-worker`（layer `media`），把既有的
存储/会话/媒体门/审批/决策日志与受监督媒体子进程装配为一个**软件监督闭环**，并明确其边界：

1. **装配根是库，不是引擎**。`nai-runtime` 提供 `Composition`/`Supervisor`；`apps/engine` 可执行、
   loopback HTTP 控制服务启动（D14 的 ControlClient 服务端侧）与 broker 在装配中的接线均未实现。
   二进制路径由调用方显式传入，缺失即 `CapabilityNotReady`，不存在静默跳过。
2. **可信 owner 管理只在 bootstrap**。建工作区与授权是一次性、仅 `bootstrap=true` 的显式路径，
   模型/插件不可达；正常运行只读已 provision 的配置。
3. **控制与故障处置顺序固定**：先 P0 静音（不等数据库），再会话撤权（epoch 递增），再 critical
   决策日志；`observe()` 中日志失败上报但绝不回退静音/撤权，standby 无条件设置。
   ACK 只表示状态变更，不代表硬件停止。
4. **render-worker 是演练进程**：显式 `--render-drill`；帧数据经继承 FD 与文件（PCM 原始字节、
   protobuf ticket）传递，不走 JSON/Base64 控制总线；不产生 PlaybackAck、不声明 Ready。
   冷启动超过槽位 250ms 非延长预算，故采用 `--ready` 预热标记：先 admit 再等待 reserve ticket，
   reserve→commit 只覆盖写入窗口（规格场景不变，仅排序调整）。
5. **决策日志分级**：critical（cancellation/duty_lease 等）失败即错误；telemetry 允许缓冲，
   不冒充持久化。

## 后果

- 软件层面闭环可实测：keeper 强杀 → 撤权 + standby + 日志落库 + 哈希链校验通过；render-worker
  真实像素 → owner commit 接受 → consumer 读到 owned 副本；陈旧 epoch 与篡改哈希被拒并退休整池。
- 残余缺口（不得掩盖）：无设备/窗口/音频验收；无 PlaybackAck 生产端；连接对端未认证（ADR 0018）；
  keeper 二进制需先行构建（`env!("CARGO_BIN_EXE_…")` 跨包不可见，`-Z bindeps` 在该工具链不稳定）；
  非 Linux 显式 `CapabilityNotReady`。
- 验收登记保持诚实：R01/R03/A01/S01 维持 implemented，不标 tested。
