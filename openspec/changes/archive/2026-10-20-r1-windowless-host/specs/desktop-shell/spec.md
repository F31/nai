## ADDED Requirements
### Requirement: Windowless desktop host and engine startup
Windows桌面壳 SHALL 在Debug和Release都使用GUI子系统，宿主启动后台引擎 SHALL 不创建控制台窗口，且保留stdio控制及错误结果。
#### Scenario: Open engineering build
- **WHEN** 操作者启动Debug工程桌面壳
- **THEN** 显示应用界面，不附带控制台
#### Scenario: Execute background engine operations
- **WHEN** 启动引擎、初始化、预检或测试Provider
- **THEN** 子进程不创建控制台，私有控制/有界诊断保持可用
