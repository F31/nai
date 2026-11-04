# Proposal

## Why

批准候选只能写预览文件，原生后端只能播放测试音。缺少操作者从同一批准文本发起实际本机音画输出的路径。

## What Changes

- 预览命令增加显式 local_playback；界面保留文件预览，另设本机播放动作。
- 引擎写出有界原始PCM/RGBA，经私有继承Protobuf控制向隔离keeper签发/刷新短租约。
- keeper重新检查完整审批/哈希/事实/期限，播放最多5秒并显示真实渲染帧；撤权、EOF、超时失败关闭。
- 状态只报告实际设备提交，不产生硬件PlaybackAck。非Windows或缺侧车在TTS前明确拒绝。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `local-control-surface`: 批准候选的显式本机播放。
- `media-basics-timeline`: 隔离keeper媒体提交与短租约。

## Impact

先修改OpenAPI/Protobuf并生成消费者。影响engine、keeper、storage审批读取和桌面。复用现有Windows依赖，无新常驻服务。正式IPC身份/签名/硬件完成认证不在本轮，不提升registry状态。
