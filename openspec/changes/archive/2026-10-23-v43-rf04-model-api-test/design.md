## Decisions
复用唯一writer、outbox、预算和有界worker；独立TestLlm命令和publication_llm_test产物，无发布快照依赖。沿用Read/Cancel，但Resolve拒绝测试产物。固定输入和最长256输出token/15秒，只发一次，不重试；已发送未知费用继续占用。模型修订/ACL/停止在派发和完成重新验证。响应仅显示有限正文，不显示头/原始错误体/密钥。
## Validation
定向storage/adapter/engine状态机、取消/重启及UI/host；生成无漂移和三语言夹具。Windows构建及烟测独立执行；本地替身不作为真实认证，原安装不激活。
