# 隔离实验事件等待后先停再检租约

## 结果

实验驱动在事件等待返回后、发布设备观测前先检查停止标志，再消费调用方的无阻塞租约回调。停止标志置位时拒绝发布。回调失效则撤销一次性栅栏并停止该循环，该停止不是设备故障。未武装时不查询该回调。事件等待本身仍可能跑完当前 100ms 上限。暂停路径与等待路径共用同一检查。

`cargo test -p nai-audio-cpal --lib --features clocked-experiment --offline -- --test-threads=1` 27 项通过，含新增的 `after_wait_stop_or_lease_drop_refuses_publish`。`cargo clippy -p nai-audio-cpal --features clocked-experiment --all-targets --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.30 已按这次验收勾选。1.3.10 整项和 1.4 没有勾选。

## 没有改变的路径

没有 OBS 实录，没有口型评分，没有平台开播。没有真实挑战续租、认证 IPC 或生产许可。没有替换 `open_controlled`。没有授予播放许可。已提交的操作系统缓冲区仍无法撤回。超时脱离后的工作线程仍可能跑完当前事件等待上限。没有重跑静音设备硬件项。164 项验收仍全部 pending。
