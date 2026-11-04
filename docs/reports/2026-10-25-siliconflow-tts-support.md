# SiliconFlow 语音接口核查说明

可复制给硅基流动官方支持；不含API Key。时间均为UTC+08:00。

前两次使用已保存配置进行直连/系统代理对照；第三次另经用户授权，使用本轮提供的凭据和独立客户端。每次各一次，无自动重试：

| 路线策略 | 客户端派发时间 | X-Trace-Id | 结果 |
| --- | --- | --- | --- |
| 禁用HTTP代理直连 | 2026-10-25 16:37:23 | 90d5bf70-355b-452b-9ef7-10c05b30bbb4 | 45.277秒未取得HTTP响应头，客户端超时 |
| 系统代理默认策略 | 2026-10-25 16:38:09 | 66c04297-01f4-4cd1-a447-2c0492f525a6 | 45.159秒未取得HTTP响应头，客户端超时 |
| 另行授权单次：独立Windows HTTP/1.1禁用HTTP代理 | 2026-10-25 17:10:14 | 36594de3-3fde-4e4e-809d-21c5f65a1423 | 60.066秒未取得HTTP响应头，客户端超时 |

请求：POST https://api.siliconflow.cn/v1/audio/speech，Bearer认证（Key不在本说明提供），Content-Type application/json。

```json
{"model":"FunAudioLLM/CosyVoice2-0.5B","voice":"FunAudioLLM/CosyVoice2-0.5B:alex","input":"你好，这是语音合成测试。","response_format":"wav","sample_rate":24000,"stream":false}
```

客户端未取得HTTP状态码/供应商返回的追踪头；上表为客户端主动发送的X-Trace-Id，请先确认是否收到，不代表已确认请求抵达。三次均没有取得可用音频，费用状态未知。第三次使用独立Windows客户端及用户本轮提供的凭据，不经过工作台/Rust传输；此前两次与第三次分别有明确授权，均未自动重试。

请帮助核查：

1. 是否收到这三个X-Trace-Id，分别停在认证、模型排队还是生成阶段？
2. CosyVoice2 alex当前服务是否可用，是否存在排队/故障？
3. 三次请求是否实际生成或计费，服务端首次响应及结束时间是什么？

补充：无Key空请求直连/系统代理均快速返回401，只用于连通性验证。请通过官方控制台选择对应账号，不要在公开渠道粘贴API Key。
