# 工作台形象接入

## 结果

工作台一键生成、私有预览、工作区选择和公共输出状态已经接到 `nai-media-core` 的 `CharacterWorkbench`。生成绑定已准入许可并打开 `CharacterPreviewHost`，运行状态为 Preview。预览动作只允许 idle 与 speak；其他动作具名 `UNSUPPORTED_ACTION`。选择记录工作区形象，状态仍为 Preview。切换选择释放当前预览，不把原许可记入撤销。公共输出状态的后端固定为 `builtin-2d`。该接入不发出 PublicActive，不替换公共 keeper。生成、预览和选择都不是播放许可。桌面壳已接线 `workbench_character_generate` / `workbench_character_action` / `workbench_character_select` / `workbench_character_revoke`。2.4 已勾选。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。

## 生成、预览、选择与公共输出

`CharacterLicenseGate::release` 取消活宿主并丢掉活引用，不把摘要写入撤销集合，也不抬高 epoch 水位。工作台用它在改选形象时释放当前预览，原许可可以再次生成。

`CharacterWorkbench::generate` 在已有不同身份的活绑定时先 release 再 bind；同一身份直接返回当前预览。`preview` 走宿主 `begin_action`，未声明动作映射为 `UnsupportedAction`。`select` 等于 generate 并记下工作区选择。`public_output` 的 backend 恒为 `builtin-2d`，`playback_permit()` 恒为 false。`revoke` 走许可门撤权并清空选择。

桌面形象页导入或预览 PNG 之后调用生成和待机动作；选择先走原生 `characterSelect` 再写工作区草稿；撤销先走原生撤权，未绑定则忽略 `CHARACTER_UNBOUND`。照片路径仍开放。直播准备清单在已选择自定义形象时保持 action，并写明公共输出仍使用内置形象。

生产源码不含 `CharacterRuntimeState::PublicActive` 或 `PublicStandby` 赋值。PNG `PreviewSlot` 仍是静态图，不是 `CharacterPreviewHost`。

## 校验

`cargo test -p nai-media-core --offline --lib`：91 项通过，其中许可 7 项（含 release 1 项）、工作台 7 项。`cargo clippy -p nai-media-core --all-targets --offline -- -D warnings` 通过。嵌套工作区 `apps/workbench-desktop` 的 `cargo check --offline` 通过。`tsc --noEmit -p apps/workbench-desktop/tsconfig.json` 通过。`node --experimental-strip-types --test tests/ui/live-readiness.test.mjs`：5 项通过，含自定义形象选择保持 action、公共输出仍为内置形象。本轮未打开实际桌面窗口点击导入或选择。

生成打开 Preview，公共后端为 builtin-2d，选择为空，playback_permit 为 false。待机与说话预览状态仍为 Preview。`dance` 返回 UnsupportedAction。选择后 selected 为该身份，状态不是 PublicActive。改选另一形象后原许可可再次生成。撤权后 preview 为 Closed，同一摘要再次生成返回 Restore。

## 仍开放的项

2.4.1 视频导入/预处理、2.4.2 虚拟摄像头与 OBS 信号、2.4.3 外部渲染服务仍开放。2.5 与 3.x 仍开放。1.3.1 写实样片仍开放。自定义选择不会进入公共执行。164 项验收仍全部 pending。
