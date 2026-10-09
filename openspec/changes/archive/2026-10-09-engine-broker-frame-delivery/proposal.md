## Why
接管engine-assembly后核对代码：broker与keeper帧投递未装配，预热暂停续租、阻塞worker管道读及早退child回收存在缺口。依据V3.6 W05/W06/W10完成真实单帧工程闭环。
## What Changes
- root以FrameBroker建池，声明spawn pid；worker/keeper通过UDS触发/proc owner与池FD核验后才reserve/commit/消费，保留ADR0018对端未认证边界。
- consumer继承只读FD，keeper复检live Rehearsal lease并给出buffer_ref/SHA256诊断，不产生PlaybackAck。
- 等待期间持续续租/observe/撤权检查；有界worker事件队列与停机，错误先P0/撤权/关键记录再回收。
- 同步交接归档中的broker、信号停机和自动重签误写。
## Capabilities
### New Capabilities
无。
### Modified Capabilities
- `runtime-composition`: broker-gated keeper单帧投递与有界故障监督。
## Impact
schemas/media与生成产物、engine/runtime/worker/keeper/media-core及测试/文档。复用已有依赖，无新外部包或常驻服务。最多2个peer、2条worker通知、一个≤8MiB owned帧；池预算不变。生产IPC/Windows映射/设备认证与PlaybackAck仍待后续。
