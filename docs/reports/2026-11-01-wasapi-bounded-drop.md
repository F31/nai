# 隔离实验探测析构有界停止

## 结果

实验探测析构走 `stop_within(DROP_JOIN_BUDGET)`，期限 500ms。超时脱离 `JoinHandle` 所有权，析构在该期限内返回。超时不是硬件静音。停靠探测构造收到 `parked_probe`。

`cargo test -p nai-audio-cpal --lib --features clocked-experiment --offline -- --test-threads=1` 24 项通过，含新增的 `drop_of_unfinished_probe_returns_within_budget`。`cargo clippy -p nai-audio-cpal --features clocked-experiment --all-targets --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.27 已按这次验收勾选。1.3.10 整项和 1.4 没有勾选。

## 没有改变的路径

没有 OBS 实录，没有口型评分，没有平台开播。没有真实挑战续租、认证 IPC 或生产许可。没有替换 `open_controlled`。没有授予播放许可。超时脱离后的工作线程仍可能继续跑到自己的停点。已提交的操作系统缓冲区仍无法撤回。没有重跑静音设备硬件项。164 项验收仍全部 pending。
