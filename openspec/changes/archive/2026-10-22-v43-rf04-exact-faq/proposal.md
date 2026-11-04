## Why
V43-RF03手工备播已贯通，下一步RF04先提供设计§14.5/§15.2/§21.1–21.3的规则/精确FAQ私有短答，避免未配置模型就用常识编造回答。对应ACC-KB-02/ACC-CONT-02/ACC-AG-08目标及INV-03/05/11/21/35/40，仅补局部前置证据。
## What Changes
新增有界手工精确FAQ任务与来源卡片，任务和建议状态分离，允许忽略/取消或明确采用/编辑后追加至产品话术草稿。固定发布上下文、当前输入版本/权限/撤销/有效期复检，未知/歧义拒答。产物保留publication_保护命名空间，仅为工作区共享资料派生；不添加私有观众输入。正文不能直接外发。
## Capabilities
### New Capabilities
- `exact-faq-assistance`: 有依据的私有规则问答、任务生命周期及原子草稿采纳。
### Modified Capabilities
无。
## Impact
protobuf和publication JSON Schema先生成，runtime-core纯检索策略、SQLite owner、宿主白名单和新UI消费者。沿用对象版本/审计事务，无新增表、运行依赖或常驻服务。只执行零Provider调用的确定性路径，不使用Key。LLM/API配置、付费任务调度/预算预留与异步在途取消仍未实现，不冒充RF04/M2完成。
