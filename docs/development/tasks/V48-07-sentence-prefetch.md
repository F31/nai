# V48-07：分句计划与两槽预取基础

goal: 发布原文确定分句及可取消的有界预取机制，为自动衔接提供可验证前置。
contracts: LiveSentence、LiveSpeechPlan、ReadLiveSpeechPlan；先生成再改消费者。
limits: 32单元/128句，每句120 Unicode字符，计划原文32768 UTF-8字节；预取固定2槽。
status: 本切片实现；实际TTS/播放器预取及自动连播未接线，整卡InProgress。
verification: 核心53、存储136、UI9、242消息生成无漂移及相关clippy。
next: 分句身份贯穿业务链→明确批量批准→root有界预取与自动衔接→插问归位→真人独立音轨。
ACC: 164项全部保持pending。
report: docs/reports/2026-10-26-v48-07-sentence-prefetch.md
