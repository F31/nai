# 许可与合规

## ADDED Requirements

### Requirement: 代码与权重分开核查

系统 SHALL 分别记录 FlashHead 代码、模型权重、VAE/依赖、Qwen3-TTS、EchoMimicV3、OBS、ffmpeg 的许可证与再分发条件；SHALL NOT 因代码为 Apache-2.0 就把权重/整栈声明为 Apache-2.0。

#### Scenario: FlashHead 代码
- **WHEN** 集成 FlashHead 代码
- **THEN** 保留 Apache-2.0 许可与 NOTICE；不删除版权/许可声明

#### Scenario: FlashHead 权重
- **WHEN** 下载/打包/分发权重
- **THEN** 单独核查 HF 模型卡 `license: other` 与 VAE_LTX OpenRAIL-M 限制；未确认时限制再分发/商用

#### Scenario: LiveAct
- **WHEN** 未取得 LiveAct 书面许可
- **THEN** 不打包、不分发、不进入主线；只允许隔离研究

### Requirement: 用户素材与声音授权

系统 SHALL 要求用户确认肖像、参考音频/声音克隆、音乐/素材的使用权；SHALL 记录确认时间/来源/范围；SHALL NOT 把勾选当作法律证明。

#### Scenario: 未确认肖像
- **WHEN** 用户导入肖像但未确认授权
- **THEN** 系统拒绝开始直播，显示原因

#### Scenario: 声音克隆
- **WHEN** 用户导入参考音频用于克隆
- **THEN** 系统要求确认说话人授权，并记录；未确认不使用

### Requirement: 许可门与发布

系统 SHALL 在发布/商用前检查许可清单；未知/未确认 SHALL 阻断发布或锁定商用开关；SHALL NOT 以“官方 README 写了 Apache”替代权重/依赖核查。

#### Scenario: 发布检查
- **WHEN** 生成发布候选
- **THEN** 许可清单每项有结论/证据/适用范围；未确认项阻断

#### Scenario: 许可变化
- **WHEN** 模型/权重/依赖版本变化
- **THEN** 旧许可结论失效，重新核查后才更新发布清单

### Requirement: 合规输出边界

系统 SHALL 只做本地输出/OBS 手动直播；SHALL NOT 自动开播平台、绕过平台审核、伪造平台授权或使用第三方未授权采集。

#### Scenario: 平台开播
- **WHEN** 用户要求自动开播抖音/快手/视频号
- **THEN** 系统说明未实现/未授权，只提供 OBS/手动输出

#### Scenario: 第三方弹幕
- **WHEN** 用户接入外部弹幕采集器
- **THEN** 系统只接收通用 HTTP 事件；合规责任由用户承担，不内置平台签名/Cookie
