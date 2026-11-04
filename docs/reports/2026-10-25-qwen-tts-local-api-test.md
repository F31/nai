# 用户本地Qwen TTS接口测试

2026-10-25。用户明确指定已自行部署的本地API，要求验证可用性。本轮仅请求本机服务一次，无付费供应商调用、无产品代码/数据库/Key修改。

请求POST http://127.0.0.1:8000/v1/audio/speech，禁用HTTP代理、不跟随重定向；JSON为input“你好，世界”、voice“serena”、language“chinese”，无认证头。

结果：HTTP200，Content-Type audio/wav，耗时2655ms，76844字节。RIFF长度及fmt/data闭包检查成功；PCM格式码1，fmt16字节，24kHz、单声道、16bit，音频data76800字节，38400样本，时长1.6秒。峰值13632、RMS约1771.80、35145个非零样本，不是静音文件。未作主观内容/音质、真实设备或OBS认证。

本机证据位于忽略目录.nai-diagnostics/qwen-tts-local-test-2026-10-25：result.json、audio-check.json、out.wav。可供用户试听，不提交音频产物。

结论：本地独立API可返回符合现有24kHz单声道PCM16媒体格式的音频。当前工作台仍限制生产HTTP/免Key，且请求未发送language；不能仅填URL即宣称完成集成。后续如用户要求接入，应新增显式本地API Provider契约与本地地址/无认证边界，保留默认远程HTTPS/凭据限制、取消/期限/守门；不作为静默回退或将Python推理打入nai安装包。当前不升级V48-05或164ACC状态。
