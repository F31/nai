# Design
新增EnqueueRunQuestion封装现有入池参数、ReviewSource及expected_epoch。root核验场次未结束、发布与工作区一致，live epoch匹配，manual明确当前记录；原EnqueueQuestion保留独立私有查询用途，不自动推断场次。
每问题一个publication_review_question（按workspace/actor/question ID确定性寻址）与每场次一个publication_review_question_index，最多64项；旧120秒问题池32项规则不变，重复计数最多99，跨场次不去重。新记录为独立复盘用途的操作员原始问题，不含候选答案，保存候选ID/hash和处理状态；运行TTL仍禁止继续任务/候选，归档原问题仅在发布/事实与当前权限仍有效时可读。
所有处理回执与原操作同事务。FAQ采用/编辑/忽略/取消与公共音频提交/软件完成分字段，后者不声称平台播出或硬件完成。问题父对象删除/撤销水位与跨区身份必须复检。历史未绑定记录不自动补归属。
ReadReview附有界问题清单，显示来源/重复/处理/输出证据；人工点击整理为重点只填入原问题并要求事实/回答核对。ReviewNoteSave可带问题记录ref；保存、预览、应用及幂等读回均复检记录存在、归属和版本，问题变化给出冲突，不复制旧候选或批准。
定向验证涵盖两场同资料同问题不串、幂等/重启、处理状态区分、超时不延长、来源/权限/问题删除后拒绝、记录容量、critical日志回滚、实际IPC和UI；不做全workspace测试。工具缺边以源码核对为准。
