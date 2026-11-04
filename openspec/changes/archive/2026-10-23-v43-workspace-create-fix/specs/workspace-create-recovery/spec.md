## ADDED Requirements
### Requirement: 原生依赖真实就绪
桌面 SHALL 明确授权固定本机音频停止/nonce命令，并在连接就绪前实际调用和校验；未授权或无原生宿主时给可执行提示，不显示已连接可写。
#### Scenario: 音频权限缺失
- **WHEN** 工作台连接时音频命令被拒绝
- **THEN** 不报告frontend ready/connected，明确提示更新桌面壳
### Requirement: 创建成功后的恢复不重复写入
系统 SHALL 在创建成功后保留新工作区ID；后续列表或切换失败重试只读取/激活该ID，不创建第二个工作区，不删除既有数据。
#### Scenario: 已创建但切换失败
- **WHEN** 后续切换调用失败
- **THEN** 显示已创建待切换，并允许恢复到同一个工作区
