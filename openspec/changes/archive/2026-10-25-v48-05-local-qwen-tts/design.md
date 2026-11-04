foundation TTS配置/视图新增protocol、language；JSON配置protocol枚举openai_speech/qwen_local，language可选。旧远程工件不重写，空协议含义明确为openai_speech。新本地模型secret=null且credential_saved=false，用协议明确表达不需要认证，不将其伪造为凭据已保存。

共享core只校验配置规则：qwen_local地址为http://127.0.0.1:端口/v1或http://[::1]:端口/v1（可尾斜杠），拒绝公网、localhost解析、userinfo/query/fragment、错误路径和端口；language来自服务返回的11种代码。root与storage/transport复检。远程协议维持HTTPS/Key，只有已有显式测试桩入口允许回环HTTP。

本地传输强制no_proxy、不重定向、不携带Authorization，使用input/voice/language/response_format=wav/model；独立API实际返回24kHz单声道PCM16，由既有WAV校验、取消/期限及媒体路径处理。测试和业务预算语义不额外更改。

UI预置取本次用户服务/v1/voices与/v1/models的真实快照：9音色、11语言，默认serena/chinese；不编造性别/口音说明，不声称运行时实时刷新。保存模型通过原认证Owner/Store，不直接写SQLite；已有同协议/地址/模型记录则复用不重复创建。保存后做一次本地测试/私有试听路径验证，无付费供应商调用。用户外部服务的GPU/维护由既有部署承担，nai不新增常驻进程或推理依赖。
