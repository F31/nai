# R1 Windows 安装候选与安装态资源

日期：2026-10-20。W09/W10、R1 顺序 3（D14/D19 仅追溯）。依据 V3.11 §15、§28.6、§29.5 与 ADR 0132。
OpenSpec 已归档为 `openspec/changes/archive/2026-10-20-r1-windows-installer/`，desktop-shell 合入 2 条需求。

## 实现

- Tauri NSIS/currentUser，明确 WebView2 下载引导；随包真实 engine/keeper、演示资源与许可。
- 原创图标在 build.rs 同时生成 PNG/ICO；无下载素材与新运行依赖。
- ShellState 接收仅由宿主提供的 resource_dir；安装态默认场景不依赖源码路径，已有配置优先。
  安装目录只读，业务数据仍在应用数据目录，缺资源不会回退到另一份源码数据冒充完整安装。
- PowerShell 7 构建入口：锁定 Tauri CLI 2.11.2、cargo --locked、MSVC 主机及 AMD64 PE 核验、
  每步 30 分钟期限、签名状态/哈希摘要、失败不保留旧成功摘要。
- 首次安装 hook 在维护选择页前拒绝已有安装，静默模式在写文件前复检；同目录/异目录覆盖和活动进程
  均拒绝，不强杀。配置与 hook 哈希跨构建不变且生成 NSIS 必须引用 hook，防止并行编辑形成旧配置候选。
- 未签名候选必须显式选择，产品名/identifier 隔离为 nai Engineering，正式候选要求签名配置及有效签名。
- 操作说明见 [Windows 安装指南](../windows-installation.md)。

## 实测

- WSL Rust 1.96.0：独立壳 lib **11/11**，集成 **5/5**；其中新增安装布局场景复制、实际 engine
  预检 scene/render=passed、数据根独立、已保存配置优先与缺资源具名拒绝。clippy 干净。
- 实际 Tauri 构建已接受 NSIS/资源/双图标配置；Linux 证据不替代 Windows 安装。
- Windows MSVC 原生 `cargo build -p nai-engine -p nai-output-keeper --locked --offline -j 2`
  成功，耗时 **2m56s**。产物位于 `target-windows/debug/`，不是 WSL ELF 替身。
- 从官方 Tauri release 获取 Windows CLI 2.11.2，下载 ZIP 与官方资产 SHA256
  `b6844470bcbf1da6e5dbf01990ae317d4d7969171628bb8badbdbff2e3d06d23` 一致，版本实测相符。
- Windows **10.0.26200.0**、MSVC **14.44.35207** / SDK **10.0.26100.0**，Rust **1.96.0**：
  显式静态 CRT 的侧车重编译 **2m52s**；原生壳 **1m27s**，最终增量构建约 4s。
  实际 NSIS **3.11** 生成未签名 Debug `nai Engineering_0.1.0_x64-setup.exe`，**8,284,500 bytes**。
  SHA256：`11dbfd3285c6171af2bbff3d0cd973687b8208bec761f3462558c2c977534e04`；Authenticode **NotSigned**。
  产物：`target-windows/shell/x86_64-pc-windows-msvc/debug/bundle/nsis/`，不入库。
- Windows 完整真实烟测 **EXIT=0**：活动 keeper 保持存活（由测试自行回收）；首次安装；实际资源与许可存在；
  installed engine 预检 scene/render=passed、provider/connectivity=not_ready、all_passed/playback_ack=false；
  原生 WebView `SHELL_PAGE_LOADED`；同目录及异目录覆盖各拒绝；卸载移除程序/安装记录。
  初次运行实际 provision 了 SQLite；最终完整复跑使用 `-PreserveExistingTestData`，不重新 provision，
  **3 个初始化数据文件逐字节哈希保持一致**。摘要如实记 `provisioned_this_run=false`，不冒称重建过数据。
- 烟测进程 PATH 仅含 Windows 系统目录，实际 engine 与原生壳均可运行；dumpbin 依赖未发现额外
  VCRUNTIME/MSVCP DLL。当前仍是开发机器，不是干净 Windows 机器矩阵认证。
- `cargo xtask check --profile full` **EXIT=0**：Rust **1209 passed/0 failed**，Node **451/0**。
  Node 较上轮 +1 是平台扫描排除 `target-windows` 构建缓存的非空洞测试，真实 src 仍必须被扫描。
  独立壳仍单独记录 11+5，不并入根门控计数。
- 最终构建/烟测摘要在 `.nai-diagnostics/windows-package/summary.json` 与
  `.nai-diagnostics/windows-install-smoke/summary.json`；实际预检/SQLite 位于工程 app identifier 的用户数据目录，保留未删除。
- 归档后严格主规格 **43/43**、归档 **161/161**，规格合入/proposal/文档链接定向回归 **30/30**；
  最终凭据扫描通过（1584 个将发布路径、26 个诊断文本）。本轮改动定向 diff 检查和两个 PowerShell 脚本语法检查通过。

## 失败、定位与修复

- 首轮 Windows 壳构建在工程标识与日志策略调整时主动停止，保留缓存后重建；不记为通过。
- PowerShell 流复制 Task 的 VoidTaskResult 被意外输出为返回值，导致工具链版本检查误判；显式吞掉该返回值后修复。
- 首个安装器并行构建在 hook 配置更新前约 4 秒读取旧配置，活动进程测试发现它未包含 hook。
  卸载这个纯测试安装后，增加配置/hash 不变和生成脚本含 hook 的硬检查，重新打包。
- 烟测最初误以为 keeper 关闭 stdin 应退出（它按设计保持静音待机），以及预检拒绝退出码为 1
  （engine main 固定为 2）；修正测试假设，不改运行时行为。测试主动回收自己的无租约 keeper，不把它算作安装器强杀。
- 异目录覆盖测试发现 NSIS 内置名称引用写成 `$^Name`，导致注册表查找未命中；修为 `$(^Name)` 后，
  最终完整烟测同目录与异目录均拒绝。两个旧测试安装经自带卸载器移除，初始化数据库与报告哈希保留。
- 手工诊断曾把安装壳与构建目录整文件哈希比较：只差 Tauri 的 3 字节 bundle-kind `UNK→NSS`
  补丁；不能将这个正常的安装标记差异误报成包被篡改。
- 首轮 full 的凭据扫描命中已校验官方 CLI 二进制中的凭据样式串：工具误放在诊断日志域。
  工具移到忽略的 `target-windows/tools/`，**不修改扫描规则/豁免**；原扫描与完整 full 随后均通过。
- Tauri CLI 发出 STATIC_VCRUNTIME 弃用提示，保留记录；原生构建仍成功。Windows 页面退出有一次
  Chrome_WidgetWin_0/1412 清理警告，但加载标志和退出码均正确；未据此声称人工桌面交互认证。

## 边界

尚无签名凭据与独立壳供应链审计；工程包不是正式发布。升级 journal/维护窗口未接安装器，不用于生产升级。
真实付费 Provider、设备窗口/音轨、硬件 PlaybackAck、命名管道 ACL/对端认证仍未完成，A12/O01 不升级。
随包场景仍是明确占位与 local_only 示例，不把安装成功当作 API/首句成功。
Windows 安装与页面加载已在本机实测；干净机、WebView2 缺失/离线矩阵、托盘人工交互与签名分发未验收。
