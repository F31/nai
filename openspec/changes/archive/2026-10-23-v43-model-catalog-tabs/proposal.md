## Why
用户要求设置页以LLM/TTS两个Tab和列表管理多个模型，补齐RF04模型管理与RF05语音配置前置；关联INV-11/20/35/40，不将配置保存当作语音输出或供应商认证。
## What Changes
双Tab模型列表、明确新增/编辑/删除，默认收起编辑器；新增独立TTS模型/音色/加密凭据与保存接口，LLM/TTS均支持CAS逻辑删除。历史密文和计费依据保留，不删除系统凭据或用户数据。在途调用时拒绝模型删除，已删除模型不可重新调用。
## Capabilities
### New Capabilities
- `model-catalog-tabs`: 两类模型的列表化配置与受保护CRUD。
### Modified Capabilities
无。
## Impact
先改protobuf/JSON Schema并生成，再改writer、vault装配、宿主和React。复用现有加密算法、独立TTS凭据域，无新依赖/SQL迁移/常驻服务。不增加真实付费调用，不宣称TTS试听或输出已实现。
