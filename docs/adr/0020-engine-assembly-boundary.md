# ADR 0020：引擎装配可执行边界——共享会话、文件 halt、演练不下沉为设备就绪

- 日期：2026-10-09
- 状态：Accepted
- 基线：V3.6 §16（R1 口径）、§26.3；承接 ADR 0014（本机控制面）、ADR 0018/0019（媒体边界）
- 变更：`openspec/changes/engine-assembly/`

## 决策

新增 `apps/engine`（layer `app`），把装配根、控制面与受监督媒体子进程落成可启动进程，并明确边界：

1. **控制面与装配根共用同一会话**。`ControlApi::with_shared_session` 接收 `Arc<Mutex<Session>>`，
   `Composition` 暴露同一实例（`session()`/`session_lock()`）。理由：此前两侧各持一份 `Session`，
   HTTP 撤权只推进控制面自己的 epoch，监督侧租约与决策日志仍按旧 epoch 工作——违反了
   「撤权后旧权威立即失效」这一核心不变量。锁一律 `try_lock`，忙时显式 409；P0 静音走
   `EmergencyHandle`，不经该锁，因此锁竞争不会推迟静音。
2. **可信 owner 配置独立成步**。`--provision <profile.json>` 一次完成建工作区、授权、写运行时 profile
   并退出；正常启动不再有 bootstrap 分支（原设计里 bootstrap 与「profile 必须已存在」互相矛盾）。
3. **优雅停机用显式 halt 文件**。`tokio::signal` 的 `signal` feature 需要 `signal-hook-registry`，
   该 crate 不在离线缓存内；crate 内 `unsafe` 被 workspace 禁用，因此不使用信号处理器。
   这是**临时且已记录**的触发方式，被外部强杀的进程不得表述为优雅停机；接入 protected IPC 后替换。
4. **撤权后不自动重签租约**。控制面撤权（epoch 递增）后引擎停止续租并记录
   `renewal_halted_after_revoke`；重新授予输出权限必须由显式操作者决定。续租失败不再静默退出监督
   循环，而是继续走 `observe()` 的静音→撤权→日志→待机。
5. **媒体演练仍是显式演练**。`--media-drill` 使用 `/dev/shm` 私有池、真实 render-worker、真实
   reserve→commit→consumer 读回，结果只记 telemetry。**未接线**：broker 的 UDS 授权
   （render-worker 无 broker 客户端）与 keeper 帧投递路径；keeper 在本轮只承担 watchdog 监督。
   不得据此声称数据面端到端完成或设备就绪。
6. **新增只读证据接口**。`nai_storage::Store::decision_entries` 按链序读取决策日志（限流、只读），
   供引擎测试与后续复盘使用；追加与哈希链语义不变。

## 后果

- 软件闭环可实测：无 token 401、mute 后 epoch 递增并写 critical 日志、keeper 强杀→撤权且控制服务
  仍可达、HTTP 撤权→停止续租、媒体演练真实提交、halt→退出码 0 且链校验通过。
- 残余缺口（不得掩盖）：无 protected IPC/信号停机、无 broker 接线与 keeper 帧投递、无
  PlaybackAck 生产端、无真实窗口/音频设备、无 Windows 与性能认证。验收编号 S09/R01/A01/O02 维持
  implemented，不标 tested。
