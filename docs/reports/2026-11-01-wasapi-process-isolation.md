# 隔离进程有界终止卡住的媒体任务

## 结果

父进程可以在期限内强制结束已卡住的 Windows 子进程。等待超时返回 `CLOCK_PROBE_PROCESS_STOP_TIMEOUT`，子进程句柄仍由调用方持有。进程退出不是硬件静音。无设备的卡住子进程在 80ms 预算内超时后仍可被结束。打开静音实验流后主线程卡住的子进程，父进程在 2 秒内结束，随后新客户端可以再打开设备并在 500ms 内 `stop_within` 成功。

`cargo test -p nai-audio-cpal --test clocked_process_isolation --features clocked-experiment --offline -- --test-threads=1` 2 项通过、2 项忽略（仅由父进程调用的子测试）。`cargo clippy -p nai-audio-cpal --features clocked-experiment --all-targets --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.14 已按这次验收勾选。1.3.10 整项没有勾选。

## 没有改变的路径

没有 OBS 实录，没有口型评分，没有平台开播。卡住的是子进程主线程，不是 COM 调用或内核/驱动卡死。没有生产子进程宿主、跨进程租约或设备失联演练。Drop 仍可能无界 join。该演练不授予播放许可。164 项验收仍全部 pending。
