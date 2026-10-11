## Why

基线把**宿主壳**写成了明文要求，而仓库里**一处落点都没有**，且**没有任何 ADR 记录过这个决策**：

- §1248：「**技术栈**：Rust + Tokio + serde + tracing；按需 axum 与 reqwest；**Tauri 2 + React + TypeScript + Vite**……」
- §1260：「**产物**：core-runtime……；**desktop-shell**；media-base……」
- §1151：「Tauri 的 **capabilities 与 CSP 只开放必要的命令与资源**；前端没有任意 shell、文件系统、SQL 权限……」
- §1259：「Tauri **sidecar 按目标三元组命名**并打包。默认的 UI 父子生命周期要显式处理：需要在 UI 关闭后继续运行的引擎与 keeper
  由独立 supervisor 或托盘管理，**不依赖 WebView 保活**，也不开放任意 shell。」
- §900：「数据根使用系统用户数据目录（**由 Tauri 或操作系统路径 API 解析**），安装目录只读。」
- §1101：「**同机 Tauri 优先使用受保护的 IPC**；HTTP 只用于已启用的手机、轻云与测试适配……」

对照现状：`docs/adr/` 的 **44 篇里没有一篇**裁决桌面壳形态；`apps/desktop` 是**纯 Web**
（deps 只有 `react`/`react-dom`/`vite`，无 `src-tauri/`）；上述四组条款（最小权限与 CSP、sidecar 与生命周期、数据根、受保护 IPC）
在 `crates/`、`apps/`、`schemas/` 里**零命中**；`tests/registry.yaml` 的 **66 条里没有桌面壳条目**。
唯一已就位的是 INV-18「内核不依赖 Tauri」——它**已经**由 `xtask/src/deps.rs`（`tauri` 前缀检查 + 单测夹具）强制。

⇒ **缺的不是选型（形状已定），而是「可判定的边界」与「可查的落点」**。没有它们，"壳做完了没"无法判真伪，
任何一份配置文件都能自称壳。本轮先把这一层解决。

## What Changes

- **ADR 0046** 裁决桌面壳形态与边界：形态（**Tauri 2**，§1248）、壳只做四件事（窗口/托盘与生命周期、受保护 IPC 的宿主侧、
  数据根解析、sidecar 拉起与监督）、IPC 双通道（受保护 IPC = 生产路径，loopback HTTP = 测试适配，§1101）、
  capabilities/CSP 最小权限、生命周期与 WebView 保活解耦与 sidecar 三元组命名（§1259）、数据根（§900）、
  打包与签名、**门控与验收不升级**，并**一次裁决三处**（桌面 + macOS + 移动端路线 B 共用同一套边界，后两者只写边界与前置）。
- **声明式落点**（Tauri 2 的**真实**格式，非伪造）：`apps/desktop/src-tauri/tauri.conf.json` 与
  `apps/desktop/src-tauri/capabilities/default.json`。**不引入 `tauri` crate、不建 Rust 主体、不进 workspace members**。
- **校验**（Node 侧，零新增依赖，进既有 `test:ui` 门控）：权限集合**逐一相等**、无 shell/fs/sql/process、
  CSP 无 `unsafe-eval` 与通配、连接源只含受保护 IPC、sidecar 带目标三元组、无硬编码绝对路径、前端不 import `@tauri-apps/*`；
  并带**非空洞守卫**（注入 `shell:allow-execute`、`unsafe-eval`、`@tauri-apps/plugin-fs` 三种样本，检查必须全部抓到）。
- **registry**：新增 **A12「桌面壳宿主边界」**（`status: pending`，notes 记本轮落点与缺失前置）。
- **文档**：`docs/control.md`、`docs/ui.md` 就地订正；`apps/desktop/src/main.tsx` 页脚那句「Tauri/受保护 IPC 尚未接线」改准确。
- **不做**：不引入 Tauri 依赖（会让门控依赖 WebView2 与网络，ADR 0041 已把这类「主机/网络事实混进门控结论」列为禁忌）；
  不建 Rust 壳主体、不打包、不签名；workspace 保持 19 包、零新增依赖、零 Rust 改动；不碰内核一行；不升级任何验收 `status`。

## Capabilities

### Added Capabilities

- `desktop-shell`：宿主壳的范围边界（只做四件事）、最小权限面与其「逐一相等」判定、CSP 与连接源、
  受保护 IPC 与测试适配的分工、UI 生命周期与后台进程解耦、sidecar 目标三元组命名、数据根解析、
  桌面/macOS/移动端共用边界，以及「配置落地 ≠ 壳已实现」。

## Impact

- 新增：`docs/adr/0046-desktop-shell-form-and-tauri-boundary.md`、
  `apps/desktop/src-tauri/{tauri.conf.json,capabilities/default.json}`、
  `tests/ui/desktop-shell.test.mjs`、`docs/reports/2026-10-11-w54-desktop-shell-boundary.md`。
- 修改：`tests/registry.yaml`（新增 A12）、`docs/control.md`、`docs/ui.md`、`apps/desktop/src/main.tsx`、
  `AGENTS.md`、`docs/development-plan.md`、`docs/engineering.md`（如需）。
- **依赖与包数不变**：workspace 仍 **19 包**，`check-deps` 仍是 19，**新增外部依赖为零**，**Rust 侧零改动**。
- **已知后果（如实登记）**：`tauri.conf.json` 的 `$schema` 指向官方 URL，**离线不可校验**，故本仓库的校验**自建**、
  不依赖网络；当 Tauri 升级到改动了配置字段的大版本时，配置可能漂移而**本仓库的检查不会自动发现**——
  这是「不引入真实依赖」这一取舍的代价，已写入 ADR 的「已知后果」与 A12 的 notes。
- **未做边界**：壳未构建（无 `tauri` crate、无 Rust 主体、无打包与签名链）⇒ A12 保持 `pending`；
  macOS 与移动端路线 B 只登记边界，不实现；真实 WebView2 构建、安装器与 Windows 代码签名属发布里程碑。
- **验证**：`cargo xtask check --profile full`（工程门控 + 前端门控，含新的 `tests/ui`）、`openspec validate --all --strict --archived`；
  推送后匿名复核 CI 的两平台结论。
