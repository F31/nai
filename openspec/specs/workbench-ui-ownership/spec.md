# workbench-ui-ownership Specification

## Purpose
规定新版备播工作台前端源码、构建配置及运行入口的唯一归属，保证旧桌面界面退役后仍可独立构建与启动，并明确用户数据、旧安装及未提交源文件恢复快照的保护边界。

## Requirements

### Requirement: 新工作台前端必须独立归属
系统 SHALL 仅从workbench-desktop包构建在用UI，入口为index.html，不再导入旧desktop目录，保留模型/工作区/音频等已交付能力。
#### Scenario: 独立构建
- **WHEN** 旧desktop目录不存在
- **THEN** 新包可完成类型检查与生产构建，并由新壳启动

### Requirement: 旧UI退役必须保留未提交成果和业务数据
系统 SHALL 在移除旧UI前核验本地恢复快照，旧专用测试/打包入口退出在用门控，用户数据库/Key/素材/旧安装保持不变。
#### Scenario: 清理旧入口
- **WHEN** 删除旧UI源码及专用入口
- **THEN** 可从核验快照恢复原文件，历史证据不冒充新线验收

