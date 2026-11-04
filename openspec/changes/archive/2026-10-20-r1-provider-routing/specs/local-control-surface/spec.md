## ADDED Requirements
### Requirement: Independent provider configuration UI
系统 SHALL 允许分别编辑LLM/TTS一主多备、协议/地址/模型/音色及路由策略，保存非机密配置；密钥只在本次子进程环境使用。
#### Scenario: Save and launch configured routes
- **WHEN** 操作者保存两组合法配置并启动
- **THEN** 引擎使用保存的独立能力配置且文件不含密钥，素材路径保持有效
#### Scenario: Change active configuration
- **WHEN** 引擎已运行且操作者修改配置
- **THEN** 拒绝覆盖运行配置，允许显式停止撤权后保存并重新启动
