## ADDED Requirements
### Requirement: Model deletion is directly discoverable in the deployment list
模型列表每行 SHALL 提供删除按钮，最后一项 SHALL 禁用并说明至少保留一个模型。删除 SHALL 仅修改草稿及该项待保存Key，显式保存前不修改数据库；其他模型及其Key不得被清除。
#### Scenario: Delete a selected model
- **WHEN** 停止引擎后删除存在其他同类模型的选中记录
- **THEN** 列表移除此项，展示剩余首项并提示保存；最后一项不可删除。
