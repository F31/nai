## ADDED Requirements
### Requirement: Durable provider configuration and encrypted credentials
宿主 SHALL 将供应商配置和加密凭据持久化至数据库，界面可修改主备、路由、端点、模型、音色并更换/删除Key；不得向界面回读明文Key。
#### Scenario: Restart after save
- **WHEN** 用户保存配置和Key后重启宿主
- **THEN** 配置和已保存凭据状态可读，启动与显式测试可使用保存凭据，无需重新输入
#### Scenario: Credential isolation and storage failure
- **WHEN** 端点变更、密文被篡改或系统密钥不可用
- **THEN** 不向新身份泄露旧Key，不返回伪成功或明文降级，失败事务保留旧数据

## MODIFIED Requirements
### Requirement: Independent provider configuration UI
系统 SHALL 允许分别编辑LLM/TTS一主多备、协议/地址/模型/音色及路由策略，保存数据库配置与加密凭据；明文密钥仅在本次子进程环境使用，不回读界面。
#### Scenario: Save and launch configured routes
- **WHEN** 操作者保存两组合法配置并启动
- **THEN** 引擎使用保存的独立能力配置且JSON导出不含密钥，数据库只存密文，素材路径保持有效
#### Scenario: Change active configuration
- **WHEN** 引擎已运行且操作者修改配置
- **THEN** 拒绝覆盖运行配置，允许显式停止撤权后保存并重新启动

### Requirement: Per provider test action and current result
配置界面 SHALL 提供已保存条目的显式付费测试动作，测试后清空该项Key输入、保留已保存密文，只显示身份匹配且本次产生的报告。
#### Scenario: Probe completes
- **WHEN** 操作者点击一个条目的测试按钮
- **THEN** 显示该项状态/耗时/错误码，清空该Key输入且不删除已保存凭据，不把成功描述为设备或整组认证
#### Scenario: Stale report or process failure
- **WHEN** 探针超时、进程失败或报告身份/版本不符
- **THEN** 拒绝显示旧成功，具名宿主错误与供应商失败分开
