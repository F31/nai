# V48-07：讲品队列首个切片

goal: 持久化节目单元队列，以可信公共提交回执推进检查点，安全暂停和重启恢复。
depends_on: V48-05公共提交、V48-06共同任务源上下文。
contracts: LiveRunCreate/Read/Control、LiveContext；publication_live_run严格Schema，先生成再消费。
migrations: 无表迁移，不改用户模型、Key和业务资料。
invariants: 源身份/session/epoch复检；CAS；有界32；未知在途不重播；跳过与提交分开。
verification: 存储133、纯状态1、相关UI13/Schema8、Windows IPC1及公共输出1、82互通、240消息无漂移。
status: 本切片实现及局部实测完成；V48-07整体InProgress，164ACC仍pending。
next: 长文分句/有界预取/已批准队列自动衔接→插问归位→真人独立音轨→整场OBS。
report: docs/reports/2026-10-26-v48-07-live-checkpoint.md
