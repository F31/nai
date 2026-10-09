## Context
复用 V3.6 §26.3/26.4/26.7 与现有 AvatarManifest、FrameMetadata、PluginContext、AudioBlock。当前没有生产 render-worker、数据面共享内存或 Windows 输出设备。

## Decisions
- 软件 RGBA8 合成用于首轮确定性验证；不引入图像解码器、wgpu/winit、模型或 Python。仅支持底图与位置相同的闭嘴/开嘴纹理，用 RMS 平滑强度插值嘴部后做 straight-alpha source-over 合成。
- 配置单独 JSON Schema；通用 avatar 清单保持不变。先生成 TS，再实现消费者。库只接受 caller 已安全读取的不可变素材 bytes，不解析 filesystem path；路径声明仍需规范化，root 负责文件/权限隔离。闭包必须精确匹配三层及声明 assets，不允许漏项/额外/重复。
- native/output 必须同尺寸、上限1920×1080；fps 比率 1–60；采样率8000–96000、mono f32、幅值有限且≤1。最多96000缓冲样本，不溢出扩容；满额拒绝本块，不丢旧数据。
- 素材许可引用由独立 root grant 集合显式确认，渲染每次复检；清单/license 字符串或哈希通过不授予 Ready/输出权限。真实许可审批及授权来源仍由 root 负责。
- PCM seq 从0开始且连续、sample_start 从0开始；每帧使用 floor(n×rate×fps_den/fps_num) 的窗口，不受块到达时间影响。EOS 支持最后不满一帧的尾块；分块方式改变不改变帧/嘴型。RMS gate 后按 attack/release 的样本时长指数平滑，不声称音素级口型。
- next_frame 每次返回至多一帧；事务式计算，最后一次 context check 后才推进 PCM/frame_seq/平滑状态；逐行及逐块检查期限/取消。FrameMetadata 使用1/sample_rate时间基、原样session/epoch/utterance、显式caller buffer_ref；RGBA 返回为独立 owned bytes，不经 JSON/Base64。元信息不表示提交或播放 ACK。
- 插件只提供可嵌入的渲染核心，不注册虚假的完整 avatar.render 宿主能力或 Ready。未来 composition root 装配独立 worker，数据面由共享内存传递，keeper 继续复检租约/审批/事实。

## Verification
素材哈希/授权/闭包/尺寸/角色/参数拒绝；混合透明度像素；RMS静音/低噪声/开嘴与释放；PCM分片不变性与非整数fps样本映射；尾块/序号/epoch/取消/过期/过载不推进状态。cargo test -p nai-avatar-2d --offline、portable、contracts、schema --check、前端类型检查/build、registry 和 OpenSpec strict/归档。

## Boundaries
A01 仍 implemented；Windows窗口/实际设备/桌面帧率为未执行（Blocked 环境验收），worker/共享内存生产接线未实现。没有商业素材或素材授权时显式 CapabilityNotReady/PermissionDenied，不生成假替代形象。
