# 形象视频导入

## 结果

工作台已接入合法 MP4 选择、导入、容器头预处理、使用权确认、私有源预览、有界检查缓存和撤权。草稿种类为 `avatar_video_draft`。写实驱动字段恒为 `model_missing`。预处理只走 ISO BMFF `ftyp` 与 `mvhd`，上限 32 MiB、1 秒至 120 秒、品牌 isom/iso2/mp41/mp42。加密盒 `encv`/`enca`/`sinf`/`tenc`/`senc`/`schm` 拒绝。私有预览交付已准入源视频，媒体类型 `video/mp4`。检查缓存按 sha256 与 `mp4-ftyp-mvhd-v1` 存放，命中仍与当场检查一致才采用。撤权丢掉缓存，源字节保留。备份清单允许 `avatar-videos/{sha256}.mp4`，不复制检查缓存。2.4.1 已勾选。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。

## 选择、预处理、预览与撤权

桌面命令 `workbench_avatar_choose_video` 要求勾选使用权后才打开文件选择。通过检查后写入 `avatar-videos/{sha256}.mp4`，并安装到私有 `PreviewSlot`。`workbench_avatar_video_preview` 只服务未撤销的 `avatar_video_draft`，时长、检查版本和 `realistic_drive` 必须与存储一致。形象页用 HTML5 `<video>` 播放该源。导入路径不调用 `CharacterWorkbench` 生成。公共输出后端仍为 `builtin-2d`。

存储侧 `video_sha256`、`duration_ms`、`inspect_version`、`realistic_drive` 不可改。已撤销记录不能取消撤销。首次即以撤销写入时跳过源文件并丢掉缓存。永久清空工作区时，仍引用该草稿的视频源会挡住清空。迁移备份复制源 MP4 并核对时长区间；清单校验拒绝 `avatar-video-cache/` 和错误扩展名。

准备契约把 `realistic_drive` 钉成常量 `model_missing`，生成成功形状无法表示。

## 校验

`cargo test -p nai-avatar-files --offline --lib`：10 项通过，1 项忽略（本地用户照片探针）。其中视频头限制与损坏拒绝、检查缓存与撤权各 1 项。`cargo test -p nai-runtime-core --offline --lib admitted_avatar_video`：1 项通过，路径字段和 `ready` 驱动被拒绝。`cargo test -p nai-foundation-sqlite --offline --lib video_`：备份保留源 MP4、清空被引用挡住、缓存不进备份 2 项通过。`cargo test -p nai-workbench-engine --offline --test foundation real_ipc_private_video`：真实 IPC 1 项通过。嵌套工作区 `cargo test --offline --bin nai-workbench-desktop video_preview_is_private`：私有 MP4 预览 1 项通过。`cargo clippy -p nai-avatar-files -p nai-runtime-core -p nai-foundation-sqlite --offline -- -D warnings` 通过。`tsc --noEmit -p apps/workbench-desktop/tsconfig.json` 通过。本轮未打开实际桌面窗口点击文件选择。

备份准备在写入清单前按 `model-migration-backup` 的 `files.propertyNames` 校验路径。该模式现含 `avatar-videos/[a-f0-9]{64}\.mp4`。

## 仍开放的项

2.4.2 虚拟摄像头与 OBS 信号、2.4.3 外部渲染服务仍开放。2.5 与 3.x 仍开放。1.3.1 写实样片仍开放。当前没有生产写实渲染器。视频草稿停留在 MODEL_MISSING。164 项验收仍全部 pending。
