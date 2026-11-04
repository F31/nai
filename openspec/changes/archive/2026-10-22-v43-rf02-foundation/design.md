## Decisions
延用单稳定runtime-core和生成契约，SQLite实现放adapters/storage-sqlite，组合根留apps/workbench-engine；旧业务库和壳内ProviderDb不引用。foundation入口只能显式创建独立数据根，打开时先验证新根标识；SQLite独占writer跨进程互斥，所有命令短事务、有界参数/队列/期限。主体由可信父进程认证绑定，命令不含actor授权字段。

对象正文存内容寻址文件、版本/ACL/审计/outbox同SQLite事务提交，孤儿文件不授予引用权限。发布产品语义留RF03，不以通用版本对象冒充产品已发布。预算金额整数/币种，未知结算保留预留。单调许可不跨重启恢复，会话/未交付批准在恢复时失效。

DeliveryGate仅为人工私有文字提供确定性input/evidence/output检查与精确bundle批准；未实现或未知公开/媒体目标拒绝。独立原子停止栅栏先撤许可，数据库日志失败不回退。外部副作用不放DB事务，dispatching写前记录；恢复为OutcomeUnknown而非自动重发。私有卡投影只是Submitted，没有伪造可见/观众已读或Completed。

## Verification
验证独占writer/跨workspace及撤权/CAS并发/事务回滚/篡改文件/幂等冲突/预算未知/过期与取消/审批身份和hash/停止不等数据库/重启失效/outbox不重发。真实进程重启与崩溃边界，Windows单独验证；只运行改动与相关契约消费者，不做全仓发布或安装验收。

撤销使用有界scope栅栏和独立fsync意图文件，持久层按角色revision/对象墓碑复核并清理已解决意图；P0全局栅栏不获取scope锁或DB锁。初始化根标识+数据库事务+initialized.ok构成初始journal，已有committed DB缺失禁止静默重建。对象所有权单独记录，GC每批最多128，仅删除未引用且hash一致的本writer文件，未知素材不碰。stdio收发都有限期，处理前扣除排队时间，堵塞输出线程不阻止主进程按期退出。当前入口64请求/60秒、空闲4秒，为RF02基础运行入口，RF03才接用户工作台与正式常驻生命周期。
