# Proposal

## Why

R1 顺序 3（人工辅助演示的「无命令行配置即一跑」）的仓内可验证一半尚未交付：演示包素材/场景只在测试里生成（`apps/engine/tests/common`），仓库里没有一份随源码携带的**合法演示包**；engine 也没有一个**一键预检**入口能回答「这台机器上这个演示包能不能跑」而不先启动会话。操作者目前只能靠肉眼读文档拼装参数。

## What Changes

- 新增随源码入仓的**人工辅助演示包** `demos/manual-demo/`：合法原创 2D 素材闭包（manifest/config/三张 RGBA，SHA-256 锁定，`nai-demo-license`），一份 schema 合法的 operator 场景，成本预估与连通性声明。
- 新增 `nai-engine --preflight <演示包目录或场景>` **一键预检**命令：只读地逐项检查场景 schema、知识、provider 预设与凭据、预算、素材准入与成本/连通声明；`passed`/`failed`/`not_ready` 三态各自具名，任一不确定即拒绝整体放行（沿 §12.2 与 nai-upgrade-core preflight），退出码非 0。**不** provisioning、**不**启动会话/服务、**不**在无凭据时发真实调用。
- 新增控制面 `GET /api/v1/sessions/{id}/preflight` 与桌面「预检」面板：返回最近一次预检报告，未跑过则 `CAPABILITY_NOT_READY`；界面诚实标注未播放、未接设备。
- 演示包与预检命令用进程级测试验证：合法包整体 passed、缺凭据 → `not_ready`、改素材 SHA-256 → `failed`、无 render → `not_ready`、决策链无 PlaybackAck。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `demo-package`: 演示包从「构造出的判定机制」补上**仓内真实载荷与一键预检入口**（O01 仍 pending：真实人工演练与真实端点仍外部）。
- `local-control-surface`: 新增预检报告读取路由与桌面呈现。

## Impact

新增 `demos/manual-demo/`（素材/场景/许可/说明）、`apps/engine` 的 `--preflight` 命令与预检模块、`nai-control-api` 的 `GET /sessions/{id}/preflight`。复用既有 operator 场景 schema、`resolve_render_setup`/`render_lane` 素材准入与 `nai-kernel::demo` 口径。零新增依赖；不改验收状态（O01/O05 仍 pending）。不作设备/真实付费/Tauri 壳认证。