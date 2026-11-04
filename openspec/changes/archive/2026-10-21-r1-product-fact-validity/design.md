## Context
ImportedDocument、快照检索、candidate_documents_valid及approval_valid已检查文档时间。operator导入固定None、商品草稿字段缺失，是实际断点。
## Decisions
两个可选字段valid_from_utc_ms/valid_until_utc_ms成对出现，整数0至253402300799999（小于JS安全整数，至9999年），from严格小于until，闭开区间[from,until)。旧草稿不自动补永久有效；普通手工问答保持兼容，但人工辅助节目准入要求所有事实窗口已声明且当前有效。UI本机时区展示，数据库存UTC毫秒，不含Key。
相同元数据传入现有ImportedDocument，不改SQLite schema。候选expires_at取请求TTL与所有证据有效期最早值，生成deadline同时收紧；已经派发的调用可能计费，过期取消、不重试，不留下可批准结果。既有审批/设备租约再次检查，不延长事实期限。文档类型与结构化价格/活动值、其余五面生产者及PitchRunner接线仍未实现。
## Risks
未设期限仍可手工问答，不代表机器Fresh；节目界面明确引导补充时间。发布历史/未来窗口允许保存，检索按实际时间过滤；不把未生效当无知识库错误。时间来自系统UTC，不提供外部时间同步认证。旧二进制不认识新增配置时应拒绝，不能静默删除字段。
## Validation
UI边界/时区/未知与到期状态；原生数据库保存/读取/发布保持元数据；真实engine本地协议测试未生效/过期零请求、证据期限收紧及期满审批/合成拒绝；存储相关回归和契约生成检查。Windows逻辑不替代设备/OBS认证，不访问付费API或真实用户数据库。
