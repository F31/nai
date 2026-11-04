## ADDED Requirements
### Requirement: Normal connection does not require technical file configuration
普通连接 SHALL 自动使用已有工作区与管理场景，缺省使用宿主随包配置。身份/路径/旧共享Key SHALL 默认折叠，不作为普通用户填写前置。首次显式连接且引擎库不存在时 SHALL 自动初始化；已有库或未知访问状态 MUST NOT 被当作空库覆盖。
#### Scenario: First connection without engine database
- **WHEN** 操作者完成服务配置并显式连接且引擎库不存在
- **THEN** 使用随包运行配置初始化后连接，不要求勾选技术初始化或选择文件，不调用模型。
#### Scenario: Existing database
- **WHEN** 库已存在或访问状态未知
- **THEN** 不自动重新初始化；保留已有配置，异常具名失败。
### Requirement: Models use automatically loaded deployment lists and focused editing
LLM/TTS SHALL 自动读取数据库并以主备列表/选中项编辑呈现，MUST NOT 出现常规读取数据库或跨能力批量覆盖按钮。模型/音色/Key优先，高级参数折叠；主备调整 SHALL 保留整项绑定。未知地址/模型/音色 MUST 原值保留；加载失败或旧请求迟到不得覆盖当前编辑。Key变更 SHALL 标记未保存，换地址/协议 SHALL 清除且仅清除本项未保存Key，旧绑定不得显示可用。
#### Scenario: Edit an existing deployment
- **WHEN** 进入页面或编辑/设主已有模型
- **THEN** 自动回填且无需重新输入已保存Key；其他模型/另一能力不被批量重置，不自动调用API。
#### Scenario: Unknown voice or loading failure
- **WHEN** 旧音色不在预设中或配置读取失败
- **THEN** 未知音色以自定义原样编辑；读取失败仅提供恢复入口，不允许空配置覆盖保存。
