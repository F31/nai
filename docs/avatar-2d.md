# 2D 软件渲染核心（W06 前置）

`plugins/avatar-2d` 提供 `nai-avatar-2d` 库：实际 RGBA8 分层合成、按样本时轴计算的 RMS 嘴型，以及有界/可取消的帧生产。
当前是可嵌入核心，尚无生产 `apps/render-worker`、共享内存传输、宿主 capability 注册或 Windows 窗口/设备；不授予 Ready/播放权限。

## 准入与接线

1. root 安全读取形象包并核验路径/凭据/授权，传入不可变 `AssetBytes`。库不打开清单路径、不解码图片；输入为 raw RGBA8。
2. `Avatar::admit(ctx, manifest, config, assets, grants)` 校验既有 AvatarManifest 与 `schemas/config/avatar-2d.schema.json`：三层精确闭包、尺寸/位置、SHA256、独立许可 grants。
3. 将 Avatar 包装为 Arc，通过 `avatar.stream(ctx, cue)` 创建单句 RenderStream；cue 的 session/epoch、期限和 canonical hash 必须一致。
4. `stream.push(ctx, grants, block, pcm)` 输入 mono f32；`next_frame(ctx, grants, buffer_ref)` 最多返回一个 `RenderedFrame`。root 为真实 buffer 注册并提供引用，二进制 rgba 必须进入有界媒体数据面，不经 JSON/Base64。
5. keeper 在消费时仍须复检 lease/session/epoch/审批对象哈希/事实版本/guard。生成像素或 FrameMetadata 不代表 commit/实际播放完成。

清单绑定 `model_binding=software-rgba8-v1`，native 与 output 必须同尺寸。
配置必须显式声明 base、mouth_closed、mouth_open 三个唯一角色及不同 asset_id；底图覆盖整幅画布，嘴部两层尺寸/位置一致。
支持 idle/speak 动作声明、neutral 情绪；其它动作/情绪或 cue 的不支持表演参数显式 CapabilityNotReady。
许可字符串只是引用，每次 push/render 与 root 的最新 grants 比对；真实授权来源由 root 独立核验。

## 时轴与资源

- 画布上限 1920×1080，1–60 fps；每层 bytes 严格为 width×height×4。
- mono f32、8000–96000Hz、幅值[-1,1]，拒绝 NaN/Inf；seq/sample_start 从0开始并严格连续，EOS之后不再写入。
- 缓冲配置最多96000个样本且至少容纳一帧窗口；整块超限拒绝，不覆盖旧 PCM。调用方须限制同时持有的流与返回帧。
- 第 n 帧起点为 floor(n×sample_rate×fps_den/fps_num)，使用对应样本 RMS；不同网络分片得到相同窗口/嘴型/像素，不把包到达时间用作内容时间。
- noise_gate/full_open_rms、attack_ms/release_ms 必须显式指定；按帧样本时长指数平滑并在闭嘴/开嘴纹理间插值。它是音量驱动嘴型，不是音素级口型。
- PTS 为样本起点、时间基1/sample_rate；EOS不足整帧时产生短尾帧，累计duration等于输入样本数。
- 元数据/配置与 cue 限64KiB。逻辑载荷最多约24MiB三层素材、8MiB当前帧、375KiB有效PCM；容器capacity/分配器和root输入开销不计入该载荷数值，RSS/1080p实时性未实测。
- 逐行/逐块及提交前检查 context，失败不消费 PCM、不推进帧号/平滑状态；epoch/许可/取消/期限失效封闭该流，不能用新 context 恢复旧流。

## 验证

```bash
cargo test -p nai-avatar-2d --offline
cargo xtask check --profile portable
cargo xtask schema --check
cargo xtask test --suite contracts
```

9项测试覆盖素材/hash/授权/闭包/参数拒绝、透明度参考像素、噪声门限/攻击/释放、非整数fps与分片不变性、EOS尾帧、PCM格式/序号/过载、epoch/授权撤销/取消/期限封闭。
测试素材仅在 tests 内构造，不作为发布素材或认证模型。A01保持implemented，Windows设备/桌面帧率未执行；见 [实测报告](reports/2026-10-09-w06-avatar-2d-core.md)。
