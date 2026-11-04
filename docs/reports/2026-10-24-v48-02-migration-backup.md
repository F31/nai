# V48-02前置：一致备份与隔离恢复验证

状态：本前置内部存储能力实现并定向实测；V48-02整体仍InProgress。没有用户数据库迁移、共享目录或生产回滚交付，164ACC仍pending。

## 实现

先增加model-migration-backup JSON Schema并纳入项目验证器/生成器，再实现runtime-core清单校验与Store内部维护接口。只允许root_owner、UUID目录ID、固定owned路径，不接受页面任意文件路径；复检预览hash，并拒绝未停止会话/待派发或在途outbox。

SQLite online backup按128页推进，检查取消/deadline，快照quick_check通过后执行可写句柄sync。复制活动对象全部历史版本、根标记与持久撤权记录，逐文件校验SHA256/长度。完成manifest最后create_new并sync，verified标记只追加/幂等校验；缺manifest的半成品不能验证。最多8份备份、64MiB数据库、3840对象摘要、256撤权条目；Schema总条目4096，超限拒绝，不截断后报告完整。

隔离恢复只复制到全新model-migration-verification/UUID目录，拒绝覆盖。复制后再次比较清单，验证代码可以Store::open重开读取，并保持既有恢复撤权逻辑。没有覆盖当前库、自动回滚、删除旧备份或导出Windows Keyring的入口。Unix新文件/目录限制0600/0700；Windows沿用owned根继承权限，本轮未认证独立DACL防护。

## 检查与边界

- Windows存储组件81项通过，新增3项覆盖数据库+LLM/TTS配置重开、原数据库字节不变、撤权保留、拒绝重复目标、重启验证、活动会话/越权/计划变化/路径拒绝、损坏数据库及缺清单拒绝。
- 存储/宿主/引擎相关all-targets clippy通过；本轮后加quick_check取消钩子后存储81项再次通过。
- 生成及schema --check无漂移，现有前端类型检查与生产构建通过。
- host --lib无测试用例，不计真实IPC验证；备份接口未暴露UI/IPC，未替换运行版。

这是磁盘逻辑恢复验证，不是断电认证；未测试跨机器凭据恢复、Windows凭据库备份或目录元数据断电持久性。清单明确external_keyring_not_exported；恢复配置文件不代表新的数据目录可直接解密Key。实际迁移journal的应用/失败回滚/保留提交后新事实阶段仍须下一任务实现，不能将manifest/verified两个备份阶段冒充完整迁移事务。

测试首次有临时目录提前释放导致NotFound，修正测试生命周期后通过；链接暂时LNK1104按既有脚本复跑，未终止用户程序。没有付费调用、用户Key读取/修改、真实数据清理或全仓测试。

下一步：凭据原scope验证与共享scope重加密、显式冲突映射、迁移应用journal和崩溃恢复，然后才开放共享调用及确认迁移界面；关联回收站永久清理继续依赖共享模型保护。
