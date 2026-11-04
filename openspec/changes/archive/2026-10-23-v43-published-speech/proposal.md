## Why
固定短句试听已有真实设备路径，已发布业务单元尚不能受控合成/点播。补齐单段私有业务语音的批准、预算、当前事实及会话撤销闭环。
## What Changes
- 源契约新增只读语音计划与显式批准合成，业务语音任务独立类型。
- 计划绑定preview哈希/session epoch/发布引用/角色/模型及voice版本/私有目标/有效期，审批hash先持久化再派发。
- 共用有界TTS执行/预算/取消/缓存/原生播放器；派发、结果、每次租约复检业务source，停止/紧急撤销后不可重播。
- 预演页面默认选择可用TTS，显示费用计划、批准合成与播放/停止；页面打开/检查不计费，不自动播放。
## Capabilities
### New Capabilities
- `published-speech`: 已发布单元私有语音批准、合成及撤销闭环。
### Modified Capabilities
## Impact
schemas/workbench/content、唯一writer、TTS lane、宿主allowlist及预演UI。无旧库导入、付费调用或公共音画认证；ACC保持pending。
