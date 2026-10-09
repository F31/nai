## Why
V3.6 §31.2 W06 尚无 2D 渲染实现。W05 已完成独立租约演练，下一步需要可验证的像素生产者及按音频样本时轴驱动的嘴型，作为 render-worker/共享内存接线前置。按 W06 推进，不新增 D 编号。

## What Changes
- 新增 JSON Schema avatar-2d 配置并生成 TS 契约；限定 raw RGBA8、底图/闭嘴/开嘴三层、有限画布及 PCM 缓冲、显式音频/平滑参数。
- 新增 plugins/avatar-2d 的 nai-avatar-2d 库：基于通用 avatar 清单核验素材闭包、尺寸、SHA256 和 root 授权引用；输出实际软件合成像素。
- 有界 mono f32 PCM 输入、严格 session/epoch/utterance/seq/样本连续性、逐帧 RMS/噪声门限/平滑、按样本位置生成 FrameMetadata；取消/期限失败不输出帧且不消费待渲染输入。
- 保留 worker/共享内存/Windows 窗口/设备接线为后续工作，A01 保持 implemented，软件像素测试不标真实桌面帧率通过。

## Capabilities
### New Capabilities
- `avatar-2d-rendering`: 授权分层素材准入、样本时轴 RMS 嘴型与有界像素合成。
### Modified Capabilities
无。

## Impact
修改 schemas、生成脚本及 workspace；新增一个 plugin 层 Rust 库，复用 contracts/media-core/plugin-sdk/jsonschema/sha2/serde，无新外部依赖、SDK/GPU 或常驻进程。最多 3 层/每层 1920×1080 RGBA8、约 24MiB 素材及 8MiB 当前帧，PCM 最多 96000 个 f32（约 375KiB）；每调用只生成一帧，不创建隐式队列。Windows 实时性待目标环境实测。
