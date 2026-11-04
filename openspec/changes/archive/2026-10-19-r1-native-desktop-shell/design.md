# Design

## Context

现有 src-tauri 配置声明了 main 窗口、最小权限、CSP 与两个真实 sidecar 的逻辑名。React 的 ControlClient 只支持 HTTP 测试适配。见 proposal.md。

## Goals / Non-Goals

目标：真实构建并加载现有控制台；独立锁文件和构建命令；缺前置响亮失败。
边界：不把窗口加载称为业务闭环、设备输出或 Windows 安装通过；本轮不启动 sidecar，不引入第二套业务逻辑。

## Decisions

1. src-tauri 使用独立 workspace，根 workspace 显式 exclude。拒绝将 Tauri 的原生工具链加入 portable/full；原生构建单独执行。依赖精确锁定 Tauri 2.12.4、tauri-build 2.7.3。
2. 继续消费现有 tauri.conf.json；前端使用 Vite 生产产物，不依赖开发服务器。构建前必须编译 engine/keeper；build.rs 只从显式侧车目录复制真实二进制到目标三元组文件名，不生成替身。该复制是打包准备，不构成启动或监督。
3. 原生壳不开放业务 invoke 命令。CSP 不放开 loopback；前端检测原生宿主后显示具名未接线提示，并禁用 HTTP 连接入口。浏览器测试适配继续工作。
4. --smoke 为显式加载演练：真实 WebView 完成页面导航后退出 0，20 秒未完成退出非零；不宣称 React 人工交互验收。普通运行维持窗口事件循环。无后台子进程，因此关窗不终止已有独立 engine。
5. 原生目标的依赖 BOM 和签名后续扩展，现有根 SBOM 仍只覆盖根 workspace，不把独立依赖闭包混入既有范围。

## Risks / Trade-offs

- 独立原生构建不在 full 内 → 本轮点名 cargo build/clippy 与 WebView 加载冒烟，报告分别记录。
- 原生界面暂不能控制引擎 → 明确提示、禁止尝试不受 CSP 支持的 HTTP 连接，不伪造在线状态。
- Windows 目标工具链与安装机未在本机验收 → A12/O01 保持原状态。
