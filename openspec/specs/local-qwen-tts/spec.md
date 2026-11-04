# local-qwen-tts Specification

## Purpose
通过显式受限回环API配置用户部署的Qwen语音服务，提供完整音色与语言选择，并保持共享模型、凭据和媒体准入边界。

## Requirements

### Requirement: 本地语音免认证必须显式且仅限回环
系统 SHALL 使用qwen_local显式协议保存本地语音配置并发送language；免Key MUST 仅对受限字面回环HTTP端点生效，不发送远程凭据。
#### Scenario: 本地模型保存及合成
- **WHEN** 用户选择有效回环Qwen服务、音色和语言
- **THEN** 配置可保存和重读，无Key也可测试/预演，HTTP不走代理或携带Authorization
#### Scenario: 非回环或远程缺凭据
- **WHEN** 本地协议指定公网/歧义地址，或远程协议缺Key
- **THEN** 拒绝不安全本地端点，远程仍不可派发，不以假凭据就绪替代

### Requirement: 用户本机支持音色完整可选
界面 SHALL 提供本次服务实际返回的9种音色及11种语言，默认serena和中文，并保存到共享模型。
#### Scenario: 新增或重开模型
- **WHEN** 新增本地预置或编辑已保存本地模型
- **THEN** 音色/语言均可下拉选择且保存值可回读，原远程模型/Key不被覆写
