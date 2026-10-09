# D06 会话、租约与取消实测报告

## 状态

- 已实现：kernel 会话状态机、模式变更、epoch、两层任务取消、认证上下文控制命令、有界去重；
  media-core 专用原子静音、本地单调租约、消费复检、元数据队列与完成 ACK；storage 撤销 CAS 事务。
- Linux/WSL 实测通过；新增 14 项 meaningful 测试（kernel 5、media 7、storage 1、哈希 1）。
- 未开始：真实媒体工作循环/PCM/设备缓冲/独立进程接线（D11）、实际 Provider 异步流取消（D08）。
  本轮是任务令牌和迟到结果拒绝机制，不是已完成的 Provider 取消集成。
- Blocked：Windows 原生设备/热键/强杀验收无执行环境；storage Windows GNU 检查缺 MinGW C 编译器。
- NotApplicable：线协议字段变更、数据库迁移、付费 API 集成（此轮未涉及）。

## 修改

`crates/nai-kernel/src/session.rs`、`crates/nai-media-core`、storage checkpoint、contracts 候选哈希公共算法、
workspace/Cargo.lock、xtask session suite、CI、OpenSpec 与文档。
复用现有 Protobuf，生成产物无漂移；sha256-canonical-v1 算法移至 contracts，storage 兼容重导出。
无新增第三方包版本或常驻依赖、模型下载、云资源、API 消费。

## 实际检查

| 命令 | 结果 |
|---|---|
| `cargo xtask check --profile portable` | 8 包依赖边界、fmt、Clippy -D warnings、87 项 Rust 测试通过 |
| `cargo xtask test --suite session` | kernel 23 + media 7 + storage 13 = 43 项通过 |
| `cargo xtask schema --check` | 27 能力、103 消息；Rust/TS/C++/JSON Schema/OpenAPI 无漂移 |
| `cargo check --locked -p nai-kernel -p nai-media-core --tests --target x86_64-pc-windows-gnu` | 代码与测试交叉检查通过，未运行 Windows 测试 |
| `cargo check --locked -p nai-kernel -p nai-media-core -p nai-storage --tests --target x86_64-pc-windows-gnu` | Blocked：bundled SQLite 找不到 x86_64-w64-mingw32-gcc |
| `OPENSPEC_TELEMETRY=0 pnpm exec openspec validate d06-session-leases-and-cancellation --strict --no-interactive` | 实施前工件校验通过 |

全量 87 项按实际输出计数：contracts 6、kernel 23、media 7、hosts 9、SDK 14、security 4、storage 13、xtask 11。
之前 D05 文档的全量“42 项”不是当前可复现 workspace 全量计数；本轮使用实际输出。

## 故障与负面覆盖

- 未授权/未通过 preflight 不启动；跨会话任务隔离；普通取消不增加 epoch；全场取消丢弃迟到结果。
- 旧命令在 epoch 改变后重试返回原 ACK；改载荷冲突；容量满不淘汰窗口记录；重复静音不重开播放。
- 普通锁被持有时另线程 P0 无等待；媒体先失效，即便命令去重容量耗尽/内核未更新 epoch。
- 引擎停续租后精确在单调期限过期；墙钟回退无绕过；旧租约不能 renew/arm 复活。
- 修改文本且保留旧哈希、修改哈希、事实版本、会话/epoch、撤销审批、guard/权限缺失均拒绝。
- 值守到期不播放；队列有界；续租不修改授权，解除静音须新 epoch。
- 已提交不等于完成；错误 output、样本数与取消后的 ACK 不标完成。
- 检查点未授权/CAS 冲突拒绝；同事务保存 epoch、撤销审批、取消未完成任务。安全恢复协调媒体 epoch floor。

## 失败与修复

- workspace 新成员及依赖需更新 Cargo.lock；首次 --locked 检查因此失败，随后通过 cargo check --workspace --offline
  更新锁文件，再重跑 locked 检查通过。
- 首次 storage Windows 检查缺 MinGW C 编译器，未伪造成功；kernel/media 单独交叉检查通过。
- 复检发现候选只比较自报哈希不足；复用已有规范化算法直接重算对象，补篡改内容与 map 顺序测试后全量通过。
- 归档 CLI 为新规格生成默认 Purpose 占位，归档后 strict 校验拒绝；按诊断补充主规格的实际 Purpose 后重新校验。

## 接线与边界

root 鉴权后先 EmergencyHandle::mute，再 Session 撤销，随后存储 CAS；存储失败不授予新租约。
root 必须在每次消费重建可信 Preplay，不把 guard/权限 bool 当作插件自报授权。
库要求媒体工作循环定期 poll；当前没有真实设备音轨，未声称本地 P95 ≤250ms 或强杀进程设备静音通过。
CommandAck executed 仅是状态 ACK，不是设备停止 ACK。重试账本在内存中，恢复清空但旧 epoch 命令拒绝，
未知已播进度不自动重播。checkpoint 不保存全部运行生命周期，后续 root 接线需明确状态持久化策略。

下一项：D07 调度/预算（有界优先队列、语音债务、公平、TTL、配额结算）。未提交或推送。

## OpenSpec 归档

6/6 任务完成，合并 4 条需求，归档目录：
`openspec/changes/archive/2026-10-07-d06-session-leases-and-cancellation/`。
主规格：`openspec/specs/session-leases-and-cancellation/spec.md`。
归档后全部 8 个主规格与 D01–D06 六项归档校验通过（OPENSPEC_TELEMETRY=0）。
