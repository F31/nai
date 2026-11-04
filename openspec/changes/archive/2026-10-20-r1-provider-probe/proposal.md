## Why
独立主备配置已交付，但目前只能探测当前LLM，操作者无法从界面验证指定备用与TTS。按V3.11 API Provider与显式成本授权约束补齐配置后的单项验证用户路径（A02/B03证据，不升级认证）。
## What Changes
- 接入用户提供的Agnes与SiliconFlow非机密配置，补SiliconFlow显式采样率协议。
- 每个已保存条目提供显式付费连通测试，精确命中所选模型/音色，不走故障转备。
- LLM校验非空文本，TTS校验实际PCM格式与数量；有界期限、单次派发、不播放。
- 独立Protobuf报告，逐条状态/耗时/错误码及usage，不覆盖全场景预检报告。
## Capabilities
### New Capabilities
### Modified Capabilities
- `api-provider-transport`: 显式单供应商探针。
- `local-control-surface`: 主备设置中的逐条测试与结果。
## Impact
影响control.proto及生成契约、engine、原生壳/权限与设置UI；复用现有依赖，无常驻服务。未提供真实密钥时只做协议集成，不发付费请求。
