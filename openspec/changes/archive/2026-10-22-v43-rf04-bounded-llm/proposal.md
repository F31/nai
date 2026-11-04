## Why
RF04精确FAQ完成后，需要一个真正可配置、有依据、受预算约束且能取消的私有短备稿任务。依据V4.3 §21/§22及RF04计划，补ACC-AG-02/08、ACC-KB-02局部证据；不授予完整RF04/M2或平台认证。
## What Changes
新增多模型配置/独立凭据密文、工作区人工预算、显式选择模型的短话术改写任务、异步HTTP/取消与已发生费用结算、当前上下文守门及人工采用/编辑/忽略。只输出私有待核对候选，不自动发布或调用TTS。
## Capabilities
### New Capabilities
- `bounded-llm-drafting`: 有预算与独立凭据的异步私有短备稿。
### Modified Capabilities
无。
## Impact
先生成protobuf/JSON Schema；core定义策略/端口，SQLite adapter唯一writer，新增静态受信HTTP/凭据adapter由engine装配，宿主与React只调用受限命令。复用锁定reqwest/tokio/ring/keyring，不增加常驻服务；网络工作线程没有数据库权限。不依赖旧v1业务契约、旧runtime或旧凭据。正式凭据Windows可用，非Windows未实现原生凭据后端时显式不可用；tests使用具名替身，不冒充真Provider。本轮不使用历史Key或付费API，真实调用需具体费用授权。
