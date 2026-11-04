# 逐供应商测试与 Agnes / SiliconFlow 实际接入（2026-10-20）

## 本轮交付

LLM/TTS主备面板逐条提供“测试此供应商（可能消耗API额度，不播放）”。只测试已保存的指定ID，固定模型/音色，不走主备重路由或重试。显示passed/failed/not_ready、耗时、稳定错误码、TTS样本数与采样率；测试后仅清空该条Key。运行中或未保存时禁止测试，根界面忙碌期间禁止启动并发操作。

独立CLI为`--preflight <scene> --preflight-connectivity --preflight-provider <id>`，缺显式组合则拒绝。单次网络≤30秒，宿主≤35秒，独立一次请求预算/usage报告不计入正式会话累计账本，密钥只入本次子进程环境；报告单独写`operator-artifacts/provider-probe.pb`，不覆盖整机preflight.json。报告包含真实usage状态，缺厂商账单时金额明确未知，不产生设备/PlaybackAck。宿主先失效旧结果、有界16KiB读取，复检版本、供应商/模型/音色/能力/状态与可选计数。

先改control.proto生成Rust/TS/C++，新增ProviderProbeReport，未手改生成代码。宿主到界面也直接传生成的Protobuf字节，界面复检版本、用量身份和边界，不另设JSON报告消息。18类互通fixture新增未知音频计数、跨语言大整数耗时与嵌套用量。新增siliconflow能力协议：复用既有有界语音传输，明确发送sample_rate/stream=false并返回实际请求格式；只接受官方支持的mono s16 PCM采样率，不沿用OpenAI固定24k假设。

界面提供“填入 Agnes / SiliconFlow 参数（不含密钥）”；用户提供的非机密场景位于`demos/manual-demo/agnes-siliconflow-session.json`。不包含任何Key，不自动代填密钥。SiliconFlow基础地址规范为`https://api.siliconflow.cn/v1`，模型`FunAudioLLM/CosyVoice2-0.5B`，音色`FunAudioLLM/CosyVoice2-0.5B:alex`；LLM保持用户指定`agnes-2.5-flash`。协议依据[SiliconFlow官方语音文档](https://docs.siliconflow.cn/docs/api/audio-speech-post)，其中显式PCM采样率与完整音色标识已核对。

## 真实服务结果

用户提供本次会话凭据。密钥未写源码、场景、报告或命令行；联网宿主通过掩码stdin读取后仅注入引擎环境，用后释放。受限Node环境首次两次调用在约3ms返回PROVIDER_UNAVAILABLE；匿名诊断确认其DNS为ENOTFOUND，不能当成账户拒绝。随后在获准联网宿主执行单次探针。

| 服务 | 请求的模型/音色 | 结果 | 耗时 | 证据范围 |
| --- | --- | --- | --- | --- |
| Agnes | agnes-2.5-flash | passed | 2528ms | 非空LLM正文；未播放、非全链认证 |
| SiliconFlow | CosyVoice2-0.5B / alex | passed | 2170ms | 24000Hz、22080个原始PCM样本；未播放 |

Agnes第一次16-token探针耗时2438ms但正文为空，保留failed/CAPABILITY_NOT_READY。输出预算修正为不超过配置的128 token。自动审批拒绝追加费用调用，向用户说明后用户明确“授权一次追加探针”，仅执行该一次并通过；不是SDK自动重试，也没有换模型。两家均未报告可用实际用量，真实usage状态保留NEEDS_RECONCILIATION，金额未知，不把0当账单。

二进制证据在`.nai-diagnostics/live-provider-2026-10-20/agnes-primary.pb`、`siliconflow-primary.pb`，首次空正文另存`agnes-primary.empty16.pb`，不含文本/PCM/Key，不提交诊断产物。没有继续运行付费全链或设备测试。

## 工程验证

本地真实协议回归验证：直接选择备用不调用主、固定语音模型/音色/采样率、异常PCM/缺Key/服务失败拒绝、不自动转备、CLI缺授权拒绝，原整机报告字节不变且未创建DB。SiliconFlow适配回归验证显式采样率与Authorization、无效采样率/双声道在任何请求之前拒绝。宿主报告身份、版本和缺省计数测试通过；前端单调用、Key清理、预设及组件测试通过。最终full复跑EXIT=0：107个Rust测试目标、1235 passed / 0 failed，Node 461 passed / 0 failed（界面117项）。18类Rust/TS/C++契约互通、schema --check无漂移通过；独立Windows壳12项lib与6项真实engine集成通过，clippy干净。

首次C++互通因fixture遗漏control.pb.h失败，补头文件后复跑；初版预算错误类型映射编译错误已修正。真实结果与本地fixture证据分开，不升级A02/B03等registry状态，只补实际生成证据。

## Windows工程安装包

最终NSIS包10,082,810字节，SHA256为76b3f3d41d4d206d5c13014f487f843c046502110eb7831570a16295563b8184，NotSigned。安装、活动进程/覆盖拒绝、随包预检、页面加载、卸载及既有3个数据文件保留均实测通过。日志为.nai-diagnostics/probe-package-final.log和probe-install-final.log；不代表正式签名、干净机或设备认证。

OpenSpec已归档为2026-10-20-r1-provider-probe；43个主规格严格校验通过，166项归档一致性3/3通过。变更尚未提交。
