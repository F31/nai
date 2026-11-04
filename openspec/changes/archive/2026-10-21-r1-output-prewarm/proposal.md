## Why
节目启动前目前必须先调用LLM/TTS并播放首句，才得到合法数字人待机窗口和设备检查。需要独立的静默预热入口，避免用伪候选/伪审批启动设备。
## What Changes
- 主界面增加输出预热按钮；无Provider调用，打开合法闭嘴待机与一条静音音频stream。
- 引擎复用已有素材准入与常驻keeper，独立Protobuf中性媒体描述符/命令，不签发语音租约。
- 会话/epoch/截止时间/权限复检、状态确认与有界失败回收；后续真实审批播放复用同一进程。
## Capabilities
### New Capabilities
无。
### Modified Capabilities
- `resident-local-output`: 无语音的显式静默预热。
## Impact
V3.11 §5.7/§8.4/§10.1，registry A01/R01/R03相关，不新增D编号/依赖。影响私有Protobuf、HTTP、keeper/engine/UI及直接消费者。不是连续讲品调度、硬件完成ACK、真人麦克风或OBS验收。
