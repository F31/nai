# 形象许可绑定

## 结果

素材和许可身份绑定已经落在 `nai-media-core` 的内存账本。已准入许可的 `package_sha256` 与 `rights_sha256` 必须与 `CharacterIdentity` 一致，宿主才打开为 Preview。撤权调用宿主取消，状态为 Revoked，动作和媒体引用被清空，该许可进入撤销集合。恢复已撤权利、`restore_last`，或以不高于已撤 epoch 的代次再绑定，一律作为恢复故障拒绝。该账本不发出 PublicActive，不打开声卡，不接入工作台或公共 keeper。绑定通过只说明许可与身份一致，播放许可仍由执行租约签发。2.3 已勾选。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。

## 绑定、撤权与恢复

`CharacterLicenseGate::bind` 校验身份形状、许可摘要与身份一致、摘要未被撤销、epoch 大于该场次已撤水位，然后打开 `CharacterPreviewHost`。包摘要或权利摘要不一致返回 Rights，已有活绑定返回 AlreadyBound。`revoke` 取消宿主、丢掉活宿主引用、记录 `(workspace, package, rights)`，并把该场次 epoch 写入水位。此后 `tick` 与 `begin_action` 返回 Closed。

`restore_last` 用撤权前的同一许可、身份、场次和 epoch 再绑定，固定返回 Restore。同一摘要换更高 epoch 仍为 Restore。新摘要且 epoch 仍为已撤水位则 Restore；新摘要且更高 epoch 可以打开预览。该账本不是 `adapters/storage-sqlite` 的形象权限原型，也不是生产数据库或 IPC。

生产源码不含 `CharacterRuntimeState::PublicActive` 或 `PublicStandby` 赋值，也不含 `PlaybackLease`。工作台 UI 和公共输出 keeper 本轮未接线。

## 校验

`cargo test -p nai-media-core --offline --lib`：83 项通过，其中许可 6 项。`cargo clippy -p nai-media-core --all-targets --offline -- -D warnings` 通过。

匹配绑定打开 Preview，动作和媒体为空。包摘要或权利摘要不一致、缺版本、epoch 0 均被拒绝且不打开宿主。撤权后状态为 Revoked、理由 `CANCELLED`，活宿主为空，后续 tick 被拒绝。`restore_last` 与同一摘要的更高 epoch 均返回 Restore。新摘要在旧 epoch 返回 Restore，在 epoch 2 打开 Preview；活绑定期间再次绑定返回 AlreadyBound。预览视图通过结构校验，状态不是公共活动或公共待机。

## 仍开放的项

2.4 工作台一键生成/预览/选择/公共输出仍开放。2.4.1–2.4.3 与 2.5 仍开放。1.3.1 写实样片仍开放。设置页选择形象仍不会进入公共执行。164 项验收仍全部 pending。
