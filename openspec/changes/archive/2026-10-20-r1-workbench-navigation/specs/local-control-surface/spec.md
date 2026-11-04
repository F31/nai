## ADDED Requirements
### Requirement: Task oriented workbench and categorized settings
桌面 SHALL 默认展示日常工作台、大图标常用动作与右上角设置入口，配置 SHALL 按菜单分区而非平铺。导航不得自动保存、丢弃修改或触发付费请求。
#### Scenario: Switch between LLM and TTS settings
- **WHEN** 操作者修改LLM表单后切换TTS再返回
- **THEN** 保留原输入和未保存提示，只有显式保存才持久化
#### Scenario: Workbench emergency and approved playback
- **WHEN** 工作台忙碌或存在未批准候选
- **THEN** 已连接时紧急静音仍可操作；未批准不得播放，审批仍需两次确认
#### Scenario: Keyboard settings navigation
- **WHEN** 操作者打开设置或按Escape关闭
- **THEN** 焦点保持在设置内并在关闭后返回入口，背景不可误操作
