# Tasks

## 1. 规格与校验
- [x] 1.1 完成 proposal/design/tasks/specs 并通过 `pnpm exec openspec validate w05-w06-composition-root --strict --no-interactive`。

## 2. 装配根（crates/nai-runtime，layer runtime）
- [x] 2.1 新建 crate 并登记 `members`/`default-members`/`workspace.dependencies`/`package.metadata.nai.layer`；不引入新第三方依赖。
- [x] 2.2 `Composition::prepare`：Store 单 owner、工作区引导（显式 bootstrap）、runtime 配置装载与校验、Session/MediaGate/Registry/ApprovalBackend 装配。
- [x] 2.3 `record`（decision log 分级）、`control`（先静音再撤权再写日志）、duty 包装（授予/到期/撤权写 critical 日志）。
- [x] 2.4 单元/集成测试：配置与权限拒绝路径、日志分级、control 顺序、duty 日志。

## 3. 子进程监督（crates/nai-runtime::supervisor）
- [x] 3.1 `Supervisor`：spawn keeper（`--watchdog-drill`/`--frame-drill`，池 FD 经 stderr 继承）、arm/renew、状态帧解析、shutdown 回收。
- [x] 3.2 故障处置 `observe()`：静音 → revoke（epoch 递增）→ critical 决策日志 → standby；日志失败不影响静音。
- [x] 3.3 测试：真实 keeper 子进程 arm 可见 lease_valid；强杀后撤权/standby/日志落库且链校验通过；退出状态与错误路径。

## 4. render-worker（apps/render-worker，layer media）
- [x] 4.1 `--render-drill`：池 FD 经 stderr 继承、`--pcm` 原始字节与 `--ticket` protobuf 输入、avatar-2d 真实像素、`write_frame` 写入、stdout 输出 `FrameSlotEvent`。
- [x] 4.2 复检 epoch/取消/期限；失败发 error 事件并非零退出；不产生 PlaybackAck。
- [x] 4.3 测试：真实子进程写入被 owner commit 接受、keeper 消费到 owned 像素；篡改 SHA256 负例被拒并退休。

## 5. 收尾
- [x] 5.1 `tests/registry.yaml` R01/R03/A01/S01 notes 补装配证据（状态不变）。
- [x] 5.2 docs/runtime.md（新增）、docs/media.md、ADR 0019、docs/development-plan.md、AGENTS.md 更新。
- [x] 5.3 运行 `cargo test --workspace --offline`、`cargo xtask test --suite contracts`、`check --registry`、`check-deps`、`check --profile portable`、`schema --check`、`pnpm -r typecheck`。
- [x] 5.4 写 `docs/reports/2026-10-09-w05-w06-composition-root.md`，归档并 `pnpm exec openspec validate --archived --no-interactive`。

## Workflow follow-up

- 主规格 `runtime-composition` 生效，`media-frame-slot-ownership` 增量生效。
- 后续：`apps/engine` 可执行 + loopback HTTP 控制服务启动、broker 在装配中接线、真实设备/窗口/音频与性能认证、PlaybackAck 生产端。
