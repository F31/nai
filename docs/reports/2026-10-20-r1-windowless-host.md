# 桌面GUI与后台无控制台启动（2026-10-20）

## 修复
用户报告打开控制台界面时出现额外空cmd/控制台。实测旧D:/nai Engineering/nai-desktop-shell.exe的PE Subsystem=3（CUI），入口cfg_attr只对非debug_assertions设置windows_subsystem，Debug工程安装因此保留了控制台。

桌面入口改为所有Windows构建使用GUI子系统（Subsystem=2），独立引擎CLI仍保留命令行能力。统一background::command使用标准库CommandExt的CREATE_NO_WINDOW（0x08000000），覆盖CLI私有控制启动、界面启动、provision、整机预检与逐Provider测试五条宿主入口。保留stdin/stdout私有Protobuf、已有stderr处理、期限与退出码，不使用DETACHED_PROCESS、不新增权限或运行依赖。生产本机播放keeper已有CREATE_NO_WINDOW，不改设备/媒体链路。

打包脚本对桌面壳增加GUI PE标记检查，Debug/Release产物不符合则拒绝，防止同类回归。登记新增Windows分支到既有process-supervision矩阵；未改业务契约，无需生成新消息。

## 匹配范围验证
独立Windows壳21项lib及6项真实engine集成通过，all-targets clippy干净。新增Windows真实进程测试：通过后台构造器启动系统PowerShell，测试专用P/Invoke读取GetConsoleWindow必须为0，且通过现有run_bounded捕获NAI_NO_CONSOLE_STDIO_OK；不使用生产Python/PowerShell推理，不引入生产PowerShell依赖。既有真实engine测试覆盖初始化、预检与私有IPC。

package-windows.ps1语法检查通过。日志.nai-diagnostics/windowless-host-tests.log。平台矩阵23项通过（/tmp/nai-windowless-platform.log）。新Debug壳PE Subsystem=2；实际--smoke EXIT=0、SHELL_UI_READY通过，stdout重定向仍可用（.nai-diagnostics/windowless-host-smoke.log）。打包实际GUI门槛通过，日志.nai-diagnostics/windowless-host-package.log。

遵照用户测试偏好，本轮仅运行宿主/启动/对应平台登记与打包检查，不重复无关知识、升级、Provider或全量检查；未调用付费API。数据库、密钥、代理与系统策略不修改。代码未提交。

新NSIS 10,932,019字节，SHA256 0026efafd9d944a796bbe7ffd5e738ed2de45cd3269c5315326d83fdc7c4dd21，NotSigned。用户D:/nai Engineering旧壳仍在托盘运行，已请求操作者保存表单并退出；未强停或覆盖用户进程，安装更新待退出后执行。

OpenSpec已归档2026-10-20-r1-windowless-host；43个主规格严格校验及169项归档一致性3/3通过。当前D目录PID 50164仍在后台，安装替换保持待操作者退出。

## 用户安装已更新

用户明确回复已退出后，已用产品卸载器清理旧程序，并将0026efaf…GUI修复包装回D:/nai Engineering。原安装路径实测PE Subsystem=2、--smoke EXIT=0/SHELL_UI_READY；用户6个配置/数据库文件逐项SHA256保持不变，Key保留。已重新打开新版；无引擎启动或API调用。证据.nai-diagnostics/windowless-update-result.json、windowless-installed-smoke.log及windowless-installed-open.json。此前待退出状态已解除。
