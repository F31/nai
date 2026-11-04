# 隔离实验写入消费外部无阻塞租约栅栏

## 结果

实验音频写入在武装后、暂存前和提交前消费调用方的无阻塞租约回调。回调失效时撤销一次性栅栏并停止提交，该停止不是设备故障。未武装时不查询该回调。便捷入口对外部受控栅栏返回 `CLOCK_PROBE_EXTERNAL_FENCE_REQUIRED`。两处重复检查收成 `consume_external_lease`。

`cargo test -p nai-audio-cpal --lib --features clocked-experiment --offline -- --test-threads=1` 23 项通过，含新增的 `write_stops_when_external_lease_drops`。`cargo clippy -p nai-audio-cpal --features clocked-experiment --all-targets --offline -- -D warnings` 通过。`openspec validate v49-realistic-avatar --strict` 返回 “Change 'v49-realistic-avatar' is valid”。1.3.16 已按这次验收勾选。1.3.10 整项没有勾选。

## 没有改变的路径

没有 OBS 实录，没有口型评分，没有平台开播。没有真实挑战续租、认证 IPC 或生产许可。没有替换 `open_controlled`。没有授予播放许可。Drop 仍可能无界 join。没有重跑静音设备硬件项。164 项验收仍全部 pending。
