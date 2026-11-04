## Why
Debug工程安装包被标记为控制台子系统，打开桌面界面附带空控制台；后台引擎启动路径也未显式禁止创建控制台。
## What Changes
Windows桌面壳无论Debug/Release均为GUI子系统；统一宿主后台命令构造，正常启动、CLI控制、初始化、预检和Provider测试均CREATE_NO_WINDOW，保留私有stdio及期限。发布构建检查GUI PE标记。
## Impact
R1桌面体验修复，仅宿主/安装构建检查，无新依赖/契约或内核变动。真实启动验证及受影响壳测试，不运行全量或付费API。
