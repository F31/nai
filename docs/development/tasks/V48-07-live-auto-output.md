# V48-07：实际预取与自动衔接

goal: 由可信原生 Owner 连续消费已批准讲品句，实际预取当前/下一句 TTS，并逐句通过公共音画守门执行。
contracts: LiveAuto 六类消息与五条命令，251 消息；受保护范围绑定模型、来源、费用上限及实际输出。
limits: 两槽、单次范围最长5分钟、最多128句；业务网络15秒，控制4秒，原媒体租约与挑战期限不放宽。
status: 实现及主要定向实测完成；播放中重叠预取与五项 Windows 场景已通过，独立五程序组合构建、GUI/认证启动及实际页面验证完成。
verification: 核心55/存储148/宿主6/引擎13/UI18；Windows五场景及本地Qwen两次真实合成；99互通/251生成无漂移。
environment: E盘 live-auto-build/live-auto-runtime/tool-cache 隔离；C盘旧构建缓存已哈希验证迁移，原物理副本移除。
next: 插问边界暂停/回答/原游标恢复→真人独立麦克风→连续OBS与长稳矩阵。
ACC: 164项保持pending，V48-07整卡仍进行中。
report: docs/reports/2026-10-26-v48-07-live-auto-output.md
