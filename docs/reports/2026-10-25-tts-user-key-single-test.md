# 用户指定凭据的单次TTS测试

2026-10-25。用户再次明确提供URL/Key并要求测试一次；本轮没有修改产品代码、用户配置、数据库或系统代理，没有重新构建/全量测试。

## 实际结果

客户端在2026-10-25 17:10:14（UTC+08:00）派发一次实际HTTP请求，使用用户指定的https://api.siliconflow.cn/v1及本轮提供的Key，模型FunAudioLLM/CosyVoice2-0.5B、音色同模型alex，文本“你好，这是语音合成测试。”，WAV/24kHz/stream=false。

本次独立Windows .NET HTTP客户端固定HTTP/1.1、禁用HTTP代理、禁止重定向、连接期限5秒、整体期限60秒；没有使用工作台或先前Rust reqwest客户端。结果：60066ms后仍未取得响应头，RESPONSE_TIMEOUT，没有HTTP状态码或可用音频。费用状态unknown，不声称未计费。实际请求没有自动重发。

客户端发送的X-Trace-Id：36594de3-3fde-4e4e-809d-21c5f65a1423。编号不证明服务端收到，需要供应商查询。不能从超时认定Key有效/无效，也不能单独归因系统HTTP代理、HTTP/2或某个工作台实现；共同网络路径/TUN及供应商端等待仍未区分。

## 发送前本机问题与密钥处理

最初独立Node环境在95ms返回ENOTFOUND，没有连接或HTTP发送；随后一次内存管道连接未建立，未创建原生派发凭证。这两步没有发出合成请求。保留原始失败记录，不删除凭证后自动重试。

本机测试最终通过临时RSA公钥加密信封传递凭据：私钥仅在本机请求进程内存，信封解密后立即删除，私钥/HTTP对象在结束后释放；Key未写入磁盘脚本、命令行参数、报告或控制台输出。没有写入新的数据库凭据。残留仅无密钥诊断/授权记录和公钥，密文/临时信封均已确认不存在。

## 证据与下一步

忽略目录.nai-diagnostics/tts-user-key-once-2026-10-25内：result.json（DNS发送前失败）、native-result.json（发送前通道失败）、native-dispatch.json（实际单次派发）、windows-result.json（实际结果）；授权标记tts-user-key-authorization-2026-10-25.json，不得删除后追加请求。stdout重定向有编码显示差异，以UTF-8结果JSON为准，实际请求文本正确。

已将本次请求补入[供应商核查说明](2026-10-25-siliconflow-tts-support.md)。本次授权已完成，没有追加付费测试。需要供应商核对是否接收到请求、认证/排队/生成阶段及实际计费；TTS可用性和V48-05音画仍未验收。
