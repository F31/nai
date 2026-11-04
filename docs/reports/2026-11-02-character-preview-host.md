# 形象私有预览宿主

## 结果

独立私有预览宿主已经落在 `nai-media-core`，消费 2.1 的形象身份、有界媒体和运行视图。打开即为 Preview。画面时钟按已播放源样本推进，每帧 800 样本，上限 480000。取消进入 Revoked，失联进入 Lost；此后视图不含动作或媒体，后续时钟推进被拒绝。该宿主不发出 PublicActive，不打开声卡，不接入公共 keeper。预览通过结构校验只说明形状合法，播放许可仍由执行租约签发。2.2 已勾选。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。

## 宿主与时钟

`CharacterPreviewHost::open` 校验身份、场次 UUID 和 epoch，状态固定为 Preview。`begin_action` 只接受已准入动作，并按公共尺寸写出 `CharacterBoundedMedia`（512×288、24 kHz 单声道、`rgb32le` / `s16le-mono-24000`）。样本上限 0 或超过 480000 在写入媒体前拒绝。PCM 和像素仍走独立通道，宿主消息不含原始载荷。

`tick` 用已播放源样本计算帧序号：`frame_index = min(played / 800, max_frames - 1)`，`sample_start = frame_index * 800`。已播放回退或超过 `max_samples` 返回 Bounds，状态保持 Preview。取消或失联后 `tick` 和 `begin_action` 返回 Closed。第一次关闭锁定状态：取消后再失联仍为 Revoked，失联后再取消仍为 Lost。

生产源码不含 `CharacterRuntimeState::PublicActive` 或 `PublicStandby` 赋值。工作台 UI 和公共输出 keeper 本轮未接线。

## 校验

`cargo test -p nai-media-core --offline --lib`：77 项通过，其中宿主 7 项。`cargo clippy -p nai-media-core --all-targets --offline -- -D warnings` 通过。首次链接出现 LNK1104，重试后通过，未改代码。

打开合法身份得到 Preview，动作和媒体为空。`speak` 且 3200 样本时，已播放 0 / 800 / 801 / 3200 分别对应帧 0 / 1 / 1 / 3；3201 和回退到 3199 被拒绝。取消后状态为 Revoked、理由 `CANCELLED`，后续 tick 被拒绝。失联后状态为 Lost、理由 `LOST`。缺版本身份、中文场次、epoch 0、动作 `dance` 和样本 480001 均被拒绝。预览视图通过结构校验，状态不是公共活动或公共待机。

## 仍开放的项

2.3 许可绑定、引用清理和撤权停播仍开放。2.4 工作台一键生成/预览/选择/公共输出仍开放。2.5 职责核对仍开放。1.3.1 写实样片仍开放。设置页选择形象仍不会进入公共执行。164 项验收仍全部 pending。
