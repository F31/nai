## ADDED Requirements

### Requirement: The permitted context is presented as one user turn

文本适配器 MUST 把 `permitted_context` 作为**恰好一条 `role: user` 的消息**发出，且 MUST NOT 自拟任何提示文本
（既不加系统轮，也不加模板前缀）。文本生成请求 MUST NOT 只含非 `user` 轮：系统轮不构成「用户查询」，
部分网关会以 4xx 拒绝该形状，故 system-only 请求 MUST NOT 被构造。

两种文本协议的请求体形状差异 MUST 限于路径、协议附加标志与用量字段名，MUST NOT 延伸到**同一语义字段的角色选择**——
`permitted_context` 在两种协议下的角色 MUST 一致。

#### Scenario: Both text protocols send the context as one user turn
- **WHEN** 同一个文本生成调用函数先后以两种不同文本协议运行
- **THEN** 两处请求体都把 `permitted_context` 作为唯一一条 `user` 轮发出，且都不含适配器自拟的提示文本

#### Scenario: No system-only text request is constructed
- **WHEN** 构造一次文本生成请求
- **THEN** 请求体的 `messages` 中存在 `role: user` 的消息，且不存在因适配器自拟而产生的额外轮次
