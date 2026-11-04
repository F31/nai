# 文本响应展示与TTS真实对照

日期2026-10-25，V48-05增量，ADR0172。用户确认LLM模型测试成功，要求同时明确返回正文；TTS持续RESPONSE_TIMEOUT。用户明确授权最多2次真实对照，并关闭工作台。

## 工程交付

- 文本测试既有正文输出新增“模型响应内容”标题和独立区域，继续校验成功状态/响应类型/正文，不发额外请求，不输出原始HTTP。
- TTS内部trace_id绑定现有任务UUID。仅SiliconFlow域名/显式本地测试附加X-Trace-Id；UI显示请求编号，未返回响应头时也可由供应商按客户端编号核查。[官方接口文档](https://docs.siliconflow.cn/docs/api/audio-speech-post)说明该头用于跟踪；编号存在不等于已证明抵达。
- debug专用路线通过原生Owner给单个root子进程设置，正常启动显式移除覆盖；只影响TestTts，业务预演仍默认路线，无页面路由字段，不改全局代理/数据库模型。
- 一次性诊断程序仅通过认证root读取已保存模型/凭据，不取出Key；全局授权账本及每路派发记录在调用前create_new+sync_all。最多直连/系统代理各一次，未知也不重试；诊断示例release拒绝执行。

## 实际对照：两次均失败

同一保存的模型版本1：FunAudioLLM/CosyVoice2-0.5B，音色FunAudioLLM/CosyVoice2-0.5B:alex，base URL https://api.siliconflow.cn/v1。固定文本“你好，这是语音合成测试。”、WAV/24kHz、stream=false，只有HTTP代理策略不同。

| 路线策略 | 客户端耗时 | 结果 | 客户端发送的追踪编号 |
| --- | --- | --- | --- |
| direct/no_proxy | 45277ms | Failed / RESPONSE_TIMEOUT | 90d5bf70-355b-452b-9ef7-10c05b30bbb4 |
| system_proxy默认 | 45159ms | Failed / RESPONSE_TIMEOUT | 66c04297-01f4-4cd1-a447-2c0492f525a6 |

两次没有响应头、没有有效音频、未试听，charge_status均unknown。本次2次授权已用完，没有第三次、没有自动重试。未知费用不当作0，不释放旧费用记录。独立工作区6f47caa0-b3ad-4c41-87c9-bd91f791cd32，名称“语音路径诊断（演练，不用于直播）”，使用零本地预算验证测试免预留语义；模型配置未修改。

这两次结果不能把问题单独归因系统HTTP代理，也不能排除操作系统/TUN/共同网络路径、客户端协议或供应商模型处理。无Key401探针和Agnes文本成功都不能代表SiliconFlow认证后的语音生成成功。继续延长期限不是已证实的修复；需要供应商核对是否收到Trace ID、认证/排队/生成阶段和实际计费。

[可直接提交的供应商核查说明](2026-10-25-siliconflow-tts-support.md)仅准备文本，没有代用户发送邮件/工单。

## 验证及部署

- 本地TTS传输6/6：包含真实HTTP头Trace ID、严格音频校验、期限/取消/无重试；首轮夹具漏设trace_id，修正夹具后通过。
- UI4/4：真实React渲染明确正文、无费用控件、失败提示；类型检查/构建通过。
- 相关engine/host/provider all-targets clippy -D warnings通过；Windows原生TTS成功元数据1项回归通过（本地服务，不作为付费证据）。
- 引擎/诊断例程/桌面壳构建及GUI/认证IPC smoke通过。用户关闭后统一更新，完成后重开独立工作台；数据/Key/历史费用/系统代理/旧安装不改。
- 无跨进程Schema变更，无新依赖；不重跑全workspace，不修改164ACC。真实Provider两次失败与本地工程测试通过明确分开，V48-05音画仍未闭环。

## 本机证据

.nai-diagnostics/tts-route-authorization-2026-10-25.json为本次授权账本，禁止删除后重跑；tts-route-live-2026-10-25包含setup、两路dispatch/result JSON，不包含Key或PCM。其他日志：model-catalog-native-tts-route-{provider,clippy,bins,native}.log、model-response-ui.log、model-response-smoke.log。大产物/实际数据根不提交。
