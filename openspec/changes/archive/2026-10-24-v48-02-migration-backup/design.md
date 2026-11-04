## Design
先定义JSON Schema备份清单并纳入生成；同owner检查预览hash及任务/会话排空后，SQLite online backup到固定owned路径，复制活跃对象所有历史版本和revocation，保留root/ready标记，逐文件SHA256。目录ID只接受UUID，所有路径拒链接；条目/单文件/DB大小及deadline有界。manifest最后create_new+sync发布，缺manifest是未完成，不得用于恢复；verify标记只追加。
隔离恢复只允许固定migration-verification/UUID目录，禁止覆盖活动库和任何已有目录。打开恢复库仍走Store::open既有恢复撤权；不复制Keyring秘密，不宣称跨设备可解密或生产回滚已完成。后续迁移执行器仍需post-commit新事实保护与独立journal阶段。测试覆盖改plan、活跃任务、损坏/缺文件、越权、路径和磁盘重启恢复。
