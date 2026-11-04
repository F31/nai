## Why
V43-RF04真实API验收前，操作员需要看到当前任务的预留估算、剩余额度及本机凭据绑定问题，避免点击后才得到笼统失败。依据V4.3任务预算/取消要求及INV-11/35/40，仅补ACC-AG-02/08局部前置证据，不提升M2。
## What Changes
增加不联网、不写任务/预算的本机生成条件检查，复用实际生成的资料闭包与预留算法；界面自动检查并显示具体原因。实际生成再次复检，不把检查结果当授权或供应商认证。
## Capabilities
### New Capabilities
- `llm-local-readiness`: 无付费副作用的本机生成条件检查。
### Modified Capabilities
无。
## Impact
先生成protobuf，再修改唯一writer、engine/host及React消费者；不改SQL、不增加依赖或常驻服务，不读取旧Key。本轮不调用真实付费API。
