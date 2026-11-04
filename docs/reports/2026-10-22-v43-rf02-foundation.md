# V43-RF02：存储、身份与私有交付基础

日期：2026-10-22。状态：本卡定义的基础组合已实现，并经WSL/Windows定向实测；90项ACC仍pending。旧安装、数据库、Key、许可、素材及既有未提交改动保留，无付费调用/全仓测试/提交或发布。

## 交付与真实边界

新增生成的Foundation命令/响应/角色/目标及根Schema、独立初始SQL；catalog为35项（3 frozen、32 draft），新增workbench.foundation@1，不将目标产品/节目路由冻结为已交付。core新增纯策略Operation/StopFence/DeliveryGate/BudgetPort，SQLite adapter持唯一writer与对象仓库，engine新增真实认证foundation入口；桌面没有第二个SQL owner。

已形成：创建工作区/角色→人工通用对象CAS存储与不可变版本→私有会话→prepare/精确批准→写前dispatch意图→私有文本卡读模型→进程退出/强杀→重启读取与撤掉旧授权。对象版本、幂等结果、角色、审计/outbox事务一致，文本无需媒体或API。卡片只是Submitted/Delivering，没有伪造UI可见/Completed。

角色撤销或删除先写内存scope栅栏和独立fsync撤销意图，关键审计失败后不回退权限；重启仍保持拒绝。初始journal可恢复未完成创建，已提交DB缺失拒绝而非静默重建。只清理本writer登记、未引用、hash一致的文本；未知文件、不匹配的正文/所有权标记均不删除或覆盖。全局停止为原子栅栏先行，停止结果区分待取消、已提交、执行中与结果未知。

当前入口有意限制为Local/人工私有文字的基础开发组合：64请求/60秒、容量1通道、4秒空闲/收发期限、16条待交付、8KiB正文、1024对象版本、256条未解决scope栅栏。CNY整数预算先预留，未知不置零/不释放，重启未结算保守待对账。未启用公开/媒体、Provider或长播服务。

## 实测

| 范围 | 结果 |
|---|---|
| 新SQLite adapter（Linux/WSL） | 22 passed / 0 failed：writer、跨空间/撤权、并发CAS、事务回滚、hash篡改、精确批准/角色revision、队列/范围、超时/停止、未知恢复、预算、初始化journal、删除/GC/素材保护；含Unix symlink负例 |
| 稳定core | 6 passed / 0 failed（含RF01 3项与RF02取消/期限、原子停止、标识/文本边界3项） |
| 真实foundation进程 | 8 passed / 0 failed：保存/读取/私有交付/重启、认证/版本/seq、非成员、writer互斥、priority stop、帧拒绝、强杀恢复与stdout卡住有界退出 |
| Windows MSVC原生 | SQLite 21 + core 6 + 真实进程8 passed / 0 failed；比Unix少1个symlink专属用例，不将其算Windows证据；相应clippy -D warnings通过 |
| RF01相关回归 | bootstrap engine 6、依赖守卫10、退役profile拒绝1；未运行旧全仓套件 |
| 契约/前端 | schema --check无漂移；四种夹具Rust→TS→C++→Rust及Rust→C++→TS→Rust，含Foundation中文正文/uint64极值；TS typecheck/新入口Vite生产构建通过；Node注册表6、当前source/UI/CI15（含foundation 4）通过 |

强杀测试有1个ignored的专用worker入口，由父用例通过--ignored显式启动、观察持久dispatch意图后真实kill；它不是漏测的业务用例，也不另计通过数。其余8个真实进程测试独立计数。审计故障注入/确定性超时推进只用于测试，不提供生产假成功或测试绕过参数。

日志：.nai-diagnostics/rf02-gate.log、rf02-storage.log、rf02-engine.log、rf02-native-storage.log、rf02-native-engine.log、rf02-native-clippy.log、rf02-contracts.log。均为本机忽略产物；远程CI本轮未触发。活跃CI已扩展至上述RF02模块，不以注册表partial证据标tested。

## 过程中发现和修正

SQLite有符号整数与协议u64之间改为显式范围转换；原先停止计数混入dispatching，改为独立执行中/未知字段；收包后排队时间计入TTL，输出写线程有界，堵住stdout也能退出；SQLite progress handler的安装错误实际传播，不丢弃Result；关键审计错误明确返回IoFailed，不伪装成CAS冲突，追加Linux/Windows各3项审计故障定向复核通过。撤权审计回滚的权限漏洞用独立意图日志封闭，正文GC增加所有权及hash验证，不能扫描删除陌生素材。Windows链接出现过短暂LNK1104，复查未发现残留测试进程后重试通过，未无证据归因代理/网络或用户安装占用。

OpenSpec归档：2026-10-22-v43-rf02-foundation；主规格workbench-foundation生效。凭据扫描覆盖可发布源文件及诊断，未发现已知前缀密钥；不宣称可识别任意无前缀密钥。

## 未完成与后续

本卡不是完整V4.3产品交付：新壳仍为RF01能力检查，尚未连接通用基础命令为用户工作台；真实产品事实/不可变发布、场次/节目、普通表单与实际私有预演在RF03。长期owner生命周期/OS凭据与ACL、正式备份升级/多用户远程配对、更多币种/实际Provider对账、公开目标/媒体/P0硬件、平台与商业认证各在后续卡；没有把缺实现称外部Blocked。

RF03接一条普通用户纵向路径：默认工作区→产品及来源事实/禁说项→保存草稿→场次/节目→发布snapshot→ContentOnly私有预演→停止→重启还可读取。UI从引擎命令/读模型取数据，不直接读写SQLite，不要求用户填场景/运行配置文件。已提交历史与新交付分别展示，不恢复旧批准/租约。当前不更新安装版，待可用用户任务和维护安装条件成立后再交付安装体验。
