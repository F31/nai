# Proposal: engine-assembly

## Why

`nai-runtime` 装配根与 `nai-control-api` 的 loopback 控制面都已实现并各自实测，但仓库仍没有可启动的
引擎进程：控制服务只由 `nai-control-api` 的 example 二进制启动（独立 Session、无存储/决策日志/值守/
监督），keeper/render-worker 只在测试中由 Supervisor 驱动。V3.6 要求单一 composition root 装配整体；
本轮把它落成真实可执行程序。

## What

- 共享会话：`nai-control-api` 增加共享 Session 构造（`Arc<Mutex<Session>>`），`nai-runtime` 的
  `Composition` 改为持有共享会话并暴露给控制面，消除「HTTP 撤权与监督/租约 epoch 各自为政」的分叉。
- `apps/engine`（layer `app`）：显式参数装配 `Composition`、启动 loopback 控制服务（`serve`），
  可选监督 keeper（arm/续租/值守 tick/故障处置），可选 `--media-drill`（引擎内建池/commit/读回 +
  render-worker；broker UDS 授权与 keeper 帧投递本轮未接线，见 ADR 0021 勘误）。
  优雅停机：`--halt-file` 触发撤 token、静音、回收子进程、停 HTTP。
- 端到端冒烟：`scripts/engine-smoke.mjs` 用桌面 `ControlClient` 打真实 engine 进程。
- 明确不做：生产 protected IPC、broker 连接对端认证（ADR 0018）、PlaybackAck 生产端、
  真实窗口/音频设备、Windows 认证。

## Impact

- 规格：`runtime-composition`（增量）、`local-control-surface`（增量）。
- 代码：`crates/nai-control-api`、`crates/nai-runtime`、新建 `apps/engine`、`scripts/engine-smoke.mjs`。
- 验收：`tests/registry.yaml` S09/R01/A01/O02 notes 补证据（状态不变，不标 tested）。
- 文档：docs/engine.md（新增）、docs/control.md、ADR 0020、开发计划、AGENTS、报告。
