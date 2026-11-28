# WSL Qwen3-TTS 1.7B 流式能力实测

状态：探测完成，当前服务不支持真正流式；改造未实施。未停止或修改现有服务。执行一次本地短句合成，无重试。

## 实际部署

Ubuntu24.04LTS，PID230运行`/root/qwen3-tts/venv/bin/python -m uvicorn api_server:app --host 0.0.0.0 --port 8000`。健康检查显示`/root/qwen3-tts/models/Qwen3-TTS-12Hz-1.7B-CustomVoice`、sdpa、24000Hz、约3.98GB已分配显存。包元数据：qwen-tts0.1.1、transformers4.57.3、torch2.14.1；这些是pip报告版本，不代表本轮验证过CUDA兼容性。

Windows直接访问localhost本轮失败，WSL内部相同地址健康检查成功；不能因此认定服务未启动。后续端到端测试须单独确认Windows→WSL路由。

## 证据

`api_server.py`的SpeechRequest无stream字段；请求先在线程锁内调用generate_custom_voice，取得完整音频后写完整WAV，再返回Response。mp3还增加完整文件转换。没有StreamingResponse、增量音频接口或请求断开取消链。

安装的`qwen_tts/inference/qwen3_tts_model.py`第732行方法先等待model.generate完成，再在第838行附近整体speech_tokenizer.decode。方法文档明确non_streaming_mode=false只模拟流式文本输入，不启用真正流式输入或流式生成。底层modeling_qwen3_tts.py先等待talker.generate，再聚合hidden_states提取全部code，不能用普通文本token streamer直接替代多码本音频输出。

探针POST /v1/audio/speech，serena/chinese，文本“你好，这是流式测试。”，response_format=wav，额外stream=true。200返回，Content-Length103724、audio/wav；服务X-Gen-Seconds12.11、X-Audio-Duration2.16；客户端首响应/首4096字节13.5573秒，末块13.5585秒。26次read仅是完整WAV的网络读取切分，不是生成流式。单次服务生成RTF约5.61，仅是本轮样本，不推广为稳定性能或归因SDPA/模型大小。

原始记录位于`target-windows/qwen-stream-probe/result.json`及同目录probe.py/probe.wav；未加入版本控制。没有第二次合成。

## 推荐改造顺序

1. 保留8000服务。先检查本机GPU、驱动及候选服务的精确版本支持，在E盘工具缓存建立独立验证环境，顺序启动候选、避免两个模型争抢显存。复用已有1.7B权重，不默认下载/改成0.6B。
2. 优先验证vLLM-Omni的Qwen3-TTS CustomVoice流式服务。官方当前文档要求stream=true、stream_format=audio、response_format=pcm及部署async_chunk=true。使用可配置独立端口；冻结依赖和模型版本。是否支持目标硬件、内存、音色与质量均需实测，不能照搬官方延迟。额外环境只作为对照，未达标不增加常驻依赖。
3. 若候选不兼容，才扩展现有推理：在音频多码本生成边界暴露完整codec帧，维护每请求AR缓存；按tokenizer实际上下文/重叠要求增量解码，只输出新增样本，处理首块/尾块和EOS。禁止反复解码完整历史后随意截尾，也不把完整WAV切片叫推理流式。维护成本和延迟收益不合算时判No-Go，不无限自研。
4. HTTP层接有界生产者队列，StreamingResponse输出明确24000Hz/单声道/PCM16LE二进制；初始块大小作为实测配置。请求断开/超时传播取消，单飞并拒绝超量排队，模型锁不阻塞health。能力接口明确实际模型指纹、流式支持、格式和取消语义；不再静默忽略stream。生产者报错和EOS总样本通过正式控制契约表达，音频数据不走JSON/Base64。
5. Rust客户端与原生播放器一起改造。现有完整WAV适配器和读完PCM/EOF才prepared的执行端均无法边收边播；改为有界PCM块环、样本范围、EOS校验、取消和短租约。公共音画仍需等待对应视频可用范围，纯语音试听可单独验证，不冒充音画通过。
6. 冷/热分别验收首可解码PCM早于整体结束、真实边收边播、块间缺口、持续RTF、断开取消、完整与流式音色/拼接质量、Windows→WSL可达性，以及与FlashHead共享GPU时吞吐。先验证1.7B真实流式，再顺序比较0.6B；模型大小不自动等于更低端到端延迟。

官方依据：[Qwen推理实现](https://github.com/QwenLM/Qwen3-TTS/blob/main/qwen_tts/inference/qwen3_tts_model.py)、[vLLM-Omni流式服务](https://docs.vllm.ai/projects/vllm-omni/en/stable/serving/speech_api/)、[Qwen语音服务示例](https://docs.vllm.ai/projects/vllm-omni/en/stable/user_guide/examples/online_serving/text_to_speech/)。上游页面可变化，实施前需固定commit与兼容依赖。

接入[整体方案](../v561-streaming-cache-optimization-plan.md)的SC-01能力基线和SC-03流式语音纵向包。当前34ACC不变；本轮仅确认“不支持”，未实施改造，未宣称候选服务通过。
