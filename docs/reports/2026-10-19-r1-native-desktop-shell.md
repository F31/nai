# R1 原生桌面壳：真实构建与内嵌页面加载

日期：2026-10-19。变更已归档至 `openspec/changes/archive/2026-10-19-r1-native-desktop-shell/`。
依据：[ADR 0129](../adr/0129-native-shell-build-isolation.md)。

## 现在能做什么

- 从独立锁文件构建真实 Tauri 2 壳，内嵌当前 Vite 生产控制台，不需运行开发服务器。
- 打开原生窗口；显式 `--smoke` 在 WebView 完成页面导航后退出 0，20 秒未加载退出 1。
- 原生页面明确说明受保护 engine IPC 未接线并禁用 HTTP 连接，浏览器测试适配继续可用。
- 构建从真实 engine/keeper 二进制准备目标三元组侧车文件，缺文件具名失败；尚不启动这些进程。

## 实现

`apps/desktop/src-tauri/` 添加独立 Cargo workspace、锁文件、build.rs、Rust 入口。
锁定 Tauri 2.12.4、tauri-build 2.7.3、png 0.18.1（用于源代码生成原创 MIT 图标）。
根 workspace 显式 exclude 原生壳；现有最小权限、CSP、sidecar 声明均保持原样。
独立锁文件解析 407 个包（含壳自身），不是根 workspace 的一部分；根 SBOM 不含该闭包。
壳没有 invoke handler、没有业务语义、没有子进程。数据根交 OS API 解析，未创建或访问业务数据。

## 实测

- GTK 3.24.41、WebKitGTK 2.52.6；Rust 1.96.0、Node 22.23.1。
- 前端 `pnpm --filter @nai/desktop build`：通过（38 modules）。
- `cargo build -p nai-engine -p nai-output-keeper --locked`：通过。
- 独立 `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --locked`：通过。
- 独立 clippy `--locked --all-targets -- -D warnings`：通过。
- `timeout 35s xvfb-run -a .../nai-desktop-shell --smoke`：退出 0，输出
  `SHELL_PAGE_LOADED: embedded console; engine_ipc=false; devices=false`。
  软件显示环境有 EGL/Mesa 驱动警告，但页面完成导航；未把这些警告隐去。
- 负向实测：`NAI_SIDECAR_DIR=/tmp/opencode/no-sidecars` 的独立检查以 `SIDECAR_MISSING` 失败，
  指名缺失的 engine 二进制，没有用旧复制件或占位件放行。
- 既有桌面壳边界检查：15/15 通过。
- 初次构建暴露默认图标缺失，补源代码生成的原创图标后重建成功；一次 240 秒工具超时后续跑完成，
  不把中断当作编译失败或成功证据。

根 `cargo xtask check --profile full` 已完整退出 0，最后输出完整工程基础通过；前端 UI 95/95、桌面客户端 5/5。
`pnpm exec openspec validate --all --strict`：43 份主规格与本轮变更通过。
首次 full 调用与 `git add -N` 合并且输出重定向，触及工具 900 秒超时；日志表明 Git 早已完成，
已跑完 Rust，停在前端 build。随后前端单跑通过，再独立运行 full 完整通过。
未把这次超时归为代码缺陷，也未把它记为全量检查通过；没有运行中的残留检查进程。
归档后针对变更面复核：43 份主规格、156 份归档变更严格校验通过；spec-merge、archived-proposal、
doc-links 三套件 30/30 通过，`git diff --check` 无错误。归档没有修改实现代码，未重复整套编译。

## 未实现与未验收

受保护 engine IPC、启动/监督、托盘、安装后配置向导尚未实现，属于仓内任务。
Windows 安装、WebView2、设备、签名与分发未验收。加载冒烟不验证 React 交互，不构成 O01 首句演练。
本轮无付费 API 调用、无设备输出、无 PlaybackAck。A12/O01 及其他验收状态不升级。
更完整的操作步骤见 [原生壳 README](../../apps/desktop/src-tauri/README.md)。
