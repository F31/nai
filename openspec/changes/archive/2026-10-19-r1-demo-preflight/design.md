# Design

## Context

R1 顺序 3 的仓内可验证一半：合法可演练的演示包 + 一键预检。`demos/manual-demo/` 里要有真实素材/场景
（不能捏造），`nai-engine` 要有只读预检入口（不能靠人读文档拼参数）。现有可复用件：operator 场景
schema（`schemas/operator/manual-session.schema.json`）、`apps/engine/src/pipeline.rs` 的
`resolve_render_setup`/`render_lane`（做素材 SHA-256/许可/闭包准入）、`nai-kernel::demo` 的成本/连通口径、
`nai-control-api` 的会话路由模式。

## Goals / Non-Goals

交付：随源码的演示包；`--preflight` 一键预检（逐轴三态、拒绝于不确定、可选的受预算连通探针）；
控制面 `GET /sessions/{id}/preflight` 与桌面呈现。不装安装器、不接设备输出、不产生 PlaybackAck、
不升级 O01/O05 状态、不改 Tauri/Windows 面。

## Decisions

1. **演示包是目录+场景，素材是真实原创占位图。** `demos/manual-demo/avatar/*.rgba` 为 2×1/1×1 RGBA
   原创占位（本仓库作者持有），manifest 锁定 SHA-256，`nai-demo-license` 指向随包 `LICENSE`。它**不是**
   真实主播形象或品牌素材；README/LICENSE 写明切换真实形象须替换许可与 SHA-256。这样既满足「合法授权」，
   又不冒充外部素材。
2. **`--preflight <场景>` 是显式一次命令，进 `run()` 之前的分支。** 与 `--provision` 同层：读入参数后
   先判断 `--preflight`，只读检查后打印逐轴 JSON + 退出码，**不** provisioning、不建会话、不起 `serve`。
   复用 `operator::load`（schema + 知识唯一非空）与 `pipeline::preflight_render`（素材准入）。为避免
   `render_lane` 把素材读进媒体态，预检只做准入验证、不构建 `RenderLane`/`RehearsalSink` 播放态——但
   `resolve_render_setup` 已经把路径解析做实。给出 `--preflight` 时若同时给了 `--provision`/`--keeper`
   等运行时参数则**具名拒绝**（与 `--provision` 互斥一致）。
3. **逐轴三态与「未知=拒绝」。** 检查轴：`scene`、`knowledge`、`provider`、`budget`、`render`、
   `cost`、`connectivity`。`credential_env` 未设置 ⇒ `provider=not_ready`（点名变量名）；渲染缺 render
   ⇒ `render=not_ready`；SHA-256 不符或素材缺 ⇒ `render=failed` 并指名。任一 `not_ready`/`failed` ⇒ 整体
   拒绝、退出码 1。这是沿 §12.2「未知与不足同等拒绝」与 `nai-upgrade-core::preflight` 的口径。
4. **连通探针是显式可选的受预算真实调用，不是默认动作。** `--preflight --connectivity` 且凭据已设置时，
   通过既有 `ProviderProfile → ProviderClient → 适配器` 走一发极小的 text/tts 请求（
   `InMemoryGateway` + 有界预算），分别报告端点/凭据/能力；未设置凭据或无该标志 ⇒ `connectivity=not_ready`，
   说明「探针未发」。探针结果只进预检报告，不写 decision log 的 playback 类，不冒充 eligibility。
5. **报告写入 `operator-artifacts/preflight.json`（原子写）。** `--preflight` 需要 `--data-root`；报告写到
   `<data-root>/operator-artifacts/preflight.json`，控制路由读同一路径。未跑过 ⇒ `CAPABILITY_NOT_READY`。
6. **控制路由 `GET /sessions/{id}/preflight`。** 复用会话路由与 `session.read` 权限；`OperatorLane` 不持有
   报告，由 engine 装配时注入一个读取器（或 engine 直接读文件返回 JSON）。为最小接线成本，采用
   engine 在装配时把 preflight 报告路径交给 controller 的简单读文件实现，route 只做有界读 + 严格解析 +
   `playback_ack:false` 标注。
7. **桌面「预检」面板。** 新增 `apps/desktop/src/prefight`? 统一命名 `preflight.ts`（与 `preview.ts` 同形，
   零 DOM）：`parsePreflightReport` 逐轴解析、`PREFLIGHT_NOT_PLAYBACK` 标注常量；`main.tsx` 加一个只读
   面板与重读按钮。`tests/ui/preflight.test.mjs` 覆盖«成功»与«未运行»两态与诚实标注。

## Risks / Trade-offs

- 演示包素材是占位图，可能被误读为「这就是形象」。README/LICENSE 与预检报告都写明是演练素材。
- 预检的「连通」只在有凭据+显式标志时发真实调用；无凭据时必然 not_ready → 整体拒绝。这是**诚实**的，
  不是缺陷；退出码不会假装放行。
- 控制路由读文件有界；不做目录列举。

## Migration

无数据库迁移；不改 schema 常量；`db_schema_version` 不变。

## Verification

`cargo test -p nai-engine --test engine demo_preflight`（合法包 passed / 缺凭据 not_ready / SHA 不动改
failed / 无 render not_ready / 探针声明）；`cargo test -p nai-control-api` / `-p nai-storage` 回归；
`cargo xtask check --profile full`、`schema --check`、`validate --all --strict`；`tests/ui/preflight.test.mjs`；
报告写明未播放。O01/O05 状态不动。