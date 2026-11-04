## ADDED Requirements
### Requirement: Setup files are selected through a narrow native dialog
桌面 SHALL 为场景文件和运行配置文件提供中文路径字段与原生JSON文件选择按钮。宿主 MUST 只接受两种已声明选择目标，不提供任意读取/执行能力；取消 SHALL 保留原表单，失败 SHALL 释放忙碌并显示原因。选择文件 SHALL 不自动保存或启动。
#### Scenario: Select or cancel
- **WHEN** 操作者选择JSON文件或取消对话框
- **THEN** 仅选择成功的对应路径更新，取消不修改任何字段且不触发引擎/API。
#### Scenario: Invalid or concurrent request
- **WHEN** 目标不受支持或已有选择器打开
- **THEN** 宿主拒绝该请求，不打开额外对话框。
### Requirement: Provider models have presets and lossless custom editing
每个LLM/TTS主备配置 SHALL 提供模型下拉选择，预设自动填充现有协议/地址/模型字段；只有自定义模式 SHALL 显示地址和模型输入。未知已存配置 MUST 原样回显为自定义；切换不得自动保存、测试、携带密钥或变更其他行及主备顺序。旧共享临时Key SHALL 默认折叠并说明用途和留空条件。
#### Scenario: Choose a preset
- **WHEN** 操作者选择预设
- **THEN** 当前行得到对应公开参数，保持身份/路由/预算，地址或协议变更清除该行未保存Key，不发网络请求。
#### Scenario: Existing custom endpoint
- **WHEN** 读取未精确匹配预设的已存配置或切换自定义
- **THEN** 原地址和模型保持可编辑，不被默认预设覆盖。
