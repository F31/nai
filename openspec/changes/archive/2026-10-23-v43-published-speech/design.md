## Context
复用当前publication_private_preview的受控文本/来源/session，新增publication_tts_preview任务，不继承文字呈现回执为语音批准。
## Decisions
只读计划基于当前preview+模型生成规范化批准bundle哈希，包含actor/角色revision、preview/source/session/epoch、文本、模型/voice引用、target及TTL和本次费用上限。UI核对后提交该hash；服务端事务内重算，匹配才预留预算并持久化批准。批准限本次任务，不授权自动/公共输出。
业务任务保存不可变source引用和批准hash，文本只从可信preview派生。每次派发、完成、授权和worker challenge复检preview（事实撤销/session停止/期限/当前角色）与模型，失败丢弃PCM/停止续租，独立worker失联静音。取消不假退款；重启任务不重试、缓存为空。原生优先停止不依赖DB；停止预演/离开面板先原生停止再业务命令。
## Risks / Trade-offs
默认声卡可能被OBS桌面音频采集，明确UI提示；本轮不是公共sink许可。Submitted不授予Completed或硬件ACK；不推进节目检查点。单段256字符/20秒，过长明确拒绝不截断。
## Validation
纯逻辑/存储事务：错批准、跨workspace/版本/session/事实/模型变化、取消及重启；真实引擎HTTP断言请求正文为发布话术，撤销/停止使已合成音频不可授权。Windows静音fixture单列实际播放与撤销停止；UI协议/构建及桌面烟测；三语言契约互通。不执行付费API。
