# V48-07：插问回答后归位

goal: 当前句边界暂停→已发布FAQ实际回答→原未播句恢复，不把回答记为原稿进度。
contracts: LiveQuestionControl、LiveAutoView插问状态；受保护插问和预演来源Schema，252消息。
limits: 单个在途插问、15秒、120字原文、同范围最多16个问题；实际音频缓存仍两项，不放宽网络/控制/媒体期限。
status: 工程实现/Windows定向实测/运行组合更新完成；真实本地Qwen首次合成TTS_FAILED，实际语音整链Blocked。
verification: 存储152/核心56/宿主6/UI12通过，最终10项存储/6项宿主和clippy、252生成无漂移/101互通、Windows六场景及播放中检索归位用例通过；GUI/认证启动通过。真实Qwen首个请求TTS_FAILED，无自动重试。
next: 真人独立麦克风接管→连续OBS和长稳矩阵。
ACC: 164项pending，V48-07整体进行中。
report: docs/reports/2026-10-26-v48-07-question-return.md
