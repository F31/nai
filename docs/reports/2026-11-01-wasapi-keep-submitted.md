# 隔离实验撤销后已提交游标保持

## 结果

实验写入成功提交后，栅栏撤销拒绝后续写入，已提交游标保持原值。该保持不是操作系统缓冲区被撤回，也不是硬件静音。析构超时路径额外核对停止标志已置位。

`cargo test -p nai-audio-cpal --lib --features clocked-experiment --offline -- --test-threads=1` 首次遇到 LNK1104，重试后 25 项通过，含新增的 `revoke_after_commit_keeps_submitted_cursor_and_refuses_new_writes`。`cargo clippy -p nai-audio-cpal --features clocked-experiment --all-targets --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.28 已按这次验收勾选。1.3.10 整项和 1.4 没有勾选。

## 没有改变的路径

没有 OBS 实录，没有口型评分，没有平台开播。没有真实挑战续租、认证 IPC 或生产许可。没有替换 `open_controlled`。没有授予播放许可。已提交的操作系统缓冲区仍无法撤回。超时脱离后的工作线程仍可能继续跑到自己的停点。没有重跑静音设备硬件项。164 项验收仍全部 pending。
