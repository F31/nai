# 隔离实验暂停路径消费外部租约

## 结果

实验暂停路径在发布设备观测前消费调用方的无阻塞租约回调。回调失效则撤销一次性栅栏并停止该循环，不再发布观测。该停止不是设备故障。未武装时不查询该回调。

`cargo test -p nai-audio-cpal --lib --features clocked-experiment --offline -- --test-threads=1` 首次遇到 LNK1104，重试后 26 项通过，含新增的 `hold_stops_publishing_when_external_lease_drops`。`cargo clippy -p nai-audio-cpal --features clocked-experiment --all-targets --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.29 已按这次验收勾选。1.3.10 整项和 1.4 没有勾选。

## 没有改变的路径

没有 OBS 实录，没有口型评分，没有平台开播。没有真实挑战续租、认证 IPC 或生产许可。没有替换 `open_controlled`。没有授予播放许可。已提交的操作系统缓冲区仍无法撤回。超时脱离后的工作线程仍可能继续跑到自己的停点。没有重跑静音设备硬件项。164 项验收仍全部 pending。
