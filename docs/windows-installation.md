# Windows 安装候选构建与验证

本路径生成首次安装候选，不代替设备、真实 Provider、签名链或发布认证。
实际入口是 `scripts/package-windows.ps1`；未改变尚未实现的 `cargo xtask package/certify`，这些命令仍明确非零退出。
采用 [Tauri 官方 NSIS 路径](https://v2.tauri.app/distribute/windows-installer/)，资源映射遵循
[官方 resources 机制](https://v2.tauri.app/develop/resources/)。边界见 [ADR 0132](adr/0132-windows-installer-candidate-boundary.md)。

## 构建前置

- Windows x64、项目锁定 Rust/MSVC、Windows SDK，CMake/NASM（Provider TLS 依赖）。
- PowerShell 7、Node/pnpm 与 package.json 锁定版本，已安装前端依赖。
- `cargo-tauri` **2.11.2**：在构建环境用 `cargo install tauri-cli --version 2.11.2 --locked` 安装；不进入产品运行依赖。
- NSIS 由 Tauri 构建工具准备，首次下载需要网络；WebView2 下载引导也需要网络。
- 工程候选可未签名；正式候选必须配置 Windows 签名且校验有效。证书与私钥不入库。

在 MSVC Developer PowerShell 中，从仓库根运行：

```powershell
pwsh -NoProfile -File scripts/package-windows.ps1 -Configuration Debug -UnsignedEngineering
```

默认不带 `-UnsignedEngineering` 时要求签名配置，避免把未签名产物误当正式发布。
`-Configuration Release -UnsignedEngineering` 可生成优化的工程候选，仍不是签名发布。
分离 Windows/WSL 的开发机器可先在同一 checkout 运行前端 gate，再显式加 `-FrontendPrepared`；
摘要会记录该事实，脚本只验证现有前端产物存在，不假称自己重跑了前端检查。

脚本先构建真实 engine/keeper 并检查 AMD64 PE，再用同目标目录构建壳及当前用户 NSIS 安装器。
同目标侧车及壳显式静态链接 MSVC CRT；本机 dumpbin 未发现额外 VC++ runtime DLL。
每一步最多 30 分钟，失败或超时不写成功摘要；日志在 `.nai-diagnostics/windows-package/`。
二进制在忽略的 `target-windows/`，不提交。
配置或安装 hook 在构建期间变化会拒绝该候选，生成的 NSIS 脚本必须实际引用首次安装 hook。

## 包与首次运行

工程候选显示为 **nai Engineering**，独立 identifier 为 `cn.nai.desktop-shell.engineering`，
不与正式 `cn.nai.desktop-shell` 共用安装记录或应用数据路径。摘要包含实际安装器路径、大小、
SHA256、Authenticode 状态及 `release_certified=false`。SHA256 只是内容标识，不是来源证明。

安装包包含原生壳、同目标 engine/keeper、原创图标与 `demos/manual-demo/` 场景、素材及许可。
安装后双击壳即可看到设置面板；默认场景与 runtime profile 从宿主资源目录解析，无需源码目录。
已保存的自选设置优先；安装资源只读，配置、数据库与预检报告写应用数据目录。
应用启动不会自动派生 engine。选择初始化并“启动并连接”仍是显式操作。

随包 Provider 端点/模型/声线仍是演示占位，示例知识 local_only=true；未配置实际 API 不会伪造首句。
可以选择自己的合法场景文件并输入本次会话密钥，然后运行预检。该包尚未提供设备播放。

## 安装/卸载验证

在受控 Windows 测试机首次安装工程候选，记录：系统、WebView2 版本、包哈希、签名状态、安装路径，
打开设置、资源文件存在、同目录侧车、初始化/连接/预检；然后从托盘退出，确认无残余 engine/keeper。
卸载后核对程序已移除、用户配置/事实/审计未被静默清除。安装时缺 WebView2 且网络不可用应记 Blocked。

安装 hook 会拒绝已有安装（包含更换目标目录），拒绝活动 engine/keeper/壳及进程核验失败，不强杀。
不要用当前 NSIS 候选覆盖活动会话或把它当升级器。生产升级须先接通维护窗口、备份、journal、
兼容范围和崩溃恢复；该安装包没有完成该链路。手机/平台/媒体认证与本节分别验收。

可在专用测试账号运行真实烟测：

```powershell
pwsh -NoProfile -File scripts/test-windows-install.ps1 -Installer '<工程 setup.exe 绝对路径>'
```

烟测会拒绝既有工程安装/数据，安装目标固定为本工作区 `target-windows/install-smoke`。
验证活动 keeper 不被安装器强杀、首次安装、随包资源/真实 engine 预检、WebView 加载、同目录和异目录拒绝，
最后卸载并逐文件核验真实初始化数据哈希。执行时 PATH 仅含 Windows 系统目录，无 Node/Python 前置。
卸载后保留工程应用数据，不自动抹除。专用测试账号在保留本测试数据后重跑，可显式选择
`-PreserveExistingTestData`；这会复用而不重新 provision 现有数据库，摘要如实记 `provisioned_this_run=false`。
测试采用 `_?=` 直接等待卸载，可能在专用安装缓存留下 `uninstall.exe`；下轮需先清理该缓存目录，不能删除应用数据来让测试重跑。

本机逐项实测与缺口见 [本轮报告](reports/2026-10-20-r1-windows-installer.md)。

## 2026-10-20 黑屏修复后的界面验证

旧工程包可能因重复React实例导致useState异常、深色背景但无界面。已用Vite解析去重修复；与代理无关，不需修改CSP。新的--smoke必须收到DOM挂载及主机配置区显示后的SHELL_UI_READY才成功，导航完成不再算界面通过。用户已有安装须退出壳/托盘和engine，再卸载旧程序并安装新候选；保留应用数据与已保存凭据。见[修复报告](reports/2026-10-20-r1-react-bundle-startup.md)。
