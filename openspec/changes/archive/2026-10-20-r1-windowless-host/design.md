# Design
main使用cfg_attr(target_os="windows",windows_subsystem="windows")，不以debug_assertions决定用户是否看到控制台。Windows下后台命令经std::os::windows::process::CommandExt设置CREATE_NO_WINDOW，非Windows保持原语义；作用域只在可信宿主引擎命令，不改变引擎CLI诊断形态。播放keeper路径已有该标记，不新增模拟设备或重写协议。
run_bounded继续捕获有界stdout/stderr；私有ControlProcess保留stdio。测试使用Windows PowerShell系统API读取子进程GetConsoleWindow，断言为0且输出可读。壳已有真实引擎集成覆盖初始化/预检/私有IPC。安装构建补PE Subsystem=2检查，明确拒绝CUI壳。
更新开发计划与报告；只跑壳/对应UI权限与安装相关检查。交付包不自动关闭可能存在的用户引擎会话。
