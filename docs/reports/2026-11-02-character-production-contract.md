# 形象生产契约

## 结果

生产宿主装配前，形象身份、版本、运行状态、动作和有界媒体已经落在 `schemas/domain/media.proto`，并生成 Rust、TypeScript 和 C++。结构校验在 `nai-contracts`。2.1 已勾选。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。

身份必须带工作区 UUID、形象标识、版本，以及包摘要和权利摘要的小写十六进制 SHA-256。动作必须属于调用方给出的已准入列表。有界媒体只描述尺寸、采样、帧/样本上限和格式。公共尺寸钉在 512×288、24 kHz 单声道、每帧 800 样本、最多 480000 样本，与现有公共 PCM 上限一致。撤权、失联或不可用的视图不得携带动作或媒体。结构校验通过只说明消息形状合法，播放许可仍由执行租约签发。

## 生成与校验

`CharacterRuntimeState` 取值是不可用、预览、公共待机、公共活动、失联和撤权。`CharacterBoundedMedia` 与 `CharacterRuntimeView` 不含 `bytes`、PCM 或像素字段。PCM 和像素继续走独立媒体通道。

`cargo xtask schema` 写出 `crates/nai-contracts/src/generated/nai.v1.rs`、`packages/contracts-ts/src/generated/domain/media.ts` 与 `native/contracts/generated/domain/media.pb.{h,cc}`。TypeScript 入口已导出新类型。没有新增 frozen 能力，`avatar.render@1` 仍是 draft。

`cargo test -p nai-contracts --offline`：库测试 9 项通过，其中形象边界 5 项；协议测试 8 项通过，含运行状态视图未知字段往返。`cargo clippy -p nai-contracts --all-targets -- -D warnings` 通过。

身份缺版本、大写摘要或中文工作区会被拒绝。动作 `dance` / `wave` 在 `idle`/`speak` 列表外会被拒绝。样本上限 480001、帧数与样本不对齐、`f32le` 格式和 960×540 尺寸会被拒绝。撤权视图带媒体、失联视图带动作、未指定状态会被拒绝。公共活动和公共待机在媒体齐全时通过。

## 仍开放的项

2.2 独立宿主、音画样本时钟、失联取消和私有预览尚未装配。2.3 许可绑定、2.4 工作台接入和 2.5 职责核对仍开放。设置页选择形象仍不会进入公共执行。1.3.1 写实样片仍开放。164 项验收仍全部 pending。
