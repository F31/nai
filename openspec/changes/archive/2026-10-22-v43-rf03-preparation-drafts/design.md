## Context
RF02已提供唯一writer和不可变对象正文；本任务沿用新线存储机制，不复用旧产品表。草稿使用专用Protobuf命令，JSON文档由单一Schema定义；Rust直接消费源Schema，TS类型从同源生成。
## Decisions
四类草稿为product、host、series、show。事实记录类型、来源、权威声明、条件和有效期，FAQ/话术显式绑定本地事实ID。场次引用固定UUID/revision/SHA256。跨对象存在性和新鲜度属于后续发布检查，本轮不声称已经核验。
对象首次保存后固定类型，generic put_object不能覆写草稿类型；交付prepare及执行前均拒绝草稿信封。现有8KiB正文/16KiB命令限制优先于各字段上限。未完成草稿可以保存，但不成为发布快照或批准对象。
## Risks and Limitations
不提取事实、不验证自由文本语义、不创建发布快照/依赖图/直播会话，不提供普通用户编辑UI。人工确认仅记录用户声明。Schema演进须另作版本及迁移决策，不引入兼容层。
## Validation
源Schema正反例及生成无漂移；核心语义验证；真实SQLite保存/重启读取/CAS/ACL/类型擦除与交付拒绝；真实引擎进程往返。RF03整卡仍进行中，ACC仍pending。
