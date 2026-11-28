# CosyVoice3 部署初查

用户已移除Qwen-TTS，安装Fun-CosyVoice3-0.5B，提供地址http://127.0.0.1:50000/。后续TTS接入以新服务为目标；旧Qwen报告仅为历史，不继续安装Qwen流式候选。本轮只读探测，未修改服务、模型配置或执行合成。

WSL内50000由docker-proxy监听，容器cosyvoice3-api在/workspace/CosyVoice运行runtime/python/fastapi/server.py，进程参数model_dir为pretrained_models/Fun-CosyVoice3-0.5B。这里确认的是进程配置，未独立验证权重哈希。另有CosyVoice网页容器；不据此判断双份权重是否已占用GPU。

WSL内部openapi.json成功；Windows本轮直接访问127.0.0.1:50000连接失败。需要单独确认Windows→WSL转发；未修改系统网络或防火墙。

实际API为表单接口/inference_sft、/inference_zero_shot、/inference_cross_lingual、/inference_instruct和/inference_instruct2，不是/v1/audio/speech的JSON协议。zero_shot需要tts_text、prompt_text和prompt_wav；不能直接沿用Qwen音色名。当前没有执行真实合成或确认可用预设音色。

运行容器源码使用StreamingResponse，内部inference_*调用未传stream；cosyvoice/cli/cosyvoice.py对应默认stream=False。因此HTTP是生成器输出，但未启用模型块级流式；文本前端可能按句生成，不能据此称单句实时音频流。输出为numpy int16原始音频，没有WAV头，实际采样率须读取模型配置，不照搬Qwen参数。

下一步：确认Windows路由→核对合法参考音频或可用预设音色及实际采样率→给服务增加显式stream参数并传递到模型→单次真实首块/末块/音质/取消探针→定义CosyVoice独立Adapter契约并接Rust有界PCM播放。任何默认模型和架构映射变更先更新OpenSpec/ADR；目前不宣称新服务已集成nai或性能通过，34ACC不变。
