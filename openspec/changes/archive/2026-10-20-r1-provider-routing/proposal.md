## Why
现有界面只接受一个共享模型与密钥，无法配置真实LLM/TTS，也无法在供应商故障时为后续请求选备用。按V3.11 Provider替换、预算与有界调用约束推进R1产品配置（A02/B03/S08证据，不升级认证）。
## What Changes
- 独立LLM/TTS主备列表，每组1主最多4备、音色绑定、会话密钥。
- 健康熔断、冷却后试探、优先级或延迟路由；已派发失败不自动跨供应商重发，下一请求选备用。
- 原生配置UI、宿主非机密保存、启动时注入全部密钥，兼容旧场景。
## Capabilities
### New Capabilities
### Modified Capabilities
- `local-control-surface`: Provider设置界面与安全保存。
- `api-provider-transport`: 独立能力路由与请求绑定。
## Impact
影响schema、engine、桌面与壳；无新服务，壳复用workspace已锁定jsonschema校验库。无需付费调用即可验证协议，真实账号认证独立。
