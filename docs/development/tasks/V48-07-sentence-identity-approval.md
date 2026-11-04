# V48-07：分句身份与批量文本批准

goal: 贯通发布源→句预演→TTS父来源→音画候选→root提交检查点，提供可撤销整场文本范围批准。
contracts: LiveSentencePreview、LiveBatchApproval、LiveBatchControl，245条消息；JSON Schema新增预演身份及受保护批准字段。先生成再改消费者。
limits: 当前/下一句准备窗口2，128句/32KiB，文本批准5分钟/已用ID最多32个；实际媒体仍500ms租约且逐句复检。
migrations: 无表迁移；旧单元队列保留但拒绝执行，需要明确结束重建；不自动授予新权。
status: 工程交付及定向实测完成；自动预取/连续播出、插问归位、真人接管仍待装配。
next: root范围消费授权（模型/音色/费用/输出目标）与有界TTS预取/自动衔接→问题池中断及归位→真人独立音轨→OBS整场。
ACC: 164项保持pending。
report: docs/reports/2026-10-26-v48-07-sentence-identity-approval.md

verification: 核心54/存储141/UI21、Windows IPC及公共输出回归各1、87条互通/245消息无漂移；真实Qwen后续隔离业务两句通过，首次超时证据保留。

