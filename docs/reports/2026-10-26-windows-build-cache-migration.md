# Windows 构建缓存迁移

日期：2026-10-26。状态：缓存迁移已完成，迁移后原生构建、定向测试、桌面构建和启动冒烟已通过。

按用户要求，Windows 构建、临时文件及包缓存统一放在 `E:\projects\nai\target-windows`。本轮三个受保护目录为 `live-auto-build`、`live-auto-runtime`、`tool-cache`；用户已安排其他构建和清理任务避开它们。

迁移逐文件核验 SHA256：35,629 个文件、1,351,161,688 字节。C 盘 Cargo registry、npm/pnpm 缓存、Rustup 下载/临时目录的原物理缓存已移除，旧目录仅保留指向 E 盘的目录联接。Cargo 全局缓存文件迁往 E 盘后，C 盘旧文件数量为零。详细迁移账本位于 `target-windows/tool-cache/migration-20261026.json`。已安装 Rust 工具链、工具可执行文件、模型、凭据和业务数据库不属于清理范围。

Windows 命令先执行：

```powershell
. ./scripts/use-project-build-cache.ps1 -Target live-auto-build
```

脚本将 Cargo、npm/pnpm、TEMP/TMP/TMPDIR 指向项目 E 盘目录，并选择项目原生 Node/protoc。默认 Cargo target 改为 `target-windows/build`，避免默认清理命令删除同级工具缓存。用户环境变量同步指向 E 盘；已运行的终端仍应使用上述脚本刷新环境。

旧诊断目录的联接仅用于保留工具路径，不代表协议或产品兼容层。独立运行版入口默认使用 `target-windows/live-auto-runtime`；必须完成五个程序的构建、校验和维护窗口替换后才能宣布更新成功。

此前共享 rf01/shell 目录在编译期间被其他任务清理，已隔离本轮目录。首次迁移后重编译因系统内存不足失败，后续在提交额度恢复后继续单任务构建；不修改系统分页、不结束用户程序、不将构建转回 C 盘。
