# ADR 0173：用户部署的本地Qwen TTS API

日期：2026-10-25；状态：采纳。

用户要求将已有本机Qwen服务加入共享TTS配置，并可选择全部支持音色。仍采用API边界；不打包Python推理、不启动本地推理常驻进程、不作为远程供应商故障自动回退。

Schema先新增protocol/language后生成消费者。qwen_local显式协议仅允许字面回环HTTP（127.0.0.1或[::1]、有效端口、/v1路径），拒绝DNS别名/公网/凭据/查询片段；本地传输强制no_proxy、无认证头、不重定向。远程openai_speech保持HTTPS和凭据要求。local的credential_saved保持false，通过协议表达无需Key。

本地配置使用既有共享目录/版本、任务取消/期限、WAV校验和私有缓存授权；不修改公共输出守门、事实版本、审批及媒体租约。UI预置来自本次/v1/models和/v1/voices真实响应：9音色/11语言、默认serena/chinese，是快照而非动态发现。

用户原服务已有GPU资源/运维成本，由用户部署承担，nai不新增推理依赖。保存由认证Owner调用Store完成，不直接编辑用户SQLite。真实保存配置合成及缓存授权成功不等于设备听感、公共播出或OBS验收。