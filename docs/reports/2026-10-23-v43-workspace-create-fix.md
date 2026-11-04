# V4.3 工作区创建与原生权限修复

日期：2026-10-23。状态：实现、定向验证及独立桌面壳构建完成；90项ACC保持pending。

## 原因与行为

新增的本机音频停止和nonce命令已注册到handler，却没有进入AppManifest和窗口能力清单。创建工作区已写入后，切换工作区会等待音频停止，命令被ACL拒绝，因此界面只显示通用失败。此前空库启动烟测没有调用这两个命令，未覆盖该问题。

补充仅限这两个固定命令的权限。连接在显示就绪前真实调用停止、读取并验证nonce；权限缺失、非原生宿主和协议错误分别给出具体说明。

创建后保留工作区ID，后续读取或切换失败可“恢复切换”，只恢复原ID。写入结果未知时先读取核对，不重复提交创建；确定拒绝不会显示已创建。已存在的工作区不删除，重连会读取列表。

## 验证

- 前端类型检查及专用V4.3生产构建通过。
- 工作区恢复、原生权限、bootstrap、业务语音和音频授权相关前端测试：16 passed / 0 failed。
- Windows真实foundation宿主集成：4 passed / 0 failed。
- Windows桌面壳构建通过；隔离数据根原生启动烟测通过，输出 `WORKBENCH_PREPARATION_READY: authenticated owner protocol=2 live_ready=false`。就绪前已执行两个真实原生命令。

本轮不运行全仓测试，不调用付费API，不迁移数据库或修改Key，不更新旧开始菜单安装。未执行浏览器自动交互；用户真实界面点击仍需人工确认。既有数据库、工作区和凭据保留。

用户关闭窗口后已更新独立开发版桌面壳，并重新打开，进程路径核对为 `target-windows/shell/debug/nai-workbench-desktop.exe`。OpenSpec变更已归档为 `2026-10-23-v43-workspace-create-fix`。
