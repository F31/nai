## ADDED Requirements
### Requirement: Rendered application startup acceptance
桌面启动冒烟 SHALL 仅在真实应用根挂载且关键界面可见后成功，导航完成不得替代React渲染成功。生产构建 SHALL 使用同一React实例提供组件与渲染器。
#### Scenario: JavaScript startup crashes
- **WHEN** 导航完成但React启动异常导致空root
- **THEN** 冒烟不得报成功，并在期限内具名失败
#### Scenario: Production application mounts
- **WHEN** 真实生产bundle成功渲染工作台并挂载
- **THEN** 冒烟才标记界面就绪，浏览器验证标题与控制入口实际存在
