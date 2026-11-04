# model-catalog-tabs Specification

## Purpose
让操作员通过LLM文本与TTS语音两个模型列表直接管理已保存配置，保持模型版本、权限与凭据域隔离，逻辑删除不破坏历史费用或密文引用，并明确区分配置保存、API认证及实际语音输出。

## Requirements

### Requirement: Typed model catalogs and encrypted CRUD
系统 SHALL 分别提供LLM和TTS列表及新增/编辑/删除，TTS音色与Key真实保存，安全读取不得返回Key；模型修改和删除使用当前修订/admin授权并写原子审计。
#### Scenario: Stale or unauthorized mutation
- **WHEN** 非管理员或陈旧编辑器尝试修改/删除模型
- **THEN** 拒绝操作，配置/费用/历史密文不被覆盖；TTS凭据不能跨域用于LLM。

### Requirement: Deletion preserves history and prevents reuse
系统 SHALL 使删除的模型退出可用列表且不可继续调用，同时保留历史版本、账单和加密凭据；删除不可变成自动退款、重试或隐式恢复。
#### Scenario: In flight or completed task reference
- **WHEN** 操作员删除模型
- **THEN** 在途任务时拒绝删除；允许删除后已完成结果不恢复调用许可，已知/未知费用与历史依据仍保留。

### Requirement: Direct two tab list management
界面 SHALL 以LLM文本模型/TTS语音模型两个Tab展示列表和逐行编辑/删除，新增或编辑时才展开表单，删除二次明确确认；切换不隐式保存或调用模型。
#### Scenario: Switch tabs or cancel editor
- **WHEN** 操作员切Tab、取消编辑或只打开设置
- **THEN** 不产生付费请求或配置写入，输入Key清空；TTS执行缺失明确显示未接入，不提供假测试成功。
