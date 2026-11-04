# Design

## Context

现有配置声明 nsis/msi 但未构建；资源未打包、release defaults 为空。侧车 staging 已有目标三元组命名，安装态 engine_candidates 已支持与壳同目录的去三元组侧车。

## Decisions

- 首个可执行安装格式选择 Tauri NSIS/currentUser，不新增自制安装器。默认 WebView2 下载引导前置明确，断网缺 WebView2 不能视为可安装。
- `bundle.resources` 映射固定演示目录到 `demos/manual-demo/`；Rust 宿主用 resource_dir 作为首选，开发模式保留既有源码 fallback。WebView 不指定资源根。
- 新增资源根 builder，不改既有 HostConfig/Protobuf/OpenAPI。缺文件仍由原有设置/预检拒绝。安装目录只读，业务数据保持原 app_data/runtime。
- Windows 图标在既有原创像素构建过程中生成 ICO，复用 PNG payload，不下载素材。
- PowerShell 构建脚本仅在 Windows/MSVC 运行，固定 CLI 2.11.2（现有已安装版本）、Rust 由项目 rust-toolchain 锁定、cargo --locked；支持 Debug 工程与 Release。侧车目录按 root 目标缓存和实际 profile 推导并显式传递；前端只构建一次。
- 未签名构建必须显式工程选项，报告记录 Authenticode；正式发布要求已签名候选，不能把 SHA256 当签名。脚本不下载证书或含密钥。
- 工程候选使用 `nai Engineering` 产品名和独立 app identifier，安装与数据根均不覆盖正式产品。
- 同目标侧车及壳统一静态链接 MSVC CRT，避免把未声明的 VC++ runtime 变成安装前置；摘要只选择本次产生的安装器。
- NSIS 仅用于首次安装工程候选；程序升级与维护窗口仍属 nai-upgrade-core，本轮不声称已有升级集成。

## Risks / Trade-offs

Windows 工具链/NSIS 下载/WebView2 影响实测，失败保留原始日志与 Blocked，不能用 Linux 构建替代。当前付费 Provider 和设备输出未接齐，安装成功也不能宣称首句或 O01 完成。根 full 不含独立壳；分别验证。

## Verification

独立壳单元/真实 engine 集成与 clippy；资源目录在临时安装布局中测试，并经真实 engine 预检。尝试 Windows 原生编译和工程包构建；执行条件齐备时在隔离目录测试安装/卸载，保留数据。根 full 与 OpenSpec 校验/归档。大产物仅在忽略缓存，不提交。
