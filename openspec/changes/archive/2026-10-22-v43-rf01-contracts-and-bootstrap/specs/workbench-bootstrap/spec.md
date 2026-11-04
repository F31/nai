## ADDED Requirements
### Requirement: Bootstrap contracts are versioned bounded and authenticated
新引擎 SHALL 使用生成的版本化协议，私有IPC帧最大64KiB、小端长度、有限TTL/单次请求和认证，拒绝旧版本、未知枚举、截断/超限、错误token与越界配置，不输出凭据。
#### Scenario: Invalid peer request
- **WHEN** 收到旧wire版本、错误token或未知操作
- **THEN** 具名拒绝且不启动Provider/媒体或授予业务Ready。
### Requirement: Presentation dependencies are reported without fabricated readiness
五维配置 SHALL 从Schema生成，inspect仅报告实际能力，Prepare SHALL 明确未实现依赖；ContentOnly不得要求或启动TTS/keeper/avatar/GPU，Avatar不得因控制面在线冒充可用。新数据根不得复用旧库。
#### Scenario: Inspect a ContentOnly bootstrap
- **WHEN** 原生桌面显式读取独立新引擎
- **THEN** 返回真实控制能力与未开始业务原因，无付费/媒体/旧数据读取；缺业务保持not_ready。
### Requirement: V43 acceptance registry replaces legacy status without inheriting passes
tests/registry.yaml SHALL 完整登记V4.3第39章全部ACC与任务/INV引用，旧表归档可核验，缺号/重复/未知任务/无证据升级均失败。RF01 SHALL 只提供前置证据，不把局部bootstrap升级完整ACC。
#### Scenario: Registry drift
- **WHEN** 删除ACC、搬入旧编号/通过状态或伪造证据
- **THEN** 检查器具名失败；合法初始90项保持pending。
