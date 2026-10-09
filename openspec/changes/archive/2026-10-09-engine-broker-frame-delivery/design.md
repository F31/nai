## Decisions
root创建broker→keeper继承只读FD并触发UDS→声明pid验证consumer→arm→worker继承写FD并预热/触发UDS→声明pid验证producer→reserve/原子ticket→worker提交并等待finish→root接受commit、pin Reading、relay→匹配seq/epoch/buffer_ref/SHA256的keeper诊断→release→finish/回收worker。
UDS仍不提供SO_PEERCRED，仅核验声明pid/uid/FD；不称为认证连接。broker最多2个唯一role并复检授权；consumer FD只读。数据读必须复检完整pool/session/epoch/双方/stride/extent；keeper明确frame epoch匹配live rehearsal lease及允许utterance。
Worker事件由reader送入2条有界队列，owner非阻塞poll；卡读不阻塞halt/timer。bootstrap按5ms轮询持续续租/observe/共享epoch检查；250ms槽期限不延长。主错误路径先P0→撤权→critical记录→回收；Supervisor析构兜底。
keeper保留owned像素、报告consumed_buffer_ref/consumed_sha256，待机清除，output_available恒false。这是诊断消费，不是PlaybackAck/设备完成。
勘误engine-assembly archived tasks 3.2/3.3/3.4与smoke信号文字，改为实际已验证的停止续租/engine内消费/halt文件；规范增量同步边界。ADR0021记录新接线不升级ADR0018认证状态。
## Verification
真实engine+broker+keeper+worker单帧、非法资产、worker卡读/预热期间halt、HTTP撤权、只读FD与陈旧metadata回归；portable/contracts/schema/前端/engine-control-smoke与registry/归档。
