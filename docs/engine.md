# 引擎装配（apps/engine）

`nai-engine` 是本仓库第一个可启动的装配可执行程序：它把存储 owner、共享会话、loopback 控制面与
受监督媒体子进程装配成一个进程（ADR 0020）。它不是生产形态：无 protected IPC、无设备/窗口绑定、
无 PlaybackAck 生产端、无 Windows 认证。

## 参数

```
nai-engine --data-root <dir> --workspace <id> --actor <id> --profile <name>
           --token-file <path> [--control-port <u16>] [--halt-file <path>]
           [--keeper <path> --render-worker <path>]
           [--media-drill --pcm <path> --avatar <manifest> --avatar-config <config>
            --asset <id=path>... --license <id>... [--slot-bytes <n>] [--time-base-den <n>]]
nai-engine ... --provision <runtime-profile.json>      # 可信 owner 一次性配置
```

- `--provision`：建工作区、授予运行时权限、写入并校验运行时 profile，随后退出 0（不监听）。
- token 文件必须 `0600` 且内容为 32–256 个 url-safe 字符；权限全集的临时令牌（TTL 1h）。
- `--keeper` 与 `--render-worker` 必须成对给出（`Supervisor::new` 会校验两个二进制都存在）。
- `--halt-file`：显式优雅停机请求。`tokio::signal` 需要 `signal-hook-registry`，该 crate 不在离线
  缓存内且 crate 内禁 `unsafe`，因此本轮用文件触发；被强杀进程不计为优雅停机。

## 启动与运行

1. `Composition::prepare`（唯一存储 owner、配置校验、Session/MediaGate/Registry/ApprovalBackend）。
2. `ControlApi::with_shared_session`：控制面与装配根共用同一会话，撤权 epoch 立即对两侧可见。
3. 绑定 `127.0.0.1:port`（`port=0` 时取临时端口），stdout 打印一行：
   `nai-engine control: http://127.0.0.1:<port>/api/v1`。
4. 监督线程（给出 keeper 时）：spawn keeper（floor = 现场 epoch）→ 以 `floor+1` 签发 Rehearsal 租约
   并 arm → 每 tick **先续租、再 observe**。控制面撤权后停止续租（telemetry
   `renewal_halted_after_revoke`），**不自动重签**；续租失败不静默退出，由 `observe()` 走
   静音→撤权→critical 日志→待机。

## 媒体演练（--media-drill）

Linux `/dev/shm` 私有池 → root 以 `FrameBroker` 声明真实 spawn pid → keeper 经继承的**只读**池 FD 重建
读取端并触发 UDS → broker 按 `/proc` 证据（pid 存活、属 pool owner、持有 pool 文件）授权 → render-worker
（`--ready` 预热，冷启动 ~290ms 超过 250ms 槽位预算）继承写 FD、reserve 并原子写 ticket → 读
`write_started`/`committed` → root accept commit（槽 pin Reading）→ `begin_consume` 后由 keeper 读回
owned 像素 → 匹配 seq/epoch/`buffer_ref`/SHA256 的 keeper 诊断 → `release` → 写 finish/回收 worker。
结果记为 telemetry（`media_drill_committed`，含 buffer_ref/SHA256/字节数与 keeper 观察）。

预授权/等待期间每 tick 续租、`observe()` 并复检共享 epoch 与 halt；worker 事件经 reader 线程入有界队列，
owner 非阻塞轮询，卡读不阻塞期限/停机。keeper 只给 `consumed_buffer_ref`/`consumed_sha256` 诊断，
**不**产生 PlaybackAck、**不**声明 Ready，`output_available` 恒 `false`。

**未接线（如实）**：broker 连接对端认证（UDS 仍只作活性触发，核验的是声明 pid+uid+FD 能力，ADR 0018/0021）、
SCM_RIGHTS/Windows 命名管道句柄传递、protected IPC/信号停机、PlaybackAck 生产端与真实设备/窗口。

## 停机

halt 文件 → 撤 token → P0 静音 + 撤权 + critical `cancellation` → 监督线程 stop（kill + 有界回收）→
HTTP watch 停机 → 退出码 0。

## 验证

```bash
cargo test -p nai-engine --offline
cargo build -p nai-output-keeper -p nai-render-worker --offline   # supervisor 测试需要
pnpm test:engine-smoke            # 真实 engine + 桌面 ControlClient
```

实测见 [报告](reports/2026-10-09-engine-assembly.md)：无 token 401、mute 后 epoch 递增并落 critical
日志、keeper 强杀后撤权且控制服务保持可达、HTTP 撤权后停止续租、媒体演练真实提交、halt 后退出 0。
