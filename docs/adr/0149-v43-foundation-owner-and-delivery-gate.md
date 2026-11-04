# ADR 0149：RF02单写owner、撤销意图与私有交付基础

日期：2026-10-22。状态：RF02基础范围已实现/定向实测；完整产品/渠道认证未完成。

## 决策

1. 沿用nai-runtime-core稳定机制，新增foundation纯策略/Operation/StopFence/BudgetPort。SQLite放adapters/storage-sqlite（nai-foundation-sqlite），只依赖core/契约，不依赖另一个adapter；apps/workbench-engine的nai-foundation-engine负责装配，不建第二套内核或常驻云服务。
2. FoundationRequest/Response为生成契约；actor不在命令中，可信本地父进程用临时凭据绑定principal。入口使用64KiB小端帧、严格seq、容量1通道、最多64请求/60秒、4秒空闲/收发期限，并扣除收包后的排队耗时。远程身份/Windows进程沙箱/配对认证不在此卡承诺。
3. root须显式--initialize且为空。根清单、初始数据库事务和initialized.ok构成初始化journal，崩溃可继续未完成的初始化，已提交DB缺失/版本不匹配则拒绝，不自动建空库掩盖丢失。OS文件锁+SQLite独占writer；不读旧库/Key或自动迁移。
4. 通用对象版本不可原地覆盖，CAS更新head；产品发布/事实审核仍留RF03。正文按SHA256存本地文件，数据库事务记录版本/ACL/结果/审计/outbox。对象所有权另记；GC每次最多128项，只清理writer登记、无引用且内容hash一致的文件，未知/被替换素材不删除或覆盖。
5. 权限扩大先持久化。角色撤销/内容删除先记scope栅栏及独立fsync撤销意图，后业务事务/审计；失败或重启都拒绝旧权限，已解决意图按DB revision/墓碑回收。全局StopFence为原子栅栏，不等待scope锁或DB；停止不等于撤回已经提交的结果。
6. 首个具体目标仅为本机人工私有文字卡片，执行确定性的输入/来源/输出约束、精确bundle批准和执行前重检。bundle绑定主体、角色revision、session/epoch、对象revision/hash、scope/target/TTL；公开/媒体/未知目标拒绝。目标不是音频，不启动keeper/TTS/GPU。
7. dispatching先提交，随后投影到私有卡读模型。投影只证明Submitted，不能标UI已呈现或Completed。重启清旧批准/停会话，dispatching转OutcomeUnknown且不重发；未知费用保留预留、待对账，不当作0或静默释放。当前预算profile为CNY整数最小单位，无真实Provider调用。
8. 当前基础入口为开发装配，不是正式常驻直播服务；RF03接工作台/产品发布/正式owner生命周期及真实私有预演回执。旧安装不更新，RF01壳也不持有SQL写owner。90项ACC均pending，RF02仅前置证据。

## 后果与限制

RF02提供可复用的存储/门控基础，但不声称产品备播、完整RAG、多人公共批准、公开文字/设备/平台、手机或正式备份升级已完成。普通对象编辑不是发布操作；snapshot依赖/事实影响/授权升级由后续契约定义。CNY/16条待交付/8KiB文本/1024对象版本及256条未解决scope栅栏是当前基础profile上界，不是吞吐或商业SLO。Windows ACL/跨OS共享同一根及生产多用户身份尚未认证。

验证见[RF02报告](../reports/2026-10-22-v43-rf02-foundation.md)。
