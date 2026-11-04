## Why
本机播放器把全部复制后固定等待220ms当作submitted，没有尾部回调证据，也没有给宿主的状态绑定本次媒体。自动讲品不能据此判断完成。
## What Changes
- 新增绑定媒体摘要的尾部回调排空诊断契约。
- 用后续真实回调越过预计尾部时刻替代固定等待，撤权/到期/错误不产生完整排空证据。
- 宿主要求精确媒体身份、预期样本数与排空计数一致，报告保留证据。
- 保持actual_local_playback_completed=false，不自动推进节目或写硬件ACK。
## Capabilities
### New Capabilities
无。
### Modified Capabilities
- `resident-local-output`: 有界尾部回调证据和宿主绑定。
## Impact
media.proto及生成代码、media-core否决检查、keeper回调队列、engine监督和定向测试。无新依赖/DB迁移/付费调用/安装替换，验收状态不升级。
