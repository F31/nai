# Design
来源只接受已结束manual_run_draft或已结束/提交完成publication_live_run，引用同工作区不可变发布版本；人工重点始终标self_reported，不伪造观众/系统交付证据。每场最多3项、字段有界，明确编辑版本。

nai-live负责纯FAQ提议验证/差异与克隆方案，无数据库、Tauri、模型、网络或平台SDK。sqlite adapter负责单writer事务、ACL/来源/事实/撤销水位、CAS和critical日志。消息单源Protobuf，受保护note/job单源JSON Schema，不允许generic页面写入或绕过来源读旧正文。

预览创建review_draft_job，绑定source run、note revision/hash、product/show base revision/hash、拟新建target IDs。应用前再次核验：原发布仍获准、源运行和提议存在、事实/条件未变、base仍为当前版本。目标是新的独立产品和节目草稿，不覆盖源草稿，不复制批准/会话/租约。两份草稿和job状态同事务，失败无部分业务提交。

相同业务提议/base重试返回已有job，重复apply返回同结果；不同参数复用job ID拒绝。并发修改显示base/current/proposal三方对照，需人工重建预览，不静默合并；事实变化阻止应用并转内容核对。源撤销/删除后不返回旧正文，已保存的用户草稿不冒称可远程撤回。

UI常用入口在工作区“复盘与下场”，默认所选已结束场次；FAQ/事实能选择则不要求输入ID，只有人工问题/建议文字需要填写。保存草稿后给出下场位置/编辑入口，醒目标明未发布/未播出。先验证纯领域/存储并发/日志回滚/撤权/幂等，再原生IPC/UI和构建更新。仅受影响组件检查，不跑全workspace。
