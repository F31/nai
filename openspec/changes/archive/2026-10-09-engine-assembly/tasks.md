# Tasks

## 1. 规格与校验
- [x] 1.1 proposal/design/tasks/specs 完成并通过 `pnpm exec openspec validate engine-assembly --strict --no-interactive`。

## 2. 共享会话
- [x] 2.1 `nai-control-api`：`State.session` 改 `Arc<Mutex<Session>>`，新增 `with_shared_session`，旧构造委托；锁忙 409。
- [x] 2.2 `nai-runtime`：`Composition` 持有共享会话并暴露 `shared_session()`；锁失败不阻塞已执行静音。
- [x] 2.3 测试：HTTP 撤权后装配侧租约签发读到新 epoch（分叉回归）。

## 3. apps/engine
- [x] 3.1 显式参数解析、token-file（0600）、`Composition::prepare` + `ControlApi::with_shared_session` + loopback `serve`；stdout 地址行。
- [x] 3.2 监督循环：spawn keeper、Rehearsal 租约 arm/续租、tick_duty、observe 故障处置；撤权后**停止续租且不自动重签**（`renewal_halted_after_revoke`），故障不自动重启输出。*勘误（ADR 0021）：原写“epoch 变更后显式重签”，与实际不符。*
- [x] 3.3 `--media-drill`（本轮边界）：引擎**进程内**建池/reserve/commit/`FrameConsumer` 读回+render-worker 预热；broker UDS 授权与 keeper 帧投递**未接线**（Linux，显式报错不降级）。*勘误（ADR 0021）：接线由后轮 `engine-broker-frame-delivery` 补齐。*
- [x] 3.4 停机（本轮边界）：`--halt-file` → 撤 token → 静音 → 回收子进程 → 停 HTTP → critical 日志。*勘误（ADR 0021）：原写 SIGINT/SIGTERM，实际因 `signal-hook-registry` 不在离线缓存且禁 `unsafe` 而用 halt 文件；被强杀不计优雅停机。*
- [x] 3.5 集成测试（真实子进程）：HTTP status/401/mute epoch 递增/决策日志落库；keeper 强杀→撤权 standby；停机路径。

## 4. 冒烟与收尾
- [x] 4.1 `scripts/engine-smoke.mjs` + `pnpm test:engine-smoke`：桌面 ControlClient 打真实 engine。
- [x] 4.2 registry S09/R01/A01/O02 notes 补证据（状态不变）；docs/engine.md（新增）、docs/control.md、ADR 0020、开发计划、AGENTS。
- [x] 4.3 全量验证：workspace 测试、check-deps、check --registry、schema --check、contracts suite、fmt、clippy、portable、前端 typecheck。
- [x] 4.4 报告 docs/reports/2026-10-09-engine-assembly.md，归档并 `validate --archived`。

## Workflow follow-up

- 主规格 `runtime-composition`、`local-control-surface` 增量生效。
- 后续：broker 连接对端认证（待工具链/依赖审查）、PlaybackAck 生产端、真实设备/Windows 认证、engine 生产参数化（部署 profile）。
