# ADR0159：公共候选只读准备检查

日期：2026-10-24。状态：采纳，V48-05工程增量。

公共批准前先呈现可审阅的真实来源和资源状态。页面只引用既有preview和语音task身份/hash；foundation的Store重新核验角色、会话epoch、快照来源及语音任务source绑定，TTS lane从实际有界PCM缓存重新计算SHA256和样本元数据。正文与事实hash由数据库来源计算，页面不得自报媒体、guard或公共权益成功。

检查不申请试听租约，不延长缓存寿命，不发起API调用，不自动播放，不写公共批准。返回Protobuf只读视图而非PublicMediaBundle或许可。没有生产形象许可、公共guard与媒体输出时逐项not_ready，approval_available=false；即使私有语音来源和PCM均通过，仍不能授予公共批准或live_ready。

本轮完成真实来源/PCM观察及界面入口，尚未完成生产公共权威观察器全部轴、公共批准UI、媒体数据通道与OBS验收。V48-05及164ACC状态不升级；不能将准备检查称为公共播出闭环。
