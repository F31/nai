# 本机问题任务契约与单槽调度内核

## 已完成

完成活动规格v561-local-waiting-reply的L03及L04a。schemas/v561/live_session.proto新增ResumeIntent、QuestionTask、QuestionResult：问题/结果以task_id与generation绑定，恢复意图包含计划SHA256、内容代次、从零起算的句游标与总句数；budget_ms在root准入时起算。消息本身不授予播放或恢复权限，未扩现有控制Request/Command，不能直接从UI宣称可用。

正式cargo xtask schema生成Rust/TS/Python/C++，325消息，随后schema --check无漂移。未手改生成文件。首次受限启动protoc遇Windows DLL重定位623；原生重跑protoc3.21.12正常，随后发现root Node依赖解析缺失。建立node_modules到E盘既有tool-cache/workbench-frontend/node_modules的本地目录联接后生成通过；未下载、安装或覆盖依赖，缓存仍在E盘。该故障不归因GPU或业务代码。

apps/workbench-engine/src/v561_question.rs实现独立调度内核，无Tauri、网络、数据库或模型依赖：

- 一个问题槽，Queued→Generating→Speaking→Resuming；满槽拒绝，不创建无界队列或第二模型实例。
- 问题与回答文本有界，代次/任务身份精确匹配；迟到旧结果不能清空较新的问题。
- 子调用仅获得剩余budget，不重新领完整期限；到期、显式取消、内容换代/过期、计划身份变化、worker失联撤销恢复意图。
- Authority是root必须重新提供的实际观察；每次结果/恢复转换都复检，另提供idle observe以在无模型回调时撤销，不依赖网络线程结束才取消。
- Cursor仅在完整设备样本回执且当前句序号匹配时前移；部分播放、乱序、重复、零样本或超量拒绝。半句被中断时仍指向该句，从整句重播；所有句完成后没有恢复意图。

此处“root-owned”是内核的所有权设计，生产v561_live_control尚未实例化或消费它。没有声称已将单槽、取消或游标接入用户的实际播放。

## 检查

- cargo test -p nai-workbench-engine --lib v561_question --offline：7 passed，110 filtered。
- cargo test -p nai-contracts v561_live_validation --offline：4 passed（其中新增1项问题/结果/游标校验与Rust编解码），其他集成目标0匹配不计通过数量。
- tests/v561/question-task-codec.test.cjs：1 passed，真实TypeScript编码→Python解码/回填结果→TypeScript解码，包含大于JS安全整数的uint64身份与真实中文；不是检查生成文件存在。
- cargo clippy -p nai-workbench-engine --lib --offline -- -D warnings通过；增量缓存硬链接失败回退复制的提示保留。
- 正式schema --check、OpenSpec strict、34ACC注册表检查通过。C++仅生成，未声称本轮C++编译或四语言执行互通完成。

仅匹配组件检查，未跑全量、未调用LLM/TTS、未开GPU/设备/OBS、未更新运行版。34ACC仍pending，M0不升级，活动规格不归档。

## 下一项明确入口

L04b将该内核装配进生产root：为当前已备计划生成真实身份和内容代次，由实际每句设备完成回执更新游标；插问优先撤销当前播放执行权但保留原稿恢复意图，与操作员Stop清除意图区分。准备好回答后交单一执行链消费，回答完成再复检原稿并按游标继续。不能通过事后读取任意JSON或UI自报完成推进游标；不能用旧资产的epoch恢复播放。

L05再接当前配置LLM的问题来源，L06再做Windows未缓存问题→生成→播放→自动归位/取消/失联及持续混合负载。当前没有新UI按钮或可点击问答入口，现有Start仍是直接讲解文本，不冒充AI回答。
