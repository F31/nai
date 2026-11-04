## Design
统一AgentTaskContext绑定workspace、snapshot id/hash、facts refs hash和域分离context hash。root从已发布且当前有效的完整闭包构造，不信任页面水位；父问题和子FAQ在读取/运行/采用时复检角色、来源、父状态及期限。

问题记录publication_question_task及publication_question_index走既有对象版本/CAS与audit。确定性上下文/操作者/120秒桶UUID定位两个版本索引对象（各最多32个问题ID），读取最多64项并按时间过滤，窗口超过32拒绝；不扫描全部审计历史；窗口包含已处理项以确保查询、去重、历史都有界。TTL不因重复延长。32条是初始产品限额，历史审计与版本保留，不自动删业务数据。

运行只进行一次本机FAQ lookup，父Running/WaitingHuman和子产物同事务提交。子文档写入不可由页面伪造的parent_task，context gate验证对应父对象/child id。取消父任务只取消未结案子建议，已采用草稿保留。任务完成、候选采用、公开播出分别显示。

Operation.child共享祖先取消标志并取父/子deadline最小值，有界深度；取消子不影响父/兄弟。真实运行使用子Operation，而非仅有脱离消费者的接口。当前root串行本机检索，不能声称可抢占所有运行中的Provider。

UI复用现有问题输入；显式加入问题池、准备或查看答案和取消。列表有到期禁用/状态提示、读取过滤窗口及上下文代次防迟到，后台不轮询模型。后续V48-07再添加真实session/epoch、当前产品/段落与主持ownership水位。

私有问题取消在同一事务提交父状态、子候选结案和审计；审计失败返回未确认且回滚，不冒称已取消或自动重试。它不是P0静音通道。已采用草稿不回滚。
