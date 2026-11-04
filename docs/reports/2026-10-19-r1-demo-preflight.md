# 演示包 + 一键预检（顺序 3 仓内可验证部分）

日期：2026-10-19。变更：`openspec/changes/archive/2026-10-19-r1-demo-preflight/`。

## 本轮交付的用户路径

操作者拿到 `demos/manual-demo/` 后，不启动会话即可先回答「这台机器上这个演示包能不能跑」：

```bash
NAI_DEMO_OPERATOR_KEY=... nai-engine --data-root <dir> --workspace <id> --actor <owner> --profile <name> \
  --preflight demos/manual-demo/manual-session.json
```

每轴一个 `passed` / `failed` / `not_ready` 与原因；任一 non-pass 整体拒绝、退出码非 0。运行后桌面
「演示包一键预检」面板可读回报告。

## 演示包（随源码入仓）

`demos/manual-demo/`：

- `avatar/manifest.json`、`config.json`、`base.rgba`/`closed.rgba`/`open.rgba`：**仓库作者原创**的
  2×1 / 1×1 RGBA 占位图，manifest 锁定三张素材的 SHA-256，通过 `schemas/plugin/avatar` 与
  `schemas/config/avatar-2d` 校验。
- `LICENSE`：`nai-demo-license` 许可引用的落点，明确这是**演练素材**，不构成第三方形象/品牌/重量级素材授权。
- `manual-session.json`：schema 合法的 operator 场景（provider 预设 + `NAI_DEMO_OPERATOR_KEY` 凭据引用
  + 知识种子 + prompt + budget + render 闭包）。
- `README.md`：预检与真实行使的说明。

**它不是真实主播形象**。README/LICENSE 写明：切换真实演示形象须替换许可与 SHA-256。

## 一键预检命令

`nai-engine --preflight <scene>` 进 `run()` 前分支，与 `--provision`/`--operator-config`/media/event 模式
**互斥**：

- **只读**：不 provisioning、不建会话、不起服务；
- **复用运行期同一入口**：场景 schema（`operator::load`）、provider 构造（抽成共享的
  `operator::build_provider`）、素材准入（`resolve_render_setup` + `render_lane` 的 SHA-256/许可/三资产
  闭包）；
- **逐轴三态**：`scene`/`knowledge`/`provider`/`budget`/`render`/`connectivity`；
- **拒绝于不确定**：缺凭据 ⇒ `provider=not_ready`（点名环境变量）；无 render ⇒ `render=not_ready`；
  SHA-256 与素材不符 ⇒ `render=failed` 并指名；任一 non-pass ⇒ 整体拒绝、退出码非 0；
- **连通探针可选**：`--preflight-connectivity` 且凭据已设置才发一条受预算约束的真实文本请求（
  `InMemoryGateway` + 真实适配器 + 有界 budget），结果只进报告，不记录为验收或来源 eligibility；
  未设置凭据/未选探针 ⇒ `connectivity=not_ready`（探针未发），MUST NOT 跳过来冒充通过；
- 报告原子写入 `<data-root>/operator-artifacts/preflight.json`，`playback_ack` 恒 `false`。

## 控制面与桌面

- `GET /api/v1/sessions/{id}/preflight`（`session.read`）：读回最近预检报告；未运行过或读取失败 ⇒
  **`CAPABILITY_NOT_READY`（503）**，不伪造空报告；
- 桌面新增「演示包一键预检」面板（`apps/desktop/src/preflight.ts` + `control.ts` 的 `PreflightView`/
  `parsePreflight` + `main.tsx`）：逐轴原样呈现，`preflightSummary` 计数轴并始终带「未播放、未接设备」；
  未预检时如实显示「尚未运行 `nai-engine --preflight <scene>`」，而不是绿灯。

## 验证

本机（Linux/WSL）实测：

- `cargo test -p nai-engine --test engine operator_flow` **13/13**（新增 5 项 preflight 进程级测试）：
  缺凭据整体拒绝并点名变量、SHA 改动 `failed`、无 render `not_ready`、合法场景 + 本地协议 fixture +
  探针 = `overall pass`（fixture 实际收到一次 chat 调用）、控制路由读回报告 / 未跑=503、与 operator-config
  组合被拒；
- `node --test tests/ui/preflight.test.mjs` **6/6**；
- `cargo xtask schema` 无漂移（含新增 `GET /sessions/{id}/preflight` 路由）；
- `cargo fmt --all`、`cargo clippy -p nai-engine -p nai-control-api --all-targets -- -D warnings` 干净；
- 前端 gate 六步通过、UI 套件 95 项（含本轮 6）；
- 真实引擎进程以 `demos/manual-demo/manual-session.json` **provision 成功**（建工作区、导入知识、建会话、
  准入 render），`--preflight` 对随源码包逐轴 `PASS`（scene/knowledge/provider/budget/render），
  connectivity 按设计 `not_ready`/探针结果如实回报；
- `cargo xtask check --profile full` **EXIT=0**：**102 测试目标（84 Running + 18 Doc-tests）、1201 passed / 0 failed**；Node **438 项 / 0 失败**（UI 95 含本轮 6）。变更归档为 `openspec/changes/archive/2026-10-19-r1-demo-preflight/`，归档后复跑同一命令仍 EXIT=0、计数逐项相同。

## 边界与未做

- **O01 保持 `pending`**：仓内演示包与预检是「判据+载荷+入口」，真实形象/知识正文/真实连通/人工演练仍外部。
- **无设备/窗口/播放**：没有 TTS 播放、2D 窗口、PlaybackAck、帧槽或 keeper 消费；报告与界面四处标注。
- 连通探针在无真实付费凭据时只会 `not_ready`（探针未发），不会假装放行；受预算探针结果不升级任何验收。
- Windows 安装/Tauri 壳/签名分发仍为下一步，未在本轮。
- 素材是演练占位图，不是真实授权形象；预检通过不等于可以播出。