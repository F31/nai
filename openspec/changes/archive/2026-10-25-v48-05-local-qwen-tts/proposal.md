# 本地Qwen TTS共享模型与全音色选择

## Why
用户已部署本机Qwen3-TTS并实测成功，明确要求加入共享TTS模型配置及所有支持音色。现有配置/传输只接受带Key远程HTTPS，不能伪装凭据就绪。
## What Changes
- 新增显式qwen_local协议和language字段，配置/读取/测试/预演统一传递。
- 免Key仅用于该协议且限定字面回环HTTP地址，不给本地服务发送认证头，不放宽远程模型要求。
- 本机/v1/voices实测9个音色及11种语言作为预置选择快照，默认serena/chinese，用户仍可编辑配置。
- 通过原引擎保存一条共享本地模型并验证，不覆写原SiliconFlow或Key；更新运行版。
## Impact
V48-05、Provider配置与语音链路、INV不泄密/有界网络/显式能力。先schema生成后消费者，无新依赖、无表迁移，不把Python模型打进nai，不做静默远程故障降级。
